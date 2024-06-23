use crate::drivers::pci::*;
use crate::io::audio::{AudioDriver, AudioManager};
use crate::mem::{
    mmio::{MmioRegU16, MmioRegU32, MmioRegU8, MmioSlice},
    MemoryManager,
};
use crate::sync::{semaphore::Semaphore, Mutex};
use crate::task::scheduler::{Priority, SpawnOption, Timer};
use crate::*;
use core::intrinsics::copy_nonoverlapping;
use core::mem::transmute;
use core::num::{NonZeroU8, NonZeroUsize};
use core::ops::Add;
use core::slice;
use core::sync::atomic::{fence, AtomicUsize, Ordering};
use core::time::Duration;

pub type Result<T> = core::result::Result<T, ControllerError>;

pub struct HdaDriverRegistrar();

impl HdaDriverRegistrar {
    const PREFERRED_CLASS: PciClass = PciClass::code(0x04).sub(0x03);

    #[inline]
    pub fn new() -> Box<dyn PciDriverRegistrar> {
        Box::new(Self()) as Box<dyn PciDriverRegistrar>
    }
}

impl PciDriverRegistrar for HdaDriverRegistrar {
    fn instantiate(&self, device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        if device.class_code().matches(Self::PREFERRED_CLASS) {
            unsafe { HdAudioController::new(device) }
        } else {
            None
        }
    }
}

#[allow(dead_code)]
pub struct HdAudioController {
    addr: PciConfigAddress,
    mmio: MmioSlice,

    global: &'static GlobalRegisterSet,
    gcap: GlobalCapabilities,
    cmd: Mutex<CommandBuffer>,
    idss: Box<[Mutex<StreamDescriptor>]>,
    odss: Box<[Mutex<StreamDescriptor>]>,

    outputs: Vec<WidgetAddress>,
    inputs: Vec<WidgetAddress>,
    output_pins: Vec<WidgetAddress>,
    input_pins: Vec<WidgetAddress>,
    widgets: BTreeMap<WidgetAddress, Node>,

    current_output: Mutex<Option<WidgetAddress>>,
    current_dac: Mutex<Option<WidgetAddress>>,

    sem_event_thread: Semaphore,
}

unsafe impl Send for HdAudioController {}
unsafe impl Sync for HdAudioController {}

impl HdAudioController {
    pub const DRIVER_NAME: &'static str = "hdaudio";
    pub const CURRENT_VERSION: (usize, usize) = (1, 0);
    pub const WAIT_DELAY_MS: u64 = 100;

    #[inline]
    pub fn registrar() -> Box<dyn PciDriverRegistrar> {
        HdaDriverRegistrar::new()
    }

    pub unsafe fn new(device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        let Some(bar) = device.bars().next() else {
            return None;
        };
        let Some(mmio) = MmioSlice::from_bar(bar) else {
            return None;
        };

        device.set_pci_command(PciCommand::MEM_SPACE | PciCommand::BUS_MASTER);

        let global = mmio.transmute::<GlobalRegisterSet>(0);

        global.set_status(GlobalStatus::all());
        global.set_control(GlobalControl::empty());

        let deadline = Timer::new(Duration::from_millis(100));
        loop {
            if deadline.is_expired() || !global.get_control().contains(GlobalControl::CRST) {
                break;
            }
            Timer::sleep(Duration::from_millis(1));
        }
        assert!(
            !global.get_control().contains(GlobalControl::CRST),
            "HDAudio initialization failed"
        );

        global.set_control(GlobalControl::CRST);

        let deadline = Timer::new(Duration::from_millis(100));
        loop {
            if deadline.is_expired() || global.get_control().contains(GlobalControl::CRST) {
                break;
            }
            Timer::sleep(Duration::from_millis(1));
        }
        assert!(
            global.get_control().contains(GlobalControl::CRST),
            "HDAudio initialization failed"
        );

        let deadline = Timer::new(Duration::from_millis(100));
        loop {
            if deadline.is_expired() || global.get_state_change_status() != 0 {
                break;
            }
            Timer::sleep(Duration::from_millis(1));
        }

        let gcap = global.capabilities();
        let iss = gcap.iss;
        let oss = gcap.oss;

        let mut idss = Vec::with_capacity(iss);
        for i in 0..iss {
            idss.push(Mutex::new(StreamDescriptor::new(
                mmio.transmute::<StreamDescriptorRegisterSet>(0x80 + i * 0x20),
            )));
        }

        let mut odss = Vec::with_capacity(oss);
        for i in 0..oss {
            odss.push(Mutex::new(StreamDescriptor::new(
                mmio.transmute::<StreamDescriptorRegisterSet>(0x80 + iss * 0x20 + i * 0x20),
            )));
        }

        let immediate = match (device.vendor_id(), device.device_id()) {
            (PciVendorId(0x8086), PciDeviceId(0x2668)) => false,
            _ => true,
        };

        let cmd = Mutex::new(CommandBuffer::new(&mmio, immediate));

        let mut driver = Self {
            addr: device.address(),
            global,
            mmio,
            cmd,
            gcap,
            idss: idss.into_boxed_slice(),
            odss: odss.into_boxed_slice(),

            outputs: Vec::new(),
            inputs: Vec::new(),
            output_pins: Vec::new(),
            input_pins: Vec::new(),
            widgets: BTreeMap::new(),

            current_output: Mutex::new(None),
            current_dac: Mutex::new(None),

            sem_event_thread: Semaphore::new(0),
        };

        match driver.enumerate() {
            Ok(_) => (),
            Err(_) => return None,
        }

        if let Some(addr) = driver.find_best_output_pin() {
            let path = driver.path_to_dac(addr);
            let dac = *path.first().unwrap();
            driver.set_current_output(addr);
            driver.set_current_dac(dac);

            let stream_format = PcmFormat::default();
            let stream_id = StreamId(NonZeroU8::new_unchecked(1));
            let mut sd = driver.odss[0].lock().unwrap();
            sd.prepare_buffer(stream_id, stream_format);

            let mut cmd = driver.cmd.lock().unwrap();

            // TODO: magic number
            cmd.run(Command::new(
                addr,
                Verb::SetPinWidgetControl(PinWidgetControl(0xC0)),
            ))
            .unwrap();
            cmd.run(Command::new(addr, Verb::SetEapdBtlEnable(0x02)))
                .unwrap();

            for widget in path {
                let widget = driver.widgets.get(&widget).unwrap();

                cmd.set_amplifier_gain_mute(
                    widget.addr(),
                    AmplifierGainMuteSetPayload::new(
                        true,
                        false,
                        true,
                        true,
                        0,
                        false,
                        widget.output_amplifier_capabilities().offset(),
                    ),
                )
                .unwrap();

                // TODO: magic number
                cmd.run(Command::new(widget.addr(), Verb::SetPowerState(0x00)))
                    .unwrap();
            }

            cmd.set_pcm_format(dac, stream_format).unwrap();
            cmd.set_stream_id(dac, stream_id).unwrap();

            driver.global.ssync.write_volatile(1);

            sd.run();
        } else {
            return None;
        }

        if false {
            log!("OUTPUT: {:?}", driver.find_best_output_pin());
            for node in driver.widgets.values() {
                let cap = node.capabilities();
                let config = node.configuration_default();
                let out_cap = node.output_amplifier_capabilities();
                let in_cap = node.input_amplifier_capabilities();

                log!(
                    "{} {:08x} {:08x} {:08x} {:08x} {:?} {} {:?}",
                    node.addr().nid().0,
                    cap.bits(),
                    config.bits(),
                    in_cap.bits(),
                    out_cap.bits(),
                    cap.widget_type(),
                    cap.number_of_channels(),
                    config.default_device(),
                );
            }
            panic!("todo");
        }

        let driver = Arc::new(driver);

        let p = driver.clone();
        SpawnOption::with_priority(Priority::Realtime).spawn(
            move || {
                p._event_thread();
            },
            Self::DRIVER_NAME,
        );

        let p = Arc::as_ptr(&driver);
        Arc::increment_strong_count(p);
        device.register_msi(Self::_msi_handler, p as usize).unwrap();

        driver.global.set_interrupt_control((1 << 31) | (1 << iss));

        AudioManager::set_audio_driver(HdaSoundDriver::new(&driver));

        Some(driver as Arc<dyn PciDriver>)
    }

    fn _msi_handler(p: usize) {
        let this = unsafe { &*(p as *const Self) };
        this.sem_event_thread.signal();
    }

    /// event loop
    fn _event_thread(self: Arc<Self>) {
        loop {
            self.sem_event_thread.wait();
            self.process_events();
        }
    }

    pub fn process_events(&self) {
        let sts = self.global.interrupt_status();
        if (sts & (1 << 31)) != 0 {
            if (sts & (1 << 30)) != 0 {
                // TODO: Controller Interrupt Status
            }
            let mut sis = sts & 0x3FFF_FFFF;

            for i in 0..self.gcap.iss {
                if (sis & 1) != 0 {
                    let mut input = self.idss[i].lock().unwrap();
                    input.handle_interrupt();
                }
                sis >>= 1;
            }

            for i in 0..self.gcap.oss {
                if (sis & 1) != 0 {
                    let mut output = self.odss[i].lock().unwrap();
                    output.handle_interrupt();
                }
                sis >>= 1;
            }
        }
    }

    #[inline]
    pub fn current_output(&self) -> Option<WidgetAddress> {
        *self.current_output.lock().unwrap()
    }

    #[inline]
    pub fn set_current_output(&self, addr: WidgetAddress) {
        *self.current_output.lock().unwrap() = Some(addr);
    }

    #[inline]
    pub fn set_current_dac(&self, addr: WidgetAddress) {
        *self.current_dac.lock().unwrap() = Some(addr);
    }

    pub fn enumerate(&mut self) -> Result<()> {
        let cad = Cad::new(0);
        let mut cmd = self.cmd.lock().unwrap();

        let (start, count) = cmd.get_subordinate_node_count(WidgetAddress::new(cad, Nid::ROOT))?;

        for i in 0..count {
            let fg = WidgetAddress::new(cad, start + i);

            let (start, count) = cmd.get_subordinate_node_count(fg)?;

            for i in 0..count {
                let addr = WidgetAddress::new(cad, start + i);
                let widget = Node::new(&mut cmd, addr)?;
                match widget.capabilities().widget_type() {
                    WidgetType::AudioOutput => self.outputs.push(addr),
                    WidgetType::Audioinput => self.inputs.push(addr),
                    // WidgetType::BeepGenerator => todo!(),
                    WidgetType::PinComplex => {
                        if widget.configuration_default().is_output() {
                            self.output_pins.push(addr)
                        } else if widget.configuration_default().is_input() {
                            self.input_pins.push(addr)
                        }
                    }
                    _ => (),
                }
                self.widgets.insert(addr, widget);
            }
        }

        Ok(())
    }

    pub fn find_best_output_pin(&self) -> Option<WidgetAddress> {
        if self.output_pins.len() < 2 {
            self.output_pins.first().map(|v| *v)
        } else {
            for device in [
                DefaultDevice::HPOut,
                DefaultDevice::LineOut,
                DefaultDevice::Speaker,
            ] {
                for &pin in &self.output_pins {
                    let widget = self.widgets.get(&pin).unwrap();
                    let config = widget.configuration_default();
                    if config.sequence() == 0
                        && config.port_connectivity() != PortConnectivity::NoPhysicalConnection
                        && config.default_device() == device
                    {
                        return Some(pin);
                    }
                }
            }

            for device in [
                DefaultDevice::HPOut,
                DefaultDevice::LineOut,
                DefaultDevice::DigitalOtherOut,
            ] {
                for &pin in &self.output_pins {
                    let widget = self.widgets.get(&pin).unwrap();
                    let config = widget.configuration_default();
                    if config.sequence() == 0 && config.default_device() == device {
                        return Some(pin);
                    }
                }
            }

            None
        }
    }

    pub fn path_to_dac(&self, addr: WidgetAddress) -> Vec<WidgetAddress> {
        let mut vec = Vec::new();
        self._path_to_dac(&mut vec, addr, 8);
        vec
    }

    fn _path_to_dac(&self, vec: &mut Vec<WidgetAddress>, addr: WidgetAddress, ttl: usize) -> bool {
        let widget = self.widgets.get(&addr).unwrap();
        if widget.capabilities().widget_type() == WidgetType::AudioOutput {
            vec.push(addr);
            true
        } else {
            if ttl == 0 {
                return false;
            }
            widget
                .connections()
                .first()
                .map(|child| {
                    if self._path_to_dac(vec, *child, ttl - 1) {
                        vec.push(addr);
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false)
        }
    }
}

// impl Drop for HdAudioController {
//     fn drop(&mut self) {
//         todo!()
//     }
// }

impl PciDriver for HdAudioController {
    fn address(&self) -> PciConfigAddress {
        self.addr
    }

    fn name<'a>(&self) -> &'a str {
        Self::DRIVER_NAME
    }

    fn current_status(&self) -> String {
        if let Some(addr) = self.current_output() {
            let widget = self.widgets.get(&addr).unwrap();
            let config = widget.configuration_default();
            format!(
                "RUNNING iss {} oss {} pin {} {:?} {:?} {:?}",
                self.gcap.iss,
                self.gcap.oss,
                widget.addr().pretty(),
                config.default_device(),
                config.port_connectivity(),
                config.color(),
            )
        } else {
            format!("* NO SOUND")
        }
    }
}

pub struct HdaSoundDriver {
    hda: Arc<HdAudioController>,
}

impl HdaSoundDriver {
    #[inline]
    pub fn new(hda: &Arc<HdAudioController>) -> Arc<dyn AudioDriver> {
        Arc::new(Self { hda: hda.clone() }) as Arc<dyn AudioDriver>
    }
}

impl AudioDriver for HdaSoundDriver {
    fn size_of_buffer(&self) -> usize {
        SIZE_OF_BUFFER
    }

    fn write_block(&self, data: &[u8]) -> Option<()> {
        let sd = self.hda.odss[0].lock().unwrap();
        sd.write_data(data)
    }

    fn set_master_volume(&self, gain: usize) -> bool {
        let addr = self.hda.current_dac.lock().unwrap().unwrap();
        let widget = self.hda.widgets.get(&addr).unwrap();
        let cap = widget.output_amplifier_capabilities();
        let delta = (gain * 4).checked_div(cap.step_size()).unwrap_or_default();
        if delta > cap.offset() {
            self.hda
                .cmd
                .lock()
                .unwrap()
                .set_amplifier_gain_mute(
                    widget.addr(),
                    AmplifierGainMuteSetPayload::new(true, false, true, true, 0, true, 0),
                )
                .unwrap();
            true
        } else {
            self.hda
                .cmd
                .lock()
                .unwrap()
                .set_amplifier_gain_mute(
                    widget.addr(),
                    AmplifierGainMuteSetPayload::new(
                        true,
                        false,
                        true,
                        true,
                        0,
                        false,
                        cap.offset() - delta,
                    ),
                )
                .unwrap();
            false
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum ControllerError {
    CommandBusy,
    CommandNotResponding,
    StreamNotReady,
}

#[derive(Clone)]
pub struct Node {
    addr: WidgetAddress,
    cap: AudioWidgetCapabilities,
    config: ConfigurationDefault,
    out_amp_cap: AmplifierCapabilities,
    in_amp_cap: AmplifierCapabilities,
    connections: Box<[WidgetAddress]>,
}

impl Node {
    pub fn new(cmd: &mut CommandBuffer, addr: WidgetAddress) -> Result<Self> {
        let cap = cmd.get_audio_widget_capabilities(addr)?;
        let config_default = cmd.get_configuration_default(addr)?;
        let out_amp_cap = cmd.get_output_amplifier_capabilities(addr)?;
        let in_amp_cap = cmd.get_input_amplifier_capabilities(addr)?;

        let val = cmd
            .get_parameter(addr, ParameterId::ConnectionListLength)?
            .as_u32();
        let count = (val & 0x7F) as u8;
        if (val & 0x80) != 0 {
            todo!();
        }

        let mut cursor = 0;
        let mut list = Vec::new();
        while cursor < count {
            let res = cmd
                .run(Command::new(addr, Verb::GetConectionListEntry(cursor)))?
                .as_u32();

            for i in 0..4 {
                let val = ((res >> (i * 8)) & 0xFF) as u8;
                if (val & 0x80) != 0 {
                    todo!();
                }
                if val == 0 {
                    break;
                }
                list.push(WidgetAddress::new(addr.cad(), Nid(val)));
            }
            cursor = list.len() as u8;
        }

        Ok(Self {
            addr,
            cap,
            config: config_default,
            out_amp_cap,
            in_amp_cap,
            connections: list.into_boxed_slice(),
        })
    }

    #[inline]
    pub const fn addr(&self) -> WidgetAddress {
        self.addr
    }

    #[inline]
    pub const fn capabilities(&self) -> AudioWidgetCapabilities {
        self.cap
    }

    #[inline]
    pub const fn configuration_default(&self) -> ConfigurationDefault {
        self.config
    }

    #[inline]
    pub const fn output_amplifier_capabilities(&self) -> AmplifierCapabilities {
        self.out_amp_cap
    }

    #[inline]
    pub const fn input_amplifier_capabilities(&self) -> AmplifierCapabilities {
        self.in_amp_cap
    }

    #[inline]
    pub fn connections(&self) -> &[WidgetAddress] {
        &self.connections
    }
}

pub enum CommandBuffer {
    RingBuffer(Corb, Rirb),
    Immediate(&'static ImmediateCommandRegisterSet),
}

impl CommandBuffer {
    pub unsafe fn new(mmio: &MmioSlice, immediate: bool) -> Self {
        if immediate {
            Self::Immediate(mmio.transmute(0x60))
        } else {
            let corb = Corb::new(mmio.transmute(0x40));
            let rirb = Rirb::new(mmio.transmute(0x50));
            corb.regs.start();
            rirb.regs.start();
            Self::RingBuffer(corb, rirb)
        }
    }

    pub fn run(&mut self, cmd: Command) -> Result<Response> {
        match self {
            CommandBuffer::RingBuffer(corb, rirb) => {
                corb.issue_command(cmd)?;
                rirb.read_response()
            }
            CommandBuffer::Immediate(ref icr) => icr.command(cmd),
        }
    }

    #[inline]
    pub fn get_parameter(&mut self, addr: WidgetAddress, param: ParameterId) -> Result<Response> {
        self.run(Command::new(addr, Verb::GetParameter(param)))
    }

    #[inline]
    pub fn get_subordinate_node_count(&mut self, addr: WidgetAddress) -> Result<(Nid, u8)> {
        self.get_parameter(addr, ParameterId::SubordinateNodeCount)
            .map(|v| {
                let v = v.as_u32();
                (Nid::new((v >> 16) as u8), v as u8)
            })
    }

    #[inline]
    pub fn get_audio_widget_capabilities(
        &mut self,
        addr: WidgetAddress,
    ) -> Result<AudioWidgetCapabilities> {
        self.get_parameter(addr, ParameterId::AudioWidgetCapabilities)
            .map(|v| unsafe { transmute(v.as_u32()) })
    }

    #[inline]
    pub fn get_output_amplifier_capabilities(
        &mut self,
        addr: WidgetAddress,
    ) -> Result<AmplifierCapabilities> {
        self.get_parameter(addr, ParameterId::OutputAmpCapabilities)
            .map(|v| AmplifierCapabilities(v.as_u32()))
    }

    #[inline]
    pub fn get_input_amplifier_capabilities(
        &mut self,
        addr: WidgetAddress,
    ) -> Result<AmplifierCapabilities> {
        self.get_parameter(addr, ParameterId::InputAmpCapabilities)
            .map(|v| AmplifierCapabilities(v.as_u32()))
    }

    #[inline]
    pub fn get_configuration_default(
        &mut self,
        addr: WidgetAddress,
    ) -> Result<ConfigurationDefault> {
        self.run(Command::new(addr, Verb::GetConfigurationDefault))
            .map(|v| ConfigurationDefault(v.as_u32()))
    }

    #[inline]
    pub fn get_supported_sample_format(
        &mut self,
        addr: WidgetAddress,
    ) -> Result<SupportedPCMFormat> {
        self.get_parameter(addr, ParameterId::SampleSizeRateCaps)
            .map(|v| SupportedPCMFormat::from_bits_retain(v.as_u32()))
    }

    #[inline]
    pub fn set_pcm_format(&mut self, addr: WidgetAddress, format: PcmFormat) -> Result<()> {
        self.run(Command::new(addr, Verb::SetConverterFormat(format)))
            .map(|_| ())
    }

    #[inline]
    pub fn set_stream_id(&mut self, addr: WidgetAddress, id: StreamId) -> Result<()> {
        self.run(Command::new(
            addr,
            Verb::SetConverterControl(id.as_u8() << 4),
        ))
        .map(|_| ())
    }

    #[inline]
    pub fn set_amplifier_gain_mute(
        &mut self,
        addr: WidgetAddress,
        payload: AmplifierGainMuteSetPayload,
    ) -> Result<()> {
        self.run(Command::new(addr, Verb::SetAmplifierGainMute(payload)))
            .map(|_| ())
    }
}

/// Command Output Ring Buffer
pub struct Corb {
    regs: &'static CorbRegisterSet,
    buffer: &'static mut [Command],
    len: usize,
}

impl Corb {
    #[track_caller]
    pub unsafe fn new(regs: &'static CorbRegisterSet) -> Self {
        let len = regs.entries().unwrap().get();
        let (pa_corb, va_corb) = MemoryManager::alloc_dma::<Command>(len).unwrap();
        let buffer = slice::from_raw_parts_mut(va_corb, len);

        regs.init(pa_corb);

        Self { regs, buffer, len }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn can_write(&self) -> bool {
        self.regs.get_write_pointer() == self.regs.get_read_pointer()
    }

    pub fn issue_command(&mut self, cmd: Command) -> Result<()> {
        let deadline = Timer::new(Duration::from_millis(HdAudioController::WAIT_DELAY_MS));
        let mut wait = Hal::cpu().spin_wait();
        while deadline.is_alive() && !self.can_write() {
            wait.wait();
        }
        if !self.can_write() {
            return Err(ControllerError::CommandBusy);
        }

        let index = (self.regs.get_write_pointer() + 1) % self.len();
        unsafe {
            *self.buffer.get_unchecked_mut(index) = cmd;
        }
        fence(Ordering::SeqCst);
        self.regs.set_write_pointer(index);

        Ok(())
    }
}

/// Response Input Ring Buffer
pub struct Rirb {
    regs: &'static RirbRegisterSet,
    buffer: &'static [Response],
    len: usize,
    read_pointer: AtomicUsize,
}

impl Rirb {
    pub unsafe fn new(regs: &'static RirbRegisterSet) -> Self {
        let len = regs.entries().unwrap().get();
        let (pa_rirb, va_rirb) = MemoryManager::alloc_dma::<Response>(len).unwrap();
        let buffer = slice::from_raw_parts(va_rirb, len);
        regs.init(pa_rirb);
        Self {
            regs,
            buffer,
            len,
            read_pointer: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn has_response(&self) -> bool {
        self.regs.get_write_pointer() != self.read_pointer.load(Ordering::SeqCst)
    }

    pub fn read_response(&mut self) -> Result<Response> {
        let deadline = Timer::new(Duration::from_millis(HdAudioController::WAIT_DELAY_MS));
        let mut wait = Hal::cpu().spin_wait();
        while deadline.is_alive() && !self.has_response() {
            wait.wait();
        }
        if !self.has_response() {
            return Err(ControllerError::CommandNotResponding);
        }

        fence(Ordering::SeqCst);
        let index = (self.read_pointer.load(Ordering::SeqCst) + 1) % self.len();
        let result = unsafe { *self.buffer.get_unchecked(index) };
        self.read_pointer.store(index, Ordering::SeqCst);

        self.regs.set_status(RirbStatus::RINTFL);

        Ok(result)
    }
}

const SIZE_OF_BUFFER: usize = 0x1000;
const NUM_OF_BUFFER: usize = 4;
pub type DmaBufferChunk = [u8; SIZE_OF_BUFFER];

pub struct StreamDescriptor {
    regs: &'static StreamDescriptorRegisterSet,
    id: Option<StreamId>,
    current_buffer: Option<*mut u8>,
    current_pos: AtomicUsize,
}

impl StreamDescriptor {
    #[inline]
    pub fn new(regs: &'static StreamDescriptorRegisterSet) -> Self {
        Self {
            regs,
            id: None,
            current_buffer: None,
            current_pos: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn stream_id(&self) -> Option<StreamId> {
        self.id
    }

    #[inline]
    pub fn set_pcm_format(&mut self, fmt: PcmFormat) {
        self.regs.set_pcm_format(fmt);
    }

    #[inline]
    pub fn run(&mut self) {
        let mut ctl = self.regs.get_control();
        ctl.insert(StreamDescriptorControl::RUN | StreamDescriptorControl::IOCE);
        self.regs.set_control(ctl);
    }

    #[inline]
    pub fn stop(&mut self) {
        let mut ctl = self.regs.get_control();
        ctl.remove(StreamDescriptorControl::RUN);
        self.regs.set_control(ctl);

        self.regs.set_status(self.regs.get_status());
    }

    pub fn handle_interrupt(&mut self) {
        self.regs.clear_interrupts();
    }

    pub fn prepare_buffer(&mut self, id: StreamId, fmt: PcmFormat) {
        self.id = Some(id);
        let (pa_bdl, bdl) =
            unsafe { MemoryManager::alloc_dma::<[BufferDescriptor; NUM_OF_BUFFER]>(1).unwrap() };
        let bdl = unsafe { &mut *bdl };

        let (pa_buff, buffer) =
            unsafe { MemoryManager::alloc_dma::<u8>(NUM_OF_BUFFER * SIZE_OF_BUFFER).unwrap() };
        for i in 0..bdl.len() {
            bdl[i] =
                BufferDescriptor::new(pa_buff + (i * SIZE_OF_BUFFER) as u64, SIZE_OF_BUFFER, true);
        }

        self.regs.set_stream_id(id);
        self.regs.set_pcm_format(fmt);
        self.regs.set_base(pa_bdl);
        self.regs.set_buffer_length(SIZE_OF_BUFFER * NUM_OF_BUFFER);
        self.regs.set_last_valid_index(NUM_OF_BUFFER - 1);

        self.current_buffer = Some(buffer);
    }

    pub fn write_data(&self, data: &[u8]) -> Option<()> {
        let buffer = match self.current_buffer {
            Some(v) => v,
            None => return Some(()),
        };
        let src = data.as_ptr();

        let lp = self.regs.link_position() / SIZE_OF_BUFFER;
        let pos = self.current_pos.load(Ordering::SeqCst);

        if lp != pos {
            self.current_pos
                .store((pos + 1) % NUM_OF_BUFFER, Ordering::SeqCst);

            unsafe {
                copy_nonoverlapping(src, buffer.add(pos * SIZE_OF_BUFFER), SIZE_OF_BUFFER);
            }
            fence(Ordering::SeqCst);
            Some(())
        } else {
            None
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(pub NonZeroU8);

impl StreamId {
    pub const MAX: Self = Self(unsafe { NonZeroU8::new_unchecked(0x0F) });

    #[inline]
    pub const fn as_u8(&self) -> u8 {
        self.0.get()
    }
}

/// Paired CAD and NID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WidgetAddress {
    pub cad: Cad,
    pub nid: Nid,
}

impl WidgetAddress {
    #[inline]
    pub const fn new(cad: Cad, nid: Nid) -> Self {
        Self { cad, nid }
    }

    #[inline]
    pub const fn as_corb(&self) -> u32 {
        self.cad.as_corb() | self.nid.as_corb()
    }

    #[inline]
    pub const fn cad(&self) -> Cad {
        self.cad
    }

    #[inline]
    pub const fn nid(&self) -> Nid {
        self.nid
    }

    #[inline]
    pub fn pretty(&self) -> String {
        format!("{:x}:{:02x}", self.cad.0, self.nid.0)
    }
}

/// Codec Address
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cad(pub u8);

impl Cad {
    pub const MAX: Self = Self(0x0F);

    #[inline]
    pub const fn new(val: u8) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn as_corb(&self) -> u32 {
        (self.0 as u32) << 28
    }
}

/// Node Id (Short form)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nid(pub u8);

impl Nid {
    pub const ROOT: Self = Self(0);
    pub const MAX: Self = Self(0x7F);

    #[inline]
    pub const fn new(val: u8) -> Self {
        Self(val & 0x7F)
    }

    #[inline]
    pub const fn as_corb(&self) -> u32 {
        (self.0 as u32) << 20
    }
}

impl<T: Into<usize>> Add<T> for Nid {
    type Output = Self;
    fn add(self, rhs: T) -> Self::Output {
        Self(self.0 + rhs.into() as u8)
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct GlobalRegisterSet {
    gcap: MmioRegU16,
    vmin: MmioRegU8,
    vmaj: MmioRegU8,
    outpay: MmioRegU16,
    inpay: MmioRegU16,
    gctl: MmioRegU32,
    wakeen: MmioRegU16,
    statests: MmioRegU16,
    gsts: MmioRegU16,
    _rsrv_12_17: [u8; 6],
    outstrmpay: MmioRegU16,
    instrmpay: MmioRegU16,
    _rsrv_1c_1f: [u8; 4],
    intcnt: MmioRegU32,
    intsts: MmioRegU32,
    _rsrv_28_2f: [u8; 8],
    counter: MmioRegU32,
    ssync: MmioRegU32,
}

impl GlobalRegisterSet {
    #[inline]
    pub fn capabilities(&self) -> GlobalCapabilities {
        self.gcap.read_volatile().into()
    }

    #[inline]
    pub fn version(&self) -> (usize, usize) {
        (
            self.vmaj.read_volatile() as usize,
            self.vmin.read_volatile() as usize,
        )
    }

    #[inline]
    pub fn output_payload_capability(&self) -> usize {
        self.outpay.read_volatile() as usize
    }

    #[inline]
    pub fn input_payload_capability(&self) -> usize {
        self.inpay.read_volatile() as usize
    }

    #[inline]
    pub fn get_control(&self) -> GlobalControl {
        GlobalControl::from_bits_retain(self.gctl.read_volatile())
    }

    #[inline]
    pub fn set_control(&self, val: GlobalControl) {
        self.gctl.write_volatile(val.bits());
    }

    #[inline]
    pub fn get_wake_enable(&self) -> u16 {
        self.wakeen.read_volatile()
    }

    #[inline]
    pub fn set_wake_enable(&self, val: u16) {
        self.wakeen.write_volatile(val);
    }

    #[inline]
    pub fn get_state_change_status(&self) -> u16 {
        self.statests.read_volatile()
    }

    #[inline]
    pub fn set_state_change_status(&self, val: u16) {
        self.statests.write_volatile(val);
    }

    #[inline]
    pub fn get_status(&self) -> GlobalStatus {
        GlobalStatus::from_bits_retain(self.gsts.read_volatile())
    }

    #[inline]
    pub fn set_status(&self, val: GlobalStatus) {
        self.gsts.write_volatile(val.bits());
    }

    #[inline]
    pub fn output_stream_payload_capability(&self) -> usize {
        self.outstrmpay.read_volatile() as usize
    }

    #[inline]
    pub fn input_stream_payload_capability(&self) -> usize {
        self.instrmpay.read_volatile() as usize
    }

    #[inline]
    pub fn interrupt_control(&self) -> u32 {
        self.intcnt.read_volatile()
    }

    #[inline]
    pub fn set_interrupt_control(&self, val: u32) {
        self.intcnt.write_volatile(val);
    }

    #[inline]
    pub fn interrupt_status(&self) -> u32 {
        self.intsts.read_volatile()
    }
}

my_bitflags! {
    pub struct GlobalControl: u32 {
        /// Controller Reset
        const CRST      = 0x0000_0001;
        /// Flush Control
        const FCNTRL    = 0x0000_0002;
        /// Accept Unsolicited Response Enable
        const UNSOL     = 0x0000_0100;
    }
}

my_bitflags! {
    pub struct GlobalStatus: u16 {
        const FSTS      = 0x0002;
    }
}

my_bitflags! {
    pub struct InterruptControl: u32 {
        const GIE       = 0x8000_0000;
        const CIE       = 0x4000_0000;
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct GlobalCapabilities {
    pub oss: usize,
    pub iss: usize,
    pub bss: usize,
    pub nsdo: usize,
    pub supports_64bit: bool,
}

impl From<u16> for GlobalCapabilities {
    #[inline]
    fn from(val: u16) -> Self {
        let oss = ((val >> 12) & 15) as usize;
        let iss = ((val >> 8) & 15) as usize;
        let bss = ((val >> 3) & 31) as usize;
        let nsdo = ((val >> 1) & 3) as usize;
        let supports_64bit = (val & 1) != 0;
        GlobalCapabilities {
            oss,
            iss,
            bss,
            nsdo,
            supports_64bit,
        }
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct CorbRegisterSet {
    lbase: MmioRegU32,
    ubase: MmioRegU32,
    wp: MmioRegU16,
    rp: MmioRegU16,
    ctl: MmioRegU8,
    sts: MmioRegU8,
    size: MmioRegU8,
}

my_bitflags! {
    pub struct CorbControl: u8 {
        /// DMA Run
        const RUN   = 0b0000_0010;
        /// Memory Error Interrupt Enable
        const MEIE  = 0b0000_0001;
    }
}

my_bitflags! {
    pub struct CorbStatus: u8 {
        /// Memory Error Indication
        const MEI   = 0b0000_0001;
    }
}

impl CorbRegisterSet {
    pub const CORBRPRST: u16 = 0x8000;

    pub unsafe fn init(&self, pa_corb: PhysicalAddress) {
        self.stop();

        self.set_write_pointer(0);

        self.rp.write_volatile(Self::CORBRPRST);
        Timer::sleep(Duration::from_millis(100));
        // self.rp.store(0, Ordering::SeqCst);

        self.set_base(pa_corb);
    }

    #[inline]
    pub fn start(&self) {
        self.set_control(CorbControl::RUN);
    }

    #[inline]
    pub fn stop(&self) {
        while self.get_control().contains(CorbControl::RUN) {
            self.set_control(CorbControl::empty());
        }
    }

    #[inline]
    pub fn set_base(&self, base: PhysicalAddress) {
        let base = base.as_u64();
        self.lbase.write_volatile(base as u32);
        self.ubase.write_volatile((base >> 32) as u32);
    }

    #[inline]
    pub fn get_write_pointer(&self) -> usize {
        (self.wp.read_volatile() & 0xFF) as usize
    }

    #[inline]
    pub fn set_write_pointer(&self, val: usize) {
        self.wp.write_volatile((val & 0xFF) as u16);
    }

    #[inline]
    pub fn get_read_pointer(&self) -> usize {
        (self.rp.read_volatile() & 0xFF) as usize
    }

    #[inline]
    pub fn get_control(&self) -> CorbControl {
        CorbControl::from_bits_retain(self.ctl.read_volatile())
    }

    #[inline]
    pub fn set_control(&self, val: CorbControl) {
        self.ctl.write_volatile(val.bits());
    }

    #[inline]
    pub fn get_status(&self) -> CorbStatus {
        CorbStatus::from_bits_retain(self.sts.read_volatile())
    }

    #[inline]
    pub fn set_status(&self, val: CorbStatus) {
        self.sts.write_volatile(val.bits());
    }

    #[inline]
    pub fn entries(&self) -> Option<NonZeroUsize> {
        match self.size.read_volatile() & 3 {
            0 => NonZeroUsize::new(2),
            1 => NonZeroUsize::new(16),
            2 => NonZeroUsize::new(256),
            _ => None,
        }
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct RirbRegisterSet {
    lbase: MmioRegU32,
    ubase: MmioRegU32,
    wp: MmioRegU16,
    rintcnt: MmioRegU16,
    ctl: MmioRegU8,
    sts: MmioRegU8,
    size: MmioRegU8,
}

my_bitflags! {
    pub struct RirbControl: u8 {
        /// Response Interrupt Control
        const RINTCTL   = 0b0000_0001;
        /// DMA Run
        const DMAEN     = 0b0000_0010;
        /// Response Overrun Interrupt Control
        const OIC       = 0b0000_0100;
    }
}

my_bitflags! {
    pub struct RirbStatus: u8 {
        /// Response Interrupt
        const RINTFL    = 0b0000_0001;
        /// Response Overrun Interrupt Status
        const OIS       = 0b0000_0100;
    }
}

impl RirbRegisterSet {
    pub const RIRBWPRST: u16 = 0x8000;

    fn init(&self, pa_rirb: PhysicalAddress) {
        self.stop();
        self.set_base(pa_rirb);
        self.set_rintcnt(1);
        self.reset_write_pointer();
    }

    #[inline]
    pub fn start(&self) {
        self.set_control(self.get_control() | RirbControl::DMAEN | RirbControl::RINTCTL);
    }

    #[inline]
    pub fn stop(&self) {
        self.set_control(self.get_control() & !RirbControl::DMAEN);
    }

    #[inline]
    pub fn set_base(&self, base: PhysicalAddress) {
        let base = base.as_u64();
        self.lbase.write_volatile(base as u32);
        self.ubase.write_volatile((base >> 32) as u32);
    }

    #[inline]
    pub fn get_write_pointer(&self) -> usize {
        (self.wp.read_volatile() & 0xFF) as usize
    }

    #[inline]
    pub fn reset_write_pointer(&self) {
        self.wp.write_volatile(Self::RIRBWPRST);
    }

    #[inline]
    pub fn get_rintcnt(&self) -> usize {
        (self.rintcnt.read_volatile() & 0xFF) as usize
    }

    #[inline]
    pub fn set_rintcnt(&self, val: usize) {
        self.rintcnt.write_volatile((val & 0xFF) as u16);
    }

    #[inline]
    pub fn get_control(&self) -> RirbControl {
        RirbControl::from_bits_retain(self.ctl.read_volatile())
    }

    #[inline]
    pub fn set_control(&self, val: RirbControl) {
        self.ctl.write_volatile(val.bits());
    }

    #[inline]
    pub fn get_status(&self) -> RirbStatus {
        RirbStatus::from_bits_retain(self.sts.read_volatile())
    }

    #[inline]
    pub fn set_status(&self, val: RirbStatus) {
        self.sts.write_volatile(val.bits());
    }

    #[inline]
    pub fn entries(&self) -> Option<NonZeroUsize> {
        match self.size.read_volatile() & 3 {
            0 => NonZeroUsize::new(2),
            1 => NonZeroUsize::new(16),
            2 => NonZeroUsize::new(256),
            _ => None,
        }
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct ImmediateCommandRegisterSet {
    ico: MmioRegU32,
    ici: MmioRegU32,
    ics: MmioRegU16,
}

impl ImmediateCommandRegisterSet {
    #[inline]
    pub fn command(&self, cmd: Command) -> Result<Response> {
        let deadline = Timer::new(Duration::from_millis(HdAudioController::WAIT_DELAY_MS));
        let mut wait = Hal::cpu().spin_wait();
        while deadline.is_alive() && self.get_status().contains(ImmediateCommandStatus::ICB) {
            wait.wait();
        }
        if self.get_status().contains(ImmediateCommandStatus::ICB) {
            return Err(ControllerError::CommandBusy);
        }

        self.ico.write_volatile(cmd.bits());

        self.set_status(ImmediateCommandStatus::ICB);

        let deadline = Timer::new(Duration::from_millis(HdAudioController::WAIT_DELAY_MS));
        let mut wait = Hal::cpu().spin_wait();
        while deadline.is_alive() && !self.get_status().contains(ImmediateCommandStatus::IRV) {
            wait.wait();
        }
        if !self.get_status().contains(ImmediateCommandStatus::IRV) {
            return Err(ControllerError::CommandNotResponding);
        }

        let res = self.ici.read_volatile() as u64 | ((self.ici.read_volatile() as u64) << 32);

        self.set_status(ImmediateCommandStatus::IRV);

        Ok(Response(res))
    }

    #[inline]
    pub fn get_status(&self) -> ImmediateCommandStatus {
        unsafe { transmute(self.ics.read_volatile()) }
    }

    #[inline]
    pub fn set_status(&self, val: ImmediateCommandStatus) {
        self.ics.write_volatile(val.bits());
    }
}

my_bitflags! {
    pub struct ImmediateCommandStatus: u16 {
        /// Immediate Command Busy
        const ICB       = 0b0000_0000_0000_0001;
        /// Immediate Result Valid
        const IRV       = 0b0000_0000_0000_0010;
        /// Immediate Command Version
        const ICVER     = 0b0000_0000_0000_0100;
        /// Immediate Response Result Unsolicitied
        const IRRUNSOL  = 0b0000_0000_0000_1000;
        /// Immediate Response Result Address
        const IRRADD    = 0b0000_0000_0001_0000;
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct StreamDescriptorRegisterSet {
    ctl_lo: MmioRegU16,
    ctl_hi: MmioRegU8,
    sts: MmioRegU8,
    lpib: MmioRegU32,
    cbl: MmioRegU32,
    lvi: MmioRegU16,
    _rsrv_8e_8f: [u8; 2],
    fifos: MmioRegU16,
    fmt: MmioRegU16,
    _rsrv_94_97: [u8; 4],
    bdpl: MmioRegU32,
    bdpu: MmioRegU32,
}

impl StreamDescriptorRegisterSet {
    #[inline]
    pub fn get_control(&self) -> StreamDescriptorControl {
        StreamDescriptorControl::from_bits_retain(
            self.ctl_lo.read_volatile() as u32 | ((self.ctl_hi.read_volatile() as u32) << 16),
        )
    }

    #[inline]
    pub fn set_stream_id(&self, id: StreamId) {
        let mut data = self.get_control();
        data.set_stream_id(Some(id));
        self.set_control(data);
    }

    #[inline]
    pub fn set_control(&self, val: StreamDescriptorControl) {
        self.ctl_lo.write_volatile(val.bits() as u16);
        self.ctl_hi.write_volatile((val.bits() >> 16) as u8);
    }

    #[inline]
    pub fn get_status(&self) -> StreamDescriptorStatus {
        StreamDescriptorStatus::from_bits_retain(self.sts.read_volatile())
    }

    #[inline]
    pub fn set_status(&self, val: StreamDescriptorStatus) {
        self.sts.write_volatile(val.bits());
    }

    #[inline]
    pub fn clear_interrupts(&self) {
        self.set_status(StreamDescriptorStatus::CLEAR_INTERRUPTS)
    }

    #[inline]
    pub fn link_position(&self) -> usize {
        self.lpib.read_volatile() as usize
    }

    #[inline]
    pub fn get_buffer_length(&self) -> usize {
        self.cbl.read_volatile() as usize
    }

    #[inline]
    pub fn set_buffer_length(&self, val: usize) {
        self.cbl.write_volatile(val as u32);
    }

    #[inline]
    pub fn get_last_valid_index(&self) -> usize {
        self.lvi.read_volatile() as usize
    }

    #[inline]
    pub fn set_last_valid_index(&self, val: usize) {
        self.lvi.write_volatile(val as u16);
    }

    #[inline]
    pub fn fifo_size(&self) -> usize {
        self.fifos.read_volatile() as usize
    }

    #[inline]
    pub fn set_pcm_format(&self, fmt: PcmFormat) {
        self.fmt.write_volatile(fmt.bits());
    }

    #[inline]
    pub fn get_format(&self) -> PcmFormat {
        PcmFormat::from_bits(self.fmt.read_volatile())
    }

    #[inline]
    pub fn set_base(&self, base: PhysicalAddress) {
        let base = base.as_u64();
        self.bdpl.write_volatile(base as u32);
        self.bdpu.write_volatile((base >> 32) as u32);
    }
}

my_bitflags! {
    pub struct StreamDescriptorControl: u32 {
        /// Stream Reset
        const SRST      = 0x0000_0001;
        /// Stream Run
        const RUN       = 0x0000_0002;
        /// Interrupt On Completion Enable
        const IOCE      = 0x0000_0004;
        /// FIFO Error Interrupt Enable
        const FEIE      = 0x0000_0008;
        /// Descriptor Error Interrupt Enable
        const DEIE      = 0x0000_0010;

        const _STRIPE   = 0x0003_0000;

        /// Traffic Priority
        const TP        = 0x0004_0000;
        /// Bidirectional Direction Control
        const DIR       = 0x0008_0000;

        const _STREAM   = 0x00F0_0000;
    }
}

impl StreamDescriptorControl {
    #[inline]
    pub fn set_stream_id(&mut self, id: Option<StreamId>) {
        let val = id.map(|v| v.0.get()).unwrap_or(0);
        *self = Self::from_bits_retain(((val as u32 & 0x0F) << 20) | (self.bits() & !0xF0_0000));
    }

    #[inline]
    pub const fn get_stream_id(&self) -> Option<StreamId> {
        match NonZeroU8::new(((self.bits() >> 20) & 0x0F) as u8) {
            Some(v) => Some(StreamId(v)),
            None => None,
        }
    }
}

my_bitflags! {
    pub struct StreamDescriptorStatus: u8 {
        /// Buffer Completion Interrupt Status
        const BCIS      = 0x04;
        /// FIFO Error
        const FIFOE     = 0x08;
        /// Descriptor Error
        const DESE      = 0x10;
        /// FIFO Ready
        const FIFORDY   = 0x20;

        const CLEAR_INTERRUPTS = Self::BCIS.bits() | Self::FIFOE.bits() | Self::DESE.bits();
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct BufferDescriptor {
    address: u64,
    length: u32,
    flags: u32,
}

impl BufferDescriptor {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            address: 0,
            length: 0,
            flags: 0,
        }
    }

    #[inline]
    pub const fn new(address: PhysicalAddress, length: usize, ioc: bool) -> Self {
        let flags = if ioc {
            BufferDescriptorFlag::IOC
        } else {
            BufferDescriptorFlag::empty()
        };
        Self {
            address: address.as_u64(),
            length: length as u32,
            flags: flags.bits(),
        }
    }
}

my_bitflags! {
    pub struct BufferDescriptorFlag: u32 {
        const IOC   = 0x0000_0001;
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Command(u32);

impl Command {
    #[inline]
    pub const fn new(addr: WidgetAddress, verb: Verb) -> Self {
        Self(addr.as_corb() | verb.as_corb())
    }

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Response(u64);

impl Response {
    #[inline]
    pub const fn bits(&self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0 as u32
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum Verb {
    GetParameter(ParameterId),
    GetConnectionSelect,
    SetConnectionSelect(u8),
    GetConectionListEntry(u8),
    GetProcessingState,
    SetProcessingState(u8),

    GetPowerState,
    SetPowerState(u8),
    GetConverterControl,
    SetConverterControl(u8),
    GetPinWidgetControl,
    SetPinWidgetControl(PinWidgetControl),

    GetEapdBtlEnable,
    SetEapdBtlEnable(u8),

    GetConfigurationDefault,
    SetConfigurationDefault1(u8),
    SetConfigurationDefault2(u8),
    SetConfigurationDefault3(u8),
    SetConfigurationDefault4(u8),

    GetConverterFormat,
    SetConverterFormat(PcmFormat),
    GetAmplifierGainMute(AmplifierGainMuteGetPayload),
    SetAmplifierGainMute(AmplifierGainMuteSetPayload),
}

impl Verb {
    #[inline]
    pub const fn as_corb(&self) -> u32 {
        use Verb::*;
        self._id()
            | match *self {
                GetParameter(v) => v as u32,
                SetConnectionSelect(v) => v as u32,
                GetConectionListEntry(v) => v as u32,
                SetProcessingState(v) => v as u32,

                SetPowerState(v) => v as u32,
                SetConverterControl(v) => v as u32,
                SetPinWidgetControl(v) => v.bits() as u32,

                SetEapdBtlEnable(v) => v as u32,

                SetConfigurationDefault1(v) => v as u32,
                SetConfigurationDefault2(v) => v as u32,
                SetConfigurationDefault3(v) => v as u32,
                SetConfigurationDefault4(v) => v as u32,

                SetConverterFormat(v) => v.bits() as u32,
                GetAmplifierGainMute(v) => v.bits() as u32,
                SetAmplifierGainMute(v) => v.bits() as u32,

                _ => 0,
            }
    }

    #[inline]
    const fn _id(&self) -> u32 {
        use Verb::*;
        match *self {
            GetParameter(_) => 0xF_00_00,
            GetConnectionSelect => 0xF_01_00,
            SetConnectionSelect(_) => 0x7_01_00,
            GetConectionListEntry(_) => 0xF_02_00,
            GetProcessingState => 0xF_03_00,
            SetProcessingState(_) => 0x7_03_00,

            GetPowerState => 0xF_05_00,
            SetPowerState(_) => 0x7_05_00,
            GetConverterControl => 0xF_06_00,
            SetConverterControl(_) => 0x7_06_00,
            GetPinWidgetControl => 0xF_07_00,
            SetPinWidgetControl(_) => 0x7_07_00,

            GetEapdBtlEnable => 0xF_0C_00,
            SetEapdBtlEnable(_) => 0x7_0C_00,

            GetConfigurationDefault => 0xF_1C_00,
            SetConfigurationDefault1(_) => 0x7_1C_00,
            SetConfigurationDefault2(_) => 0x7_1D_00,
            SetConfigurationDefault3(_) => 0x7_1E_00,
            SetConfigurationDefault4(_) => 0x7_1F_00,

            GetConverterFormat => 0xA_0000,
            SetConverterFormat(_) => 0x2_0000,
            GetAmplifierGainMute(_) => 0xB_0000,
            SetAmplifierGainMute(_) => 0x3_0000,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ParameterId {
    VendorId = 0x00,
    RevisionId = 0x02,
    SubordinateNodeCount = 0x04,
    FunctionGroupType = 0x05,
    AudioFunctionGroupCapabilities = 0x08,
    AudioWidgetCapabilities = 0x09,
    SampleSizeRateCaps = 0x0A,
    StreamFormats = 0x0B,
    PinCapabilities = 0x0C,
    InputAmpCapabilities = 0x0D,
    OutputAmpCapabilities = 0x12,
    ConnectionListLength = 0x0E,
    SupportedPowerStates = 0x0F,
    ProcessingCapabilities = 0x10,
    GpIoCount = 0x11,
    VolumeKnobCapabilities = 0x13,
}

my_bitflags! {
    pub struct AudioWidgetCapabilities: u32 {
        const STEREO            = 0x0000_0001;
        const IN_AMP            = 0x0000_0002;
        const OUT_AMP           = 0x0000_0004;
        const AMP_OVERRIDE      = 0x0000_0008;
        const FORMAT_OVERRIDE   = 0x0000_0010;
        const STRIP             = 0x0000_0020;
        const PROC_WIDGET       = 0x0000_0040;
        const UNSOL_CAPABLE     = 0x0000_0080;
        const CONN_LIST         = 0x0000_0100;
        const DIGITAL           = 0x0000_0200;
        const POWER_CNTRL       = 0x0000_0400;
        const L_R_SWAP          = 0x0000_0800;
        const CP_CAPS           = 0x0000_1000;
        const CHAN_EX_MASK      = 0x0000_E000;
        const DELAY_MASK        = 0x000F_0000;
        const TYPE_MASK         = 0x00F0_0000;
    }
}

impl AudioWidgetCapabilities {
    #[inline]
    pub const fn widget_type(&self) -> WidgetType {
        unsafe { transmute(((self.bits() & Self::TYPE_MASK.bits()) >> 20) as u8) }
    }

    #[inline]
    pub const fn number_of_channels(&self) -> usize {
        1 + ((self.bits() & Self::STEREO.bits()) | (self.bits() & Self::CHAN_EX_MASK.bits()) >> 12)
            as usize
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[allow(dead_code)]
#[non_exhaustive]
pub enum WidgetType {
    AudioOutput = 0x00,
    Audioinput = 0x01,
    AudioMixer = 0x02,
    AudioSelector = 0x03,
    PinComplex = 0x04,
    PowerWidget = 0x05,
    VolumeKnobWidget = 0x06,
    BeepGenerator = 0x07,
    VendorDefined = 0x0F,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PinWidgetControl(u8);

impl PinWidgetControl {
    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ConfigurationDefault(u32);

impl ConfigurationDefault {
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn new(val: u32) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn color(&self) -> Color {
        unsafe { transmute(((self.0 >> 12) & 0xF) as u8) }
    }

    #[inline]
    pub const fn default_device(&self) -> DefaultDevice {
        unsafe { transmute(((self.0 >> 20) & 0xF) as u8) }
    }

    #[inline]
    pub const fn port_connectivity(&self) -> PortConnectivity {
        unsafe { transmute(((self.0 >> 30) & 0x3) as u8) }
    }

    #[inline]
    pub const fn gross_location(&self) -> GrossLocation {
        unsafe { transmute(((self.0 >> 28) & 0x3) as u8) }
    }

    #[inline]
    pub const fn geometric_location(&self) -> GeometricLocation {
        unsafe { transmute(((self.0 >> 24) & 0x7) as u8) }
    }

    #[inline]
    pub const fn is_output(&self) -> bool {
        match self.default_device() {
            DefaultDevice::LineOut
            | DefaultDevice::Speaker
            | DefaultDevice::HPOut
            | DefaultDevice::CD
            | DefaultDevice::SPDIF
            | DefaultDevice::DigitalOtherOut
            | DefaultDevice::ModemLineSide => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_input(&self) -> bool {
        match self.default_device() {
            DefaultDevice::ModemHandsetSide
            | DefaultDevice::LineIn
            | DefaultDevice::AUX
            | DefaultDevice::MicIn
            | DefaultDevice::Telephony
            | DefaultDevice::SPDIFIn
            | DefaultDevice::DigitalOtherIn => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn sequence(&self) -> u8 {
        (self.0 & 0xF) as u8
    }

    #[inline]
    pub const fn default_association(&self) -> u8 {
        ((self.0 >> 4) & 0xF) as u8
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefaultDevice {
    LineOut = 0x0,
    Speaker = 0x1,
    HPOut = 0x2,
    CD = 0x3,
    SPDIF = 0x4,
    DigitalOtherOut = 0x5,
    ModemLineSide = 0x6,
    ModemHandsetSide = 0x7,
    LineIn = 0x8,
    AUX = 0x9,
    MicIn = 0xA,
    Telephony = 0xB,
    SPDIFIn = 0xC,
    DigitalOtherIn = 0xD,
    _Reserved = 0xE,
    Other = 0xF,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PortConnectivity {
    ConnectedToJack = 0x0,
    NoPhysicalConnection = 0x1,
    FixedFunction = 0x2,
    JackAndInternal = 0x3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GrossLocation {
    ExternalOnPrimary = 0x0,
    Internal = 0x1,
    SeperateChasis = 0x2,
    Other = 0x3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometricLocation {
    NA = 0x0,
    Rear = 0x1,
    Front = 0x2,
    Left = 0x3,
    Right = 0x4,
    Top = 0x5,
    Bottom = 0x6,
    Special1 = 0x7,
    Special2 = 0x8,
    Special3 = 0x9,
    _Resvd1 = 0xA,
    _Resvd2 = 0xB,
    _Resvd3 = 0xC,
    _Resvd4 = 0xD,
    _Resvd5 = 0xE,
    _Resvd6 = 0xF,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Color {
    Unknown = 0x0,
    Black = 0x1,
    Grey = 0x2,
    Blue = 0x3,
    Green = 0x4,
    Red = 0x5,
    Orange = 0x6,
    Yellow = 0x7,
    Purple = 0x8,
    Pink = 0x9,
    _Resvd1 = 0xA,
    _Resvd2 = 0xB,
    _Resvd3 = 0xC,
    _Resvd4 = 0xD,
    White = 0xE,
    Other = 0xF,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PcmFormat(pub u16);

impl PcmFormat {
    #[inline]
    pub const fn from_bits(val: u16) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn bits(&self) -> u16 {
        self.0
    }

    #[inline]
    pub const fn new(sample_rate: SampleRate, bps: Bits, channels: usize) -> Self {
        Self(((channels - 1) & 0xF) as u16 | ((bps as u16) << 4) | ((sample_rate as u16) << 8))
    }

    #[inline]
    pub const fn channels(&self) -> usize {
        (self.0 & 0xF) as usize + 1
    }

    #[inline]
    pub const fn bps(&self) -> Bits {
        unsafe { transmute((self.0 >> 4) as u8 & 0x07) }
    }

    #[inline]
    pub const fn sample_rate(&self) -> SampleRate {
        unsafe { transmute((self.0 >> 8) as u8 & 0x7F) }
    }
}

impl Default for PcmFormat {
    /// Default sample rate, same quality as CD-DA.
    fn default() -> Self {
        Self::new(SampleRate::_44K1, Bits::B16, 2)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Bits {
    B8 = 0b000,
    B16 = 0b001,
    B20 = 0b010,
    B24 = 0b011,
    B32 = 0b100,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum SampleRate {
    /// 8.0KHz 1/6 * 48
    _8K = 0b0_000_101,
    /// 11.025KHz 1/4 * 44.1
    _11K025 = 0b1_000_011,
    /// 16.0KHz 1/3 * 48
    _16K = 0b0_000_010,
    /// 22.05KHz 1/2 * 44.1
    _22K05 = 0b1_000_001,
    /// 32.0KHz 2/3 * 48
    _32K = 0b0_001_010,
    /// 44.1KHz 44.1
    _44K1 = 0b1_000_000,
    /// 48KHz 48 (Must be supported by all codecs)
    _48K = 0b0_000_000,
    /// 88.2KHz 2/1 * 44.1
    _88K2 = 0b1_001_000,
    /// 96KHz 2/1 * 48
    _96K = 0b0_001_000,
    /// 176.4KHz 4/1 * 44.1
    _176K4 = 0b1_011_000,
    /// 192KHz 4/1 * 48
    _192K = 0b0_011_000,
}

impl SampleRate {
    #[inline]
    pub const fn hertz(&self) -> usize {
        match *self {
            SampleRate::_8K => 8_000,
            SampleRate::_11K025 => 11_025,
            SampleRate::_16K => 16_000,
            SampleRate::_22K05 => 22_050,
            SampleRate::_32K => 32_000,
            SampleRate::_44K1 => 44_100,
            SampleRate::_48K => 48_000,
            SampleRate::_88K2 => 88_200,
            SampleRate::_96K => 96_000,
            SampleRate::_176K4 => 176_400,
            SampleRate::_192K => 192_000,
        }
    }
}

my_bitflags! {
    pub struct SupportedPCMFormat: u32 {
        /// 8.0KHz 1/6 * 48
        const _8K       = 0x0000_0001;
        /// 11.025KHz 1/4 * 44.1
        const _11K025   = 0x0000_0002;
        /// 16.0KHz 1/3 * 48
        const _16K      = 0x0000_0004;
        /// 22.05KHz 1/2 * 44.1
        const _22K05    = 0x0000_0008;
        /// 32.0KHz 2/3 * 48
        const _32K      = 0x0000_0010;
        /// 44.1KHz 44.1
        const _44K1     = 0x0000_0020;
        /// 48KHz 48 (Must be supported by all codecs)
        const _48K      = 0x0000_0040;
        /// 88.2KHz 2/1 * 44.1
        const _88K2     = 0x0000_0080;
        /// 96KHz 2/1 * 48
        const _96K      = 0x0000_0100;
        /// 176.4KHz 4/1 * 44.1
        const _176K4    = 0x0000_0200;
        /// 192KHz 4/1 * 48
        const _192K     = 0x0000_0400;
        /// 384KHz 8/1 * 48
        const _384K     = 0x0000_0800;

        /// 8bit
        const B8        = 0x0001_0000;
        /// 16bit
        const B16       = 0x0002_0000;
        /// 20bit
        const B20       = 0x0004_0000;
        /// 24bit
        const B24       = 0x0008_0000;
        /// 32bit
        const B32       = 0x0010_0000;

    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct AmplifierGainMuteGetPayload(u16);

impl AmplifierGainMuteGetPayload {
    #[inline]
    pub const fn new(output: bool, left: bool, index: u8) -> Self {
        Self(((output as u16) << 15) | ((left as u16) << 13) | (index as u16 & 0x0F))
    }

    #[inline]
    pub const fn bits(&self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct AmplifierGainMuteSetPayload(u16);

impl AmplifierGainMuteSetPayload {
    #[inline]
    pub const fn new(
        output: bool,
        input: bool,
        left: bool,
        right: bool,
        index: u8,
        mute: bool,
        gain: usize,
    ) -> Self {
        Self(
            ((output as u16) << 15)
                | ((input as u16) << 14)
                | ((left as u16) << 13)
                | ((right as u16) << 12)
                | ((index as u16 & 0x0F) << 8)
                | ((mute as u16) << 7)
                | (gain as u16 & 0x7F),
        )
    }

    #[inline]
    pub const fn bits(&self) -> u16 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct AmplifierCapabilities(u32);

impl AmplifierCapabilities {
    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn offset(&self) -> usize {
        self.0 as usize & 0x7F
    }

    #[inline]
    pub const fn num_steps(&self) -> usize {
        1 + ((self.0 << 8) as usize & 0x7F)
    }

    #[inline]
    pub const fn step_size(&self) -> usize {
        1 + ((self.0 << 16) as usize & 0x7F)
    }

    #[inline]
    pub const fn mute_supported(&self) -> bool {
        (self.0 & 0x8000_0000) != 0
    }
}
