use super::*;
use crate::drivers::{pci::*, usb::*};
use crate::mem::mmio::*;
use crate::mem::MemoryManager;
use crate::sync::{fifo::AsyncEventQueue, semaphore::*, RwLock};
use crate::task::{scheduler::*, Task};
use crate::*;
use alloc::collections::VecDeque;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::{size_of, transmute, MaybeUninit};
use core::num::NonZeroU8;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::slice;
use core::sync::atomic::*;
use core::task::Poll;
use core::time::Duration;
use futures_util::Future;
use megstd::mem::dispose::*;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub struct XhciRegistrar();

impl XhciRegistrar {
    const PREFERRED_CLASS: PciClass = PciClass::code(0x0C).sub(0x03).interface(0x30);

    pub fn new() -> Box<dyn PciDriverRegistrar> {
        Box::new(Self()) as Box<dyn PciDriverRegistrar>
    }
}

impl PciDriverRegistrar for XhciRegistrar {
    fn instantiate(&self, device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        if device.class_code().matches(Self::PREFERRED_CLASS) {
            unsafe { Xhci::new(device) }
        } else {
            None
        }
    }
}

/// Extensible Host Controller Interface
///
/// Many methods are made public for documentation purposes, but are not intended to be called from the outside.
pub struct Xhci {
    addr: PciConfigAddress,
    #[allow(dead_code)]
    mmio: MmioSlice,

    cap: &'static CapabilityRegisters,
    opr: &'static OperationalRegisters,
    ports: &'static [PortRegisters],
    doorbells: &'static [DoorbellRegister],
    rts: &'static RuntimeRegisters,

    roothub_usb2_off: AtomicU8,
    roothub_usb2_cnt: AtomicU8,
    roothub_usb3_off: AtomicU8,
    roothub_usb3_cnt: AtomicU8,

    max_device_slots: usize,
    dcbaa_len: usize,
    context_size: usize,
    ers: PhysicalAddress,

    ring_context: RwLock<[MaybeUninit<EpRingContext>; Self::MAX_TR]>,
    event_cycle: CycleBit,
    port_status_change_queue: AsyncEventQueue<PortId>,
    port2slot: [AtomicU8; 256],
    slot2port: [AtomicU8; 256],
    crbs: [UnsafeCell<CommandRequestBlock>; Self::MAX_CRB],
    ics: [UnsafeCell<InputContext>; Self::MAX_DEVICE_SLOTS],
    doorbell_queue: AsyncEventQueue<QueuedDoorbell>,

    sem_event_thread: Semaphore,
}

unsafe impl Send for Xhci {}
unsafe impl Sync for Xhci {}

impl Xhci {
    const DRIVER_NAME: &'static str = "xhci";

    /// The maximum number of device slots allowed.
    /// This means the maximum number of USB devices that can be connected to this controller.
    const MAX_DEVICE_SLOTS: usize = 64;
    const MAX_DOORBELLS: usize = Self::MAX_DEVICE_SLOTS * 32;
    const MAX_TR: usize = 256;
    const MAX_CRB: usize = 256;
    const SIZE_EP_RING: usize = MemoryManager::PAGE_SIZE_MIN / size_of::<Trb>();
    const MAX_TR_INDEX: usize = Self::SIZE_EP_RING - 1;
    const MAX_PORT_CHANGE: usize = 64;

    #[inline]
    pub fn registrar() -> Box<dyn PciDriverRegistrar> {
        XhciRegistrar::new()
    }

    unsafe fn new(device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        let bar = match device.bars().next() {
            Some(v) => v,
            None => return None,
        };
        let mmio = match MmioSlice::from_bar(bar) {
            Some(v) => v,
            None => return None,
        };

        let cap = mmio.transmute::<CapabilityRegisters>(0);
        let opr = cap.opr();
        let ports = cap.ports();
        let doorbells = cap.doorbells();
        let rts = cap.rts();

        let max_device_slots = usize::min(Self::MAX_DEVICE_SLOTS, cap.max_device_slots());
        let dcbaa_len = 1 + max_device_slots;
        let hcc_params1 = cap.hcc_params1();
        let context_size = if hcc_params1.contains(HccParams1::CSZ) {
            64
        } else {
            32
        };

        let ers =
            MemoryManager::alloc_pages(InterrupterRegisterSet::SIZE_EVENT_RING * size_of::<Trb>())
                .unwrap()
                .get() as PhysicalAddress;

        if false {
            log!(
                "XHCI {}.{}.{} PORTS {} SLOTS {} CTX {} INT {}",
                cap.version().0,
                cap.version().1,
                cap.version().2,
                cap.max_ports(),
                cap.max_device_slots(),
                context_size,
                cap.max_interrups(),
            );
        }

        let driver = Arc::new(Self {
            addr: device.address(),
            mmio,
            cap,
            opr,
            ports,
            doorbells,
            rts,
            roothub_usb2_cnt: AtomicU8::new(0),
            roothub_usb2_off: AtomicU8::new(0),
            roothub_usb3_cnt: AtomicU8::new(0),
            roothub_usb3_off: AtomicU8::new(0),
            max_device_slots,
            dcbaa_len,
            context_size,
            ring_context: RwLock::new(MaybeUninit::uninit_array()),
            event_cycle: CycleBit::from(true),
            ers,
            port_status_change_queue: AsyncEventQueue::new(Self::MAX_PORT_CHANGE),
            port2slot: transmute([0u8; 256]),
            slot2port: transmute([0u8; 256]),
            crbs: [CommandRequestBlock::EMPTY; Self::MAX_CRB],
            ics: [InputContext::EMPTY; Self::MAX_DEVICE_SLOTS],
            doorbell_queue: AsyncEventQueue::new(Self::MAX_DOORBELLS),
            sem_event_thread: Semaphore::new(0),
        });

        for ctx in driver.ring_context.write().unwrap().iter_mut() {
            *ctx = MaybeUninit::zeroed();
        }

        driver.clone().initialize(device);

        let p = driver.clone();
        SpawnOption::with_priority(Priority::Realtime).spawn(
            move || {
                p._event_thread();
            },
            Self::DRIVER_NAME,
        );

        UsbManager::register_xfer_task(Task::new(driver.clone()._schedule_task()));
        UsbManager::register_xfer_task(Task::new(driver.clone()._root_hub_task()));

        Some(driver as Arc<dyn PciDriver>)
    }

    ///  xHCI Initialize
    unsafe fn initialize(self: Arc<Self>, pci: &PciDevice) {
        if let Some(xecp) = self.cap.xecp() {
            let mut xecp_base = xecp.get() as *mut u32;
            loop {
                let xecp = xecp_base.read_volatile();
                // log!("XECP {:02x} {:02x}", xecp & 0xFF, (xecp >> 8) & 0xFF);
                match xecp & 0xFF {
                    0x01 => {
                        // USB Legacy Support
                        const USBLEGSUP_BIOS_OWNED: u32 = 0x0001_0000;
                        const USBLEGSUP_OS_OWNED: u32 = 0x0100_0000;
                        let usb_leg_sup = xecp_base;
                        let usb_leg_ctl_sts = xecp_base.add(1);
                        // log!(
                        //     "USB leg_sup {:08x} {:08x}",
                        //     usb_leg_sup.read_volatile(),
                        //     usb_leg_ctl_sts.read_volatile()
                        // );

                        // Hand over ownership from BIOS to OS
                        usb_leg_sup.write_volatile(xecp | USBLEGSUP_OS_OWNED);

                        if (usb_leg_sup.read_volatile() & USBLEGSUP_BIOS_OWNED) != 0 {
                            for _ in 0..100 {
                                if (usb_leg_sup.read_volatile() & USBLEGSUP_BIOS_OWNED) == 0 {
                                    break;
                                }
                                Timer::sleep(Duration::from_millis(10));
                            }
                            // Force BIOS ownership to be disabled.
                            usb_leg_sup.write_volatile(
                                usb_leg_sup.read_volatile() & !USBLEGSUP_BIOS_OWNED,
                            );
                        }

                        // Adjusting SMI settings
                        usb_leg_ctl_sts.write_volatile(
                            (usb_leg_ctl_sts.read_volatile() & 0x000E_1FEE) | 0xE000_0000,
                        );
                    }
                    0x02 => {
                        // Supported Protocol
                        let ecap = XhciSupportedProtocolCapability(xecp_base);
                        // let psic = (xecp_base.add(2).read_volatile() >> 28) as usize;

                        // let n = ecap.name();
                        // let s = unsafe {
                        //     core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        //         n.as_ptr(),
                        //         4,
                        //     ))
                        // };
                        // log!(
                        //     "XHCI_ECAP: {} {}.{:02x} {:2},{:2}",
                        //     s,
                        //     ecap.rev_major(),
                        //     ecap.rev_minor(),
                        //     ecap.compatible_port_offset(),
                        //     ecap.compatible_port_count()
                        // );
                        // for i in 0..psic {
                        //     let psi_base = xecp_base.add(4 + i);
                        //     let psi = psi_base.read_volatile();
                        //     log!("PSI {:08x}", psi);
                        // }

                        match (ecap.name(), ecap.rev_major(), ecap.rev_minor()) {
                            (XhciSupportedProtocolCapability::NAME_USB, 2, 0) => {
                                self.roothub_usb2_off
                                    .store(ecap.compatible_port_offset(), Ordering::SeqCst);
                                self.roothub_usb2_cnt
                                    .store(ecap.compatible_port_count(), Ordering::SeqCst);
                            }
                            (XhciSupportedProtocolCapability::NAME_USB, 3, _) => {
                                self.roothub_usb3_off
                                    .store(ecap.compatible_port_offset(), Ordering::SeqCst);
                                self.roothub_usb3_cnt
                                    .store(ecap.compatible_port_count(), Ordering::SeqCst);
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
                match ((xecp >> 8) & 0xFF) as usize {
                    0 => break,
                    xecp_ptr => {
                        xecp_base = xecp_base.add(xecp_ptr);
                    }
                }
            }
        }

        // reset xHC
        self.opr.write_cmd(UsbCmd::HCRST);
        // The xHC shall halt within 16ms.
        Timer::sleep(Duration::from_millis(20));
        while self.opr.read_cmd().contains(UsbCmd::HCRST) || self.opr.status().contains(UsbSts::CNR)
        {
            Timer::sleep(Duration::from_millis(10));
        }

        self.opr.set_config(self.max_device_slots, false, false);

        // make Device Context Base Address Array
        let dcbaa_size = self.dcbaa_len * 8;
        let pa_dcbaa = MemoryManager::alloc_pages(dcbaa_size).unwrap().get();
        self.opr
            .set_dcbaap(NonNullPhysicalAddress::new(pa_dcbaa).unwrap());

        // make Scratchpad
        let max_scratchpad_size = self.cap.max_scratchpad_size();
        if max_scratchpad_size > 0 {
            let array_size = max_scratchpad_size * 8;
            let sp_array = MemoryManager::alloc_pages(array_size).unwrap().get();
            let sp_size = max_scratchpad_size * self.opr.page_size();
            let scratchpad = MemoryManager::alloc_pages(sp_size).unwrap().get();
            let spava = sp_array.direct_map::<u64>();
            for i in 0..max_scratchpad_size {
                spava
                    .add(i)
                    .write_volatile(scratchpad.as_u64() + (i * self.opr.page_size()) as u64);
            }
            self.dcbaa()[0] = sp_array;
        }

        // Command Ring Control Register
        self.opr.set_crcr(self.alloc_ep_ring(None, None).unwrap());

        // Event Ring Segment Table
        self.rts
            .primary_irs()
            .init(self.ers, InterrupterRegisterSet::SIZE_EVENT_RING);

        // Interrupt
        self.rts.primary_irs().set_iman(3);
        self.opr.set_cmd(UsbCmd::INTE);
        let p = Arc::as_ptr(&self);
        Arc::increment_strong_count(p);
        pci.register_msi(Self::_msi_handler, p as usize).unwrap();

        // self.opr
        //     .set_device_notification_bitmap(DeviceNotificationBitmap::FUNCTION_WAKE);

        // start xHC
        self.wait_cnr(0);
        self.opr.set_cmd(UsbCmd::RUN);
        while self.opr.status().contains(UsbSts::HCH) {
            Timer::sleep(Duration::from_millis(10));
        }
    }

    fn dcbaa(&self) -> &'static mut [PhysicalAddress] {
        unsafe { slice::from_raw_parts_mut((self.opr.dcbaap() & !63).direct_map(), self.dcbaa_len) }
    }

    fn _msi_handler(p: usize) {
        let this = unsafe { &*(p as *const Self) };
        this.sem_event_thread.signal();
    }

    /// xHCI Main event loop
    fn _event_thread(self: Arc<Self>) {
        loop {
            self.sem_event_thread.wait();
            self.process_event();
        }
    }

    pub fn get_device_context(&self, slot_id: SlotId) -> PhysicalAddress {
        *self.dcbaa().get(slot_id.0.get() as usize).unwrap()
    }

    pub fn set_device_context(&self, slot_id: SlotId, value: PhysicalAddress) {
        *self.dcbaa().get_mut(slot_id.0.get() as usize).unwrap() = value;
    }

    pub fn ring_a_doorbell(&self, slot_id: Option<SlotId>, dci: Option<DCI>) {
        self.wait_cnr(0);
        fence(Ordering::SeqCst);
        self.doorbells[slot_id.map(|v| v.0.get() as usize).unwrap_or_default()].set_target(dci);
    }

    pub fn ring_a_doorbell_async(
        &self,
        parent_slot_id: Option<SlotId>,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
    ) -> Result<(), ()> {
        let doorbell = QueuedDoorbell::Doorbell(parent_slot_id, slot_id, dci);
        self.doorbell_queue.post(doorbell).map_err(|_| ())
    }

    #[inline]
    pub fn port_by_slot(&self, slot_id: SlotId) -> Option<PortId> {
        unsafe { transmute(self.slot2port[slot_id.0.get() as usize].load(Ordering::Relaxed)) }
    }

    #[inline]
    pub fn slot_by_port(&self, port_id: PortId) -> Option<SlotId> {
        unsafe { transmute(self.port2slot[port_id.0.get() as usize].load(Ordering::Relaxed)) }
    }

    #[inline]
    pub fn port_by(&self, port_id: PortId) -> &PortRegisters {
        self.ports.get(port_id.0.get() as usize - 1).unwrap()
    }

    #[inline]
    pub fn port_is_usb2(&self, port_id: PortId) -> bool {
        let port = port_id.0.get();
        let offset = self.roothub_usb2_off.load(Ordering::Relaxed);
        let count = self.roothub_usb2_cnt.load(Ordering::Relaxed);
        port >= offset && port < offset + count
    }

    #[inline]
    pub fn port_is_usb3(&self, port_id: PortId) -> bool {
        let port = port_id.0.get();
        let offset = self.roothub_usb3_off.load(Ordering::Relaxed);
        let count = self.roothub_usb3_cnt.load(Ordering::Relaxed);
        port >= offset && port < offset + count
    }

    #[inline]
    pub fn ports<'a>(&self) -> impl Iterator<Item = (PortId, &'a PortRegisters)> {
        self.ports.iter().enumerate().map(|(index, port)| {
            (
                PortId(unsafe { NonZeroU8::new_unchecked(index as u8 + 1) }),
                port,
            )
        })
    }

    #[inline]
    pub fn usb2_ports<'a>(&self) -> impl Iterator<Item = (PortId, &'a PortRegisters)> {
        let offset = self.roothub_usb2_off.load(Ordering::Relaxed);
        let count = self.roothub_usb2_cnt.load(Ordering::Relaxed);
        self.ports
            .iter()
            .enumerate()
            .skip(offset as usize - 1)
            .take(count as usize)
            .map(|(index, port)| {
                (
                    PortId(unsafe { NonZeroU8::new_unchecked(index as u8 + 1) }),
                    port,
                )
            })
    }

    #[inline]
    pub fn usb3_ports<'a>(&self) -> impl Iterator<Item = (PortId, &'a PortRegisters)> {
        let offset = self.roothub_usb3_off.load(Ordering::Relaxed);
        let count = self.roothub_usb3_cnt.load(Ordering::Relaxed);
        self.ports
            .iter()
            .enumerate()
            .skip(offset as usize - 1)
            .take(count as usize)
            .map(|(index, port)| {
                (
                    PortId(unsafe { NonZeroU8::new_unchecked(index as u8 + 1) }),
                    port,
                )
            })
    }

    pub fn input_context<'a>(&self, slot_id: SlotId) -> &'a mut InputContext {
        self.ics
            .get(slot_id.0.get() as usize)
            .map(|v| unsafe { &mut *v.get() })
            .unwrap()
    }

    /// wait for CNR (Controller Not Ready)
    #[inline(never)]
    pub fn wait_cnr(&self, _: usize) {
        while self.opr.status().contains(UsbSts::CNR) {}
    }

    pub fn ep_ring_index(&self, slot_id: Option<SlotId>, dci: Option<DCI>) -> Option<usize> {
        let slot_id = slot_id.map(|v| v.0.get()).unwrap_or_default();
        let dci = dci.map(|v| v.0.get()).unwrap_or_default();
        for (index, ctx) in self.ring_context.read().unwrap().iter().enumerate() {
            let ctx = unsafe { &*ctx.as_ptr() };
            if ctx.tr_base().is_some()
                && ctx.slot_id().map(|v| v.0.get()).unwrap_or_default() == slot_id
                && ctx.dci().map(|v| v.0.get()).unwrap_or_default() == dci
            {
                return Some(index);
            }
        }
        None
    }

    pub fn alloc_ep_ring(
        &self,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
    ) -> Option<NonNullPhysicalAddress> {
        if let Some(index) = self.ep_ring_index(slot_id, dci) {
            let ctx = &mut self.ring_context.write().unwrap()[index];
            let ctx = unsafe { &mut *ctx.as_mut_ptr() };
            ctx.clear();
            return ctx.tr_value();
        }
        for ctx in self.ring_context.write().unwrap().iter_mut() {
            let ctx = unsafe { &mut *ctx.as_mut_ptr() };
            if ctx.tr_base().is_none() {
                unsafe {
                    ctx.alloc(slot_id, dci);
                }
                return ctx.tr_value();
            }
        }
        None
    }

    pub fn allocate_crb<'a>(&'a self) -> Option<&'a mut CommandRequestBlock> {
        for crb in &self.crbs {
            let crb = unsafe { &mut *crb.get() };
            if crb.try_to_acquire() {
                return Some(crb);
            }
        }
        None
    }

    pub fn find_crb<'a>(
        &'a self,
        scheduled_trb: ScheduledTrb,
        state: Option<RequestState>,
    ) -> Option<&'a mut CommandRequestBlock> {
        for crb in &self.crbs {
            let crb = unsafe { &mut *crb.get() };
            let crb_state = crb.state();
            if crb_state != RequestState::Available && crb.scheduled_trb == scheduled_trb {
                match state {
                    Some(state) => {
                        if crb_state == state {
                            return Some(crb);
                        }
                    }
                    None => return Some(crb),
                }
            }
        }
        None
    }

    pub fn issue_trb(
        &self,
        crb: Option<&mut CommandRequestBlock>,
        trb: &Trb,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
    ) {
        let index = match self.ep_ring_index(slot_id, dci) {
            Some(index) => index,
            None => todo!(),
        };
        let ctx = &mut self.ring_context.write().unwrap()[index];
        let ctx = unsafe { &mut *ctx.as_mut_ptr() };

        if trb.trb_type() == Some(TrbType::SETUP) {
            ctx.last_setup.raw_copy(trb);
        }

        let tr_base = ctx.tr_base().unwrap().get();
        let tr = tr_base.direct_map::<Trb>();
        let mut index = ctx.index;

        let scheduled_trb = ScheduledTrb(tr_base + (size_of::<Trb>() * index) as u64);
        if let Some(crb) = crb {
            crb.schedule(trb, scheduled_trb);
        }

        if index == Xhci::MAX_TR_INDEX {
            let trb_link = TrbLink::new(tr_base, true);
            unsafe {
                (&*tr.add(index)).copy(&trb_link, ctx.pcs());
            }
            index = 0;
            ctx.pcs().toggle();
        }
        unsafe {
            (&*tr.add(index)).copy(trb, ctx.pcs());
        }
        index += 1;
        ctx.index = index;
    }

    /// Issue trb command
    pub fn execute_command(
        &self,
        trb: &Trb,
    ) -> Result<TrbCommandCompletionEvent, TrbCommandCompletionEvent> {
        // log!(
        //     "EXEC {:?} {:08x} {:08x} {:08x} {:08x}",
        //     trb.trb_type(),
        //     trb.raw_data()[0].load(Ordering::Relaxed),
        //     trb.raw_data()[1].load(Ordering::Relaxed),
        //     trb.raw_data()[2].load(Ordering::Relaxed),
        //     trb.raw_data()[3].load(Ordering::Relaxed),
        // );

        let mut crb = DisposableRef::new(self.allocate_crb().unwrap());
        self.issue_trb(Some(crb.as_mut()), trb, None, None);
        self.ring_a_doorbell(None, None);
        crb.wait();
        fence(Ordering::SeqCst);
        let result = match crb.response.as_event() {
            Some(TrbEvent::CommandCompletion(v)) => Some(v.copied()),
            _ => None,
        };
        drop(crb);
        match result {
            Some(result) => {
                if result.completion_code() == Some(TrbCompletionCode::SUCCESS) {
                    Ok(result)
                } else {
                    Err(result)
                }
            }
            None => Err(TrbCommandCompletionEvent::empty()),
        }
    }

    pub async unsafe fn control_async(
        device: Arc<HciDeviceContext>,
        setup: UsbControlSetupData,
        transfer_mode: TransferDirection<u8>,
    ) -> Result<UsbLength, UsbError> {
        let host = device.host();
        let slot_id = Some(device.slot_id);
        let dci = Some(DCI::CONTROL);
        let ctx = match host.ep_ring_index(slot_id, dci) {
            Some(index) => {
                let ctx = &mut host.ring_context.write().unwrap()[index];
                (&mut *ctx.as_mut_ptr()).scoped().await
            }
            None => todo!(),
        };
        ctx.set_scheduled();

        let len = setup.wLength.as_usize();
        let trt = if len > 0 {
            if setup.bmRequestType.is_device_to_host() {
                TrbTranfserType::ControlIn
            } else {
                TrbTranfserType::ControlOut
            }
        } else {
            TrbTranfserType::NoData
        };
        let dir = trt == TrbTranfserType::ControlIn;

        let buffer = ctx.buffer();
        if let TransferDirection::Write(p) = transfer_mode {
            buffer.direct_map::<u8>().copy_from_nonoverlapping(p, len);
        }

        let setup_trb = TrbSetupStage::new(trt, setup);
        host.issue_trb(None, setup_trb.as_trb(), slot_id, dci);

        if len > 0 {
            let data_trb = TrbDataStage::new(*buffer, len, dir, false);
            host.issue_trb(None, data_trb.as_trb(), slot_id, dci);
        }

        let status_trb = TrbStatusStage::new(!dir);
        host.issue_trb(None, status_trb.as_trb(), slot_id, dci);

        host.ring_a_doorbell_async(device.parent_slot_id, slot_id, dci)
            .unwrap();

        ctx.semaphore().clone().wait().await;

        let result = match ctx.response.as_event() {
            Some(TrbEvent::Transfer(v)) => Some(v.copied()),
            _ => None,
        };
        match ctx.state() {
            RequestState::Available | RequestState::Acquired | RequestState::Scheduled => {
                unreachable!()
            }
            RequestState::Completed => match result {
                Some(result) => match result.completion_code() {
                    Some(TrbCompletionCode::SUCCESS) => {
                        let size = len - result.transfer_length();
                        if let TransferDirection::Read(p) = transfer_mode {
                            p.copy_from_nonoverlapping(buffer.direct_map(), size);
                        }
                        Ok(UsbLength(size as u16))
                    }
                    Some(TrbCompletionCode::STALL) => {
                        let _ = host.reset_endpoint(slot_id.unwrap(), dci.unwrap());
                        Err(UsbError::Stall)
                    }
                    Some(err) => Err(err.into()),
                    None => Err(UsbError::General),
                },
                None => Err(UsbError::General),
            },
            RequestState::Aborted => Err(UsbError::Aborted),
        }
    }

    pub async unsafe fn transfer_async(
        device: Arc<HciDeviceContext>,
        dci: DCI,
        transfer_mode: TransferDirection<u8>,
        len: UsbLength,
    ) -> Result<UsbLength, UsbError> {
        let host = device.host();
        let slot_id = Some(device.slot_id);
        let dci = Some(dci);
        let ctx = match host.ep_ring_index(slot_id, dci) {
            Some(index) => {
                let ctx = &mut host.ring_context.write().unwrap()[index];
                (&mut *ctx.as_mut_ptr()).scoped().await
            }
            None => todo!(),
        };
        ctx.set_scheduled();

        let buffer = ctx.buffer();
        if let TransferDirection::Write(p) = transfer_mode {
            buffer
                .direct_map::<u8>()
                .copy_from_nonoverlapping(p, len.as_usize());
        }

        let trb = TrbNormal::new(*buffer, len.as_usize(), true, true);
        host.issue_trb(None, trb.as_trb(), slot_id, dci);

        host.ring_a_doorbell_async(device.parent_slot_id, slot_id, dci)
            .unwrap();

        ctx.semaphore().clone().wait().await;

        let result = match ctx.response.as_event() {
            Some(TrbEvent::Transfer(v)) => Some(v.copied()),
            _ => None,
        };
        match ctx.state() {
            RequestState::Available | RequestState::Acquired | RequestState::Scheduled => {
                unreachable!()
            }
            RequestState::Completed => match result {
                Some(result) => match result.completion_code() {
                    Some(TrbCompletionCode::SUCCESS) | Some(TrbCompletionCode::SHORT_PACKET) => {
                        let size = len.as_usize() - result.transfer_length();
                        if let TransferDirection::Read(p) = transfer_mode {
                            p.copy_from_nonoverlapping(buffer.direct_map(), size);
                        }
                        Ok(UsbLength(size as u16))
                    }
                    Some(err) => Err(err.into()),
                    None => Err(UsbError::General),
                },
                None => Err(UsbError::UnexpectedToken),
            },
            RequestState::Aborted => Err(UsbError::Aborted),
        }
    }

    #[inline]
    pub fn reset_endpoint(
        &self,
        slot_id: SlotId,
        dci: DCI,
    ) -> Result<TrbCommandCompletionEvent, TrbCommandCompletionEvent> {
        let trb = TrbResetEndpointCommand::new(slot_id, dci);
        self.execute_command(trb.as_trb())
    }

    pub fn configure_endpoint(
        &self,
        slot_id: SlotId,
        dci: DCI,
        ep_type: EpType,
        max_packet_size: UsbLength,
        interval: u8,
        copy_dc: bool,
    ) {
        let input_context = self.input_context(slot_id);
        let control = input_context.control();
        let slot = input_context.slot();
        let endpoint = input_context.endpoint(dci);
        let psiv: PSIV = slot.speed();

        control.clear();
        control.set_add(1 | (1u32 << dci.0.get()));

        if copy_dc {
            unsafe {
                let slot = slot as *const _ as *mut u8;
                let dc = self.get_device_context(slot_id).direct_map();
                slot.copy_from(dc, self.context_size);
            }
        }

        slot.set_context_entries(usize::max(dci.0.get() as usize, slot.context_entries()));

        endpoint.set_ep_type(ep_type);

        if max_packet_size.0 > 0 {
            endpoint.set_max_packet_size(UsbLength(max_packet_size.0 & 0x07FF));
            endpoint.set_max_burst_size(((max_packet_size.0 as usize) & 0x1800) >> 11)
        } else {
            endpoint.set_max_packet_size(psiv.max_packet_size());
            endpoint.set_average_trb_len(8);
        }
        if interval > 0 {
            match psiv {
                PSIV::LS | PSIV::FS => {
                    if ep_type.is_interrupt() {
                        endpoint
                            .set_interval(interval.next_power_of_two().trailing_zeros() as u8 + 3);
                    } else {
                        endpoint.set_interval(interval + 2);
                    }
                }
                _ => {
                    endpoint.set_interval(interval - 1);
                }
            }
        }
        if !ep_type.is_isochronous() {
            endpoint.set_error_count(3);
        }

        let tr = self.alloc_ep_ring(Some(slot_id), Some(dci)).unwrap().get();
        endpoint.set_trdp(tr);
    }

    pub fn configure_hub2(
        &self,
        slot_id: SlotId,
        hub_desc: &Usb2HubDescriptor,
        is_mtt: bool,
    ) -> Result<(), UsbError> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(1);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = self.get_device_context(slot_id).direct_map();
            slot.copy_from(dc, self.context_size * 2);
        }

        let slot = input_context.slot();
        slot.set_is_hub(true);
        slot.set_num_ports(hub_desc.num_ports());
        slot.set_is_mtt(is_mtt);
        slot.set_ttt(hub_desc.characteristics().ttt());

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(trb.as_trb()) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_usb_error()),
        }
    }

    pub fn configure_hub3(
        &self,
        slot_id: SlotId,
        hub_desc: &Usb3HubDescriptor,
    ) -> Result<(), UsbError> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(1);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = self.get_device_context(slot_id).direct_map();
            slot.copy_from(dc, self.context_size * 2);
        }

        let slot = input_context.slot();
        slot.set_is_hub(true);
        slot.set_num_ports(hub_desc.num_ports());
        // slot.set_max_exit_latency(max_exit_latency);

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(trb.as_trb()) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_usb_error()),
        }
    }

    pub async fn attach_child_device(
        self: Arc<Self>,
        hub: Arc<HciDeviceContext>,
        port: UsbHubPortNumber,
        speed: PSIV,
    ) -> Result<UsbAddress, UsbError> {
        let new_route = match hub.route_string.appending(port) {
            Ok(v) => v,
            Err(_) => return Err(UsbError::InvalidParameter),
        };

        let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
        let slot_id = match self.execute_command(&trb) {
            Ok(result) => result.slot_id().unwrap(),
            Err(err) => {
                return Err(err.to_usb_error());
            }
        };
        let addr = unsafe { UsbAddress::from_nonzero_unchecked(slot_id.0) };

        let device_context_size = self.context_size * 32;
        let device_context = unsafe { MemoryManager::alloc_pages(device_context_size) }
            .unwrap()
            .get();
        self.set_device_context(slot_id, device_context);

        let input_context_size = self.context_size * 33;
        let input_context_pa = unsafe { MemoryManager::alloc_pages(input_context_size) }
            .unwrap()
            .get();
        let input_context = self.input_context(slot_id);
        input_context.init(input_context_pa, self.context_size);

        let slot = input_context.slot();
        slot.set_root_hub_port(hub.root_port_id);
        slot.set_context_entries(1);
        slot.set_route_string(new_route);
        slot.set_speed(speed);

        if speed < hub.psiv {
            slot.set_parent_hub_slot_id(hub.slot_id);
            slot.set_parent_port_id(port);
        }

        self.configure_endpoint(
            slot_id,
            DCI::CONTROL,
            EpType::Control,
            UsbLength(0),
            0,
            false,
        );

        Timer::sleep(Duration::from_millis(100));

        // log!(
        //     "ATTACH HUB DEVICE: SLOT {} ROOT {} ROUTE {:05x} PSIV {:?}",
        //     slot_id.0.get(),
        //     device.root_port_id.0.get(),
        //     new_route.as_u32(),
        //     speed,
        // );

        let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
        match self.execute_command(trb.as_trb()) {
            Ok(_) => (),
            Err(err) => {
                let err = err.to_usb_error();
                UsbManager::notify_error(err);
                log!("ADDRESS_DEVICE ERROR {:?}", err);
                log!(
                    " {}.{} {:?} {:?}",
                    hub.slot_id.0.get(),
                    port.0.get(),
                    hub.speed(),
                    speed,
                );
                return Err(err);
            }
        }

        let ctx = Arc::new(HciDeviceContext {
            host: self.clone(),
            root_port_id: hub.root_port_id,
            port_id: port.into(),
            slot_id,
            parent_slot_id: Some(hub.slot_id),
            route_string: new_route,
            psiv: speed,
        });
        UsbManager::instantiate(addr, ctx as Arc<dyn UsbDeviceInterface>)
            .await
            .map(|_| addr)
    }

    pub async fn attach_root_device(self: &Arc<Self>, port_id: PortId) -> Option<UsbAddress> {
        self.wait_cnr(0);
        let port = self.port_by(port_id);

        // log!(
        //     "ATTACH_ROOT {:?} {:08x} {:?} {:?}",
        //     port_id,
        //     port.status().bits(),
        //     port.status().link_state(),
        //     port.status().speed(),
        // );

        let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
        let slot_id = match self.execute_command(&trb) {
            Ok(result) => result.slot_id().unwrap(),
            Err(err) => {
                log!("ENABLE_SLOT ERROR {:?}", err.completion_code());
                return None;
            }
        };
        let addr = unsafe { UsbAddress::from_nonzero_unchecked(slot_id.0) };

        self.port2slot[port_id.0.get() as usize].store(slot_id.0.get(), Ordering::Release);
        self.slot2port[slot_id.0.get() as usize].store(port_id.0.get(), Ordering::Release);

        let device_context_size = self.context_size * 32;
        let device_context = unsafe { MemoryManager::alloc_pages(device_context_size) }
            .unwrap()
            .get();
        self.set_device_context(slot_id, device_context);

        let input_context_size = self.context_size * 33;
        let input_context_pa = unsafe { MemoryManager::alloc_pages(input_context_size) }
            .unwrap()
            .get();
        let input_context = self.input_context(slot_id);
        input_context.init(input_context_pa, self.context_size);

        let slot = input_context.slot();
        let psiv = port.status().speed().unwrap_or(PSIV::SS);
        slot.set_root_hub_port(port_id);
        slot.set_speed(psiv);
        slot.set_context_entries(1);

        self.configure_endpoint(
            slot_id,
            DCI::CONTROL,
            EpType::Control,
            UsbLength(0),
            0,
            false,
        );

        Timer::sleep_async(Duration::from_millis(10)).await;

        let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
        match self.execute_command(trb.as_trb()) {
            Ok(_result) => {
                // log!("ADDRESS DEVICE OK {:?}", slot_id);
            }
            Err(err) => {
                log!(
                    "ROOT PORT {} ADDRESS_DEVICE ERROR {:?}",
                    port_id.0.get(),
                    err.completion_code()
                );
                return None;
            }
        }

        let ctx = Arc::new(HciDeviceContext {
            host: self.clone(),
            root_port_id: port_id,
            port_id,
            slot_id,
            parent_slot_id: None,
            route_string: UsbRouteString::EMPTY,
            psiv,
        });
        match UsbManager::instantiate(addr, ctx as Arc<dyn UsbDeviceInterface>).await {
            Ok(_) => {
                Timer::sleep_async(Duration::from_millis(10)).await;
            }
            Err(_err) => {
                let port = self.port_by(port_id);
                let status = port.status();
                port.write(status & PortSc::PRESERVE_MASK | PortSc::PRC | PortSc::PR);
            }
        }

        Some(addr)
    }

    pub fn set_max_packet_size(
        &self,
        slot_id: SlotId,
        max_packet_size: UsbLength,
    ) -> Result<(), ()> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(3);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = self.get_device_context(slot_id).direct_map();
            slot.copy_from(dc, self.context_size * 2);
        }

        let endpoint = input_context.endpoint(DCI::CONTROL);
        endpoint.set_max_packet_size(max_packet_size);

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(trb.as_trb()) {
            Ok(_) => Ok(()),
            Err(_) => todo!(),
        }
    }

    pub fn process_event(&self) {
        while let Some(event) = self.rts.primary_irs().dequeue_event(&self.event_cycle) {
            let event = match event.as_event() {
                Some(v) => v,
                None => {
                    panic!("XHCI: UNHANDLED EVENT TRB {:?}", event.trb_type(),);
                }
            };
            match event {
                TrbEvent::Transfer(event) => {
                    let event_trb = ScheduledTrb(event.ptr());

                    match unsafe { event_trb.peek().trb_type() } {
                        Some(TrbType::NORMAL) | Some(TrbType::STATUS) => {}
                        _ => {
                            // log!(
                            //     "USB Transfer error {} {:?} {:?}",
                            //     event.slot_id().map(|v| v.0.get()).unwrap_or(0),
                            //     unsafe { event_trb.peek().trb_type() },
                            //     event.completion_code(),
                            // );

                            unsafe {
                                let nop_trb = TrbNop::new();
                                let next_trb = {
                                    let next_trb = event_trb.next();
                                    if next_trb.peek().trb_type() != Some(TrbType::LINK) {
                                        next_trb
                                    } else {
                                        let link_trb: &TrbLink = transmute(next_trb.peek());
                                        ScheduledTrb(link_trb.ptr() & !0x0F)
                                    }
                                };

                                if event_trb.peek().trb_type() == Some(TrbType::DATA)
                                    && next_trb.peek().trb_type() == Some(TrbType::STATUS)
                                {
                                    // ex. STALL ERROR in DATA STAGE
                                    event_trb.peek().copy_without_cycle(&nop_trb);
                                    next_trb.peek().copy_without_cycle(&nop_trb);
                                } else if event_trb.peek().trb_type() == Some(TrbType::SETUP) {
                                    let last_trb = {
                                        let last_trb = next_trb.next();
                                        if last_trb.peek().trb_type() != Some(TrbType::LINK) {
                                            last_trb
                                        } else {
                                            let link_trb: &TrbLink = transmute(next_trb.peek());
                                            ScheduledTrb(link_trb.ptr() & !0x0F)
                                        }
                                    };
                                    // ex. USB TRANSACTION ERROR in SETUP STAGE
                                    if next_trb.peek().trb_type() == Some(TrbType::STATUS) {
                                        // SETUP - STATUS
                                        event_trb.peek().copy_without_cycle(&nop_trb);
                                        next_trb.peek().copy_without_cycle(&nop_trb);
                                    } else if next_trb.peek().trb_type() == Some(TrbType::DATA)
                                        && last_trb.peek().trb_type() == Some(TrbType::STATUS)
                                    {
                                        // SETUP - DATA - STATUS
                                        event_trb.peek().copy_without_cycle(&nop_trb);
                                        next_trb.peek().copy_without_cycle(&nop_trb);
                                        last_trb.peek().copy_without_cycle(&nop_trb);
                                    } else {
                                        todo!()
                                    }
                                } else {
                                    todo!()
                                }
                            }
                        }
                    }

                    let slot_id = event.slot_id();
                    let dci = event.dci();
                    let index = self.ep_ring_index(slot_id, dci).unwrap();
                    let ctx = &mut self.ring_context.write().unwrap()[index];
                    let ctx = unsafe { &mut *ctx.as_mut_ptr() };
                    match ctx.set_response(event) {
                        Some(_) => {}
                        None => {
                            let event_trb = unsafe { event_trb.peek() };
                            let last_setup =
                                unsafe { ctx.last_setup().transmute::<TrbSetupStage>() };
                            let last_setup = last_setup.setup_data();
                            log!(
                                "USB Transaction Error SLOT {} {} [{:02x} {:02x} {:04x} {:04x} {:04x}] {:?} CC {:?} STATUS {:?}",
                                slot_id.map(|v| v.0.get()).unwrap_or_default(),
                                ctx.index(),
                                last_setup.bmRequestType.0,
                                last_setup.bRequest.0,
                                last_setup.wValue,
                                last_setup.wIndex,
                                last_setup.wLength.0,
                                event_trb.trb_type().unwrap_or(TrbType::RESERVED),
                                event.completion_code().unwrap_or(TrbCompletionCode::INVALID),
                                ctx.state(),
                            );
                        }
                    }
                }
                TrbEvent::CommandCompletion(event) => {
                    let event_trb = ScheduledTrb(event.ptr());

                    // unsafe {
                    //     log!(
                    //         "CCE {} {:?} {:?}",
                    //         event.slot_id().map(|v| v.0.get()).unwrap_or(0),
                    //         event_trb.peek().trb_type(),
                    //         event.completion_code()
                    //     );
                    // }

                    if let Some(crb) = self.find_crb(event_trb, Some(RequestState::Scheduled)) {
                        crb.set_response(event.as_trb());
                    } else {
                        todo!()
                    }
                }
                TrbEvent::PortStatusChange(event) => {
                    let port_id = event.port_id().unwrap();
                    // log!("PSC {:?}", port_id);
                    self.port_status_change_queue.post(port_id).unwrap();
                }
                TrbEvent::DeviceNotification(event) => {
                    log!(
                        "DEVICE_NOTIFICATION {} {:?}",
                        event.slot_id().unwrap().0.get(),
                        event.completion_code()
                    );
                }
            }
        }
    }

    /// USB Hub scheduler
    async fn _schedule_task(self: Arc<Self>) {
        let mut locked_hub = None;
        let mut retired = VecDeque::new();
        while let Some(task) = self.doorbell_queue.wait_event().await {
            let mut active_tasks = VecDeque::new();
            active_tasks.push_back(task);
            loop {
                while let Some(task) = self.doorbell_queue.get_event() {
                    active_tasks.push_back(task);
                }
                while let Some(task) = active_tasks.pop_front() {
                    match task {
                        QueuedDoorbell::Doorbell(hub, slot_id, dci) => {
                            if locked_hub.is_none()
                                || locked_hub == Some(hub)
                                || locked_hub == Some(slot_id)
                            {
                                self.ring_a_doorbell(slot_id, dci);
                            } else {
                                retired.push_back(task);
                            }
                        }
                        QueuedDoorbell::FocusHub(hub) => {
                            if locked_hub.is_none() {
                                locked_hub = Some(hub);
                            } else {
                                retired.push_back(task);
                            }
                        }
                        QueuedDoorbell::UnfocusHub(hub) => {
                            if locked_hub == Some(hub) {
                                locked_hub = None;
                                while let Some(task) = retired.pop_front() {
                                    active_tasks.push_back(task);
                                }
                            } else {
                                retired.push_back(task);
                            }
                        }
                    }
                }

                Timer::sleep_async(Duration::from_millis(1)).await;
                if active_tasks.len() == 0 {
                    break;
                }
            }
        }
    }

    /// xHCI Root hub task
    async fn _root_hub_task(self: Arc<Self>) {
        self.focus_hub(None);

        for (port_id, port) in self.usb3_ports() {
            self.wait_cnr(0);
            let status = port.status();
            // log!(
            //     "USB3 {:?} {:08x} {:?} {:?}",
            //     port_id,
            //     status.bits(),
            //     status.link_state(),
            //     status.speed(),
            // );
            if status.is_connected() {
                if status.is_disabled() {
                    let deadline = Timer::new(Duration::from_millis(100));
                    loop {
                        self.wait_cnr(0);
                        if port.status().is_enabled() || deadline.is_expired() {
                            break;
                        }
                        Timer::sleep_async(Duration::from_millis(1)).await;
                    }

                    // let status = port.status();
                    // log!(
                    //     "NEW_STAT {:08x} {:?} {:?}",
                    //     status.bits(),
                    //     status.link_state(),
                    //     status.speed(),
                    // );

                    if port.status().is_enabled() {
                        self._attach_root_port(port_id).await;
                    } else {
                        self.wait_cnr(0);
                        port.set(PortSc::PR);
                        // Timer::sleep_async(Duration::from_millis(10)).await;
                    }
                } else {
                    self._attach_root_port(port_id).await;
                }
            }
        }
        for (_port_id, port) in self.usb2_ports() {
            self.wait_cnr(0);
            // let status = port.status();
            // log!(
            //     "USB2 {:?} {:08x} {:?} {:?}",
            //     _port_id,
            //     status.bits(),
            //     status.link_state(),
            //     status.speed(),
            // );
            port.set(PortSc::PR);
            // Timer::sleep_async(Duration::from_millis(10)).await;
        }

        self.unfocus_hub(None);

        while let Some(port_id) = self.port_status_change_queue.wait_event().await {
            let mut ports = Vec::new();
            ports.push(port_id);
            while let Some(port_id) = self.port_status_change_queue.get_event() {
                ports.push(port_id);
            }
            self.focus_hub(None);
            for port_id in ports {
                self._process_port_change(port_id, false).await;
                self.port_by(port_id).clear_changes();
            }
            self.unfocus_hub(None);
        }
    }

    pub async fn _attach_root_port(self: &Arc<Self>, port_id: PortId) {
        let port = self.port_by(port_id);
        self.wait_cnr(0);
        let status = port.status();

        if status.is_connected() && status.is_enabled() {
            port.clear_changes();
            self.attach_root_device(port_id).await;

            // self.slot_by_port(port_id)
            //     .and_then(|v| {
            //         UsbManager::device_by_addr(unsafe { UsbAddress::from_nonzero_unchecked(v.0) })
            //     })
            //     .map(|device| {
            //         log!(
            //             "{:03}.{:03} VID {} PID {} class {} {}{}",
            //             device.parent().map(|v| v.as_u8()).unwrap_or_default(),
            //             device.addr().as_u8(),
            //             device.vid(),
            //             device.pid(),
            //             device.class(),
            //             if device.is_configured() { "" } else { "? " },
            //             device.preferred_device_name().unwrap_or("Unknown Device"),
            //         );
            //     });
        } else {
            port.power_off();
            Timer::sleep_async(Duration::from_millis(100)).await;
            port.set(PortSc::PP | PortSc::PR);
            log!("XHCI: PORT RESET TIMED OUT {}", port_id.0.get());
        }
    }

    pub async fn _process_port_change(self: &Arc<Self>, port_id: PortId, force: bool) {
        let port = self.port_by(port_id);
        self.wait_cnr(0);
        let status = port.status();
        if force || status.is_connected_status_changed() {
            if status.is_connected() {
                // Attached USB device

                // log!("PORT {} is_connected", port_id.0.get());

                port.set(PortSc::CSC | PortSc::PR);

                let deadline = Timer::new(Duration::from_millis(200));
                loop {
                    if port.status().is_enabled() || deadline.is_expired() {
                        break;
                    }
                    Timer::sleep_async(Duration::from_millis(10)).await;
                }
                if deadline.is_expired() {
                    log!("PORT {} PROCESS TIMED OUT", port_id.0.get());
                    port.power_off();
                    Timer::sleep_async(Duration::from_millis(100)).await;
                    port.set(PortSc::PP | PortSc::PR);
                    return;
                }

                self._attach_root_port(port_id).await;
            } else {
                // log!("PORT {} is_disconnected", port_id.0.get());

                // Detached USB device
                port.set(PortSc::CSC);

                if let Some(addr) = UsbAddress::from_u8(
                    self.port2slot[port_id.0.get() as usize].swap(0, Ordering::SeqCst),
                ) {
                    let _ = UsbManager::remove_device(addr);
                }
            }
        }
    }

    pub fn focus_hub(self: &Arc<Self>, hub: Option<SlotId>) {
        self.clone()
            .doorbell_queue
            .post(QueuedDoorbell::FocusHub(hub))
            .ok()
            .unwrap();
    }

    pub fn unfocus_hub(self: &Arc<Self>, hub: Option<SlotId>) {
        self.doorbell_queue
            .post(QueuedDoorbell::UnfocusHub(hub))
            .ok()
            .unwrap();
    }
}

impl Drop for Xhci {
    fn drop(&mut self) {
        todo!()
    }
}

impl PciDriver for Xhci {
    fn address(&self) -> PciConfigAddress {
        self.addr
    }

    fn name<'a>(&self) -> &'a str {
        Self::DRIVER_NAME
    }

    fn current_status(&self) -> String {
        let cmd = self.opr.read_cmd();
        let sts = self.opr.status();
        let status = if !cmd.contains(UsbCmd::RUN) {
            "STOPPED"
        } else if sts.contains(UsbSts::HCH) {
            "HALTED"
        } else {
            "RUNNING"
        };

        format!(
            "{} ports {} slots {} ctx {}",
            status,
            self.cap.max_ports(),
            self.cap.max_device_slots(),
            self.context_size,
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum QueuedDoorbell {
    Doorbell(Option<SlotId>, Option<SlotId>, Option<DCI>),
    FocusHub(Option<SlotId>),
    UnfocusHub(Option<SlotId>),
}

pub struct EpRingContext {
    tr_base: Option<NonNullPhysicalAddress>,
    state: AtomicUsize,
    slot_id: Option<SlotId>,
    dci: Option<DCI>,
    index: usize,
    pcs: CycleBit,
    response: Trb,
    sem_scope: MaybeUninit<Pin<Arc<AsyncSemaphore>>>,
    signal: MaybeUninit<Pin<Arc<AsyncSemaphore>>>,
    last_setup: Trb,
    buffer: PhysicalAddress,
}

impl EpRingContext {
    #[inline]
    pub fn state(&self) -> RequestState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: RequestState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(
        &self,
        current: RequestState,
        new: RequestState,
    ) -> Result<(), RequestState> {
        match self.state.compare_exchange_weak(
            current as usize,
            new as usize,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(v) => Err(FromPrimitive::from_usize(v).unwrap_or_default()),
        }
    }

    #[inline]
    pub const fn tr_base(&self) -> Option<NonNullPhysicalAddress> {
        self.tr_base
    }

    #[inline]
    pub fn tr_value(&self) -> Option<NonNullPhysicalAddress> {
        self.tr_base
            .and_then(|v| NonNullPhysicalAddress::new(v.get() | self.pcs.tr_value()))
    }

    #[inline]
    pub const fn slot_id(&self) -> Option<SlotId> {
        self.slot_id
    }

    #[inline]
    pub const fn dci(&self) -> Option<DCI> {
        self.dci
    }

    #[inline]
    pub const fn pcs(&self) -> &CycleBit {
        &self.pcs
    }

    #[inline]
    pub const fn size() -> usize {
        (Xhci::SIZE_EP_RING) * size_of::<Trb>()
    }

    #[inline]
    pub const fn last_setup(&self) -> &Trb {
        &self.last_setup
    }

    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn buffer(&self) -> &PhysicalAddress {
        &self.buffer
    }

    #[inline]
    pub fn semaphore(&self) -> &Pin<Arc<AsyncSemaphore>> {
        unsafe { self.signal.assume_init_ref() }
    }

    #[inline]
    pub async fn scoped<'a>(&'a mut self) -> EpRingScopeGuard<'a> {
        unsafe { self.sem_scope.assume_init_ref() }
            .clone()
            .wait()
            .await;
        EpRingScopeGuard(self)
    }

    #[inline]
    pub fn clear(&mut self) {
        if let Some(tr_base) = self.tr_base {
            unsafe {
                let p = tr_base.get().direct_map::<c_void>();
                p.write_bytes(0, Self::size());
            }
        }
        self.pcs.reset();
        self.index = 0;
        self.response = Trb::empty();
    }

    #[inline]
    pub unsafe fn alloc(&mut self, slot_id: Option<SlotId>, dci: Option<DCI>) {
        self.tr_base =
            NonNullPhysicalAddress::new(MemoryManager::alloc_pages(Self::size()).unwrap().get());
        self.slot_id = slot_id;
        self.dci = dci;
        self.pcs.reset();
        self.index = 0;
        self.response = Trb::empty();
        self.sem_scope.write(AsyncSemaphore::new(1));
        self.signal.write(AsyncSemaphore::new(0));
        self.buffer = MemoryManager::alloc_pages(MemoryManager::PAGE_SIZE_MIN)
            .unwrap()
            .get();
    }

    #[inline]
    pub fn set_response<T: TrbBase>(&self, response: &T) -> Option<()> {
        match self.compare_exchange_state(RequestState::Scheduled, RequestState::Completed) {
            Ok(_) => {
                self.response.raw_copy(response);
                unsafe {
                    self.signal.assume_init_ref().signal();
                }
                Some(())
            }
            Err(_state) => None,
        }
    }

    #[inline]
    pub fn set_scheduled(&self) {
        self.set_state(RequestState::Scheduled);
    }
}

pub struct EpRingScopeGuard<'a>(&'a mut EpRingContext);

impl Drop for EpRingScopeGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.0.sem_scope.assume_init_ref().signal();
        }
    }
}

impl Deref for EpRingScopeGuard<'_> {
    type Target = EpRingContext;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EpRingScopeGuard<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTrb(pub PhysicalAddress);

impl ScheduledTrb {
    #[inline]
    pub const fn empty() -> Self {
        Self(PhysicalAddress::NULL)
    }

    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0 + size_of::<Trb>())
    }

    #[inline]
    pub unsafe fn peek(&self) -> &mut Trb {
        &mut *(self.0.direct_map())
    }
}

pub struct CommandRequestBlock {
    state: AtomicUsize,
    scheduled_trb: ScheduledTrb,
    signal: Semaphore,
    reuse_delay: Timer,
    request: Trb,
    response: Trb,
}

impl CommandRequestBlock {
    pub const EMPTY: UnsafeCell<Self> = UnsafeCell::new(Self::new());

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            scheduled_trb: ScheduledTrb::empty(),
            signal: Semaphore::new(0),
            reuse_delay: Timer::JUST,
            request: Trb::empty(),
            response: Trb::empty(),
        }
    }

    #[inline]
    pub fn state(&self) -> RequestState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: RequestState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(
        &self,
        current: RequestState,
        new: RequestState,
    ) -> Result<(), RequestState> {
        match self.state.compare_exchange_weak(
            current as usize,
            new as usize,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(v) => Err(FromPrimitive::from_usize(v).unwrap_or_default()),
        }
    }

    #[inline]
    pub fn schedule(&mut self, request: &Trb, scheduled_trb: ScheduledTrb) {
        self.scheduled_trb = scheduled_trb;
        self.request.raw_copy(request);
        self.response = Trb::empty();
        fence(Ordering::SeqCst);
        self.set_state(RequestState::Scheduled);
    }

    #[inline]
    pub fn try_to_acquire(&self) -> bool {
        if self.reuse_delay.is_alive() {
            return false;
        }
        self.compare_exchange_state(RequestState::Available, RequestState::Acquired)
            .is_ok()
    }

    #[inline]
    pub fn wait(&self) {
        self.signal.wait()
    }

    #[inline]
    pub fn set_response(&mut self, response: &Trb) {
        self.response.raw_copy(response);
        match self.compare_exchange_state(RequestState::Scheduled, RequestState::Completed) {
            Ok(_) => self.signal.signal(),
            Err(_err) => {
                // TODO:
            }
        }
    }
}

impl DisposeRef for CommandRequestBlock {
    #[inline]
    fn dispose_ref(&mut self) {
        self.reuse_delay = Timer::new(Duration::from_millis(10));
        self.set_state(RequestState::Available);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum RequestState {
    Available = 0,
    Acquired,
    Scheduled,
    Completed,
    Aborted,
}

impl Default for RequestState {
    #[inline]
    fn default() -> Self {
        Self::Available
    }
}

pub struct InputContext {
    pa: PhysicalAddress,
    context_size: usize,
}

impl InputContext {
    pub const EMPTY: UnsafeCell<Self> = UnsafeCell::new(Self::empty());

    #[inline]
    pub const fn empty() -> Self {
        Self {
            pa: PhysicalAddress::NULL,
            context_size: 0,
        }
    }

    #[inline]
    pub fn init(&mut self, pa: PhysicalAddress, context_size: usize) {
        self.pa = pa;
        self.context_size = context_size;
    }

    #[inline]
    pub fn raw_data(&self) -> PhysicalAddress {
        self.pa
    }

    #[inline]
    pub fn control<'a>(&self) -> &'a mut InputControlContext {
        unsafe { &mut *(self.raw_data().direct_map()) }
    }

    #[inline]
    pub fn slot<'a>(&self) -> &'a mut SlotContext {
        unsafe { &mut *(PhysicalAddress::from(self.raw_data() + self.context_size).direct_map()) }
    }

    #[inline]
    pub fn endpoint<'a>(&self, dci: DCI) -> &'a mut EndpointContext {
        unsafe {
            &mut *(PhysicalAddress::from(
                self.raw_data() + self.context_size * (1 + dci.0.get() as usize),
            )
            .direct_map())
        }
    }
}

pub struct HciDeviceContext {
    host: Arc<Xhci>,
    root_port_id: PortId,
    #[allow(dead_code)]
    port_id: PortId,
    slot_id: SlotId,
    parent_slot_id: Option<SlotId>,
    route_string: UsbRouteString,
    psiv: PSIV,
}

impl HciDeviceContext {
    #[inline]
    fn host(&self) -> Arc<Xhci> {
        self.host.clone()
    }
}

impl UsbDeviceInterface for HciDeviceContext {
    fn parent_device_address(&self) -> Option<UsbAddress> {
        self.parent_slot_id
            .and_then(|v| UsbAddress::from_nonzero(v.0))
    }

    fn route_string(&self) -> UsbRouteString {
        self.route_string
    }

    fn speed(&self) -> PSIV {
        self.psiv
    }

    fn set_max_packet_size(&self, max_packet_size: UsbLength) -> Result<(), UsbError> {
        let slot_id = self.slot_id;
        self.host()
            .set_max_packet_size(slot_id, max_packet_size)
            .map_err(|_| UsbError::General)
    }

    fn configure_hub2(&self, hub_desc: &Usb2HubDescriptor, is_mtt: bool) -> Result<(), UsbError> {
        let slot_id = self.slot_id;
        self.host().configure_hub2(slot_id, hub_desc, is_mtt)
    }

    fn configure_hub3(&self, hub_desc: &Usb3HubDescriptor) -> Result<(), UsbError> {
        let slot_id = self.slot_id;
        self.host().configure_hub3(slot_id, hub_desc)
    }

    fn focus_hub(&self) -> Result<(), UsbError> {
        self.host().focus_hub(Some(self.slot_id));
        Ok(())
    }

    fn unfocus_hub(&self) -> Result<(), UsbError> {
        self.host().unfocus_hub(Some(self.slot_id));
        Ok(())
    }

    fn attach_child_device(
        self: Arc<Self>,
        port: UsbHubPortNumber,
        speed: PSIV,
    ) -> Pin<Box<dyn Future<Output = Result<UsbAddress, UsbError>>>> {
        Box::pin(self.host().attach_child_device(self.clone(), port, speed))
    }

    fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError> {
        let host = self.host();
        let slot_id = self.slot_id;

        let ep = match desc.endpoint_address() {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        let ep_type = desc.ep_type();
        let ep_type = EpType::from_usb_ep_type(ep_type, ep.is_dir_in());
        let dci = ep.into();

        host.configure_endpoint(
            slot_id,
            dci,
            ep_type,
            desc.max_packet_size(),
            desc.interval(),
            true,
        );

        let trb = TrbConfigureEndpointCommand::new(slot_id, host.input_context(slot_id).raw_data());
        match host.execute_command(trb.as_trb()) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_usb_error()),
        }
    }

    unsafe fn control_recv(
        self: Arc<Self>,
        setup: UsbControlSetupData,
        data: *mut u8,
    ) -> Pin<Box<dyn Future<Output = Result<UsbLength, UsbError>>>> {
        Box::pin(Xhci::control_async(
            self.clone(),
            setup,
            TransferDirection::Read(data),
        ))
    }

    unsafe fn control_send(
        self: Arc<Self>,
        setup: UsbControlSetupData,
        data: *const u8,
    ) -> Pin<Box<dyn Future<Output = Result<UsbLength, UsbError>>>> {
        if setup.wLength.is_empty() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        Box::pin(Xhci::control_async(
            self.clone(),
            setup,
            TransferDirection::Write(data),
        ))
    }

    unsafe fn read(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *mut u8,
        len: UsbLength,
    ) -> Pin<Box<dyn Future<Output = Result<UsbLength, UsbError>>>> {
        let dci = DCI::from(ep);
        if len.is_empty() || !dci.can_read() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        Box::pin(Xhci::transfer_async(
            self.clone(),
            dci,
            TransferDirection::Read(buffer),
            len,
        ))
    }

    unsafe fn write(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *const u8,
        len: UsbLength,
    ) -> Pin<Box<dyn Future<Output = Result<UsbLength, UsbError>>>> {
        let dci = DCI::from(ep);
        if len.is_empty() || !dci.can_write() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        Box::pin(Xhci::transfer_async(
            self.clone(),
            dci,
            TransferDirection::Write(buffer),
            len,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
struct AsyncUsbError<T> {
    error: UsbError,
    _phantom: PhantomData<T>,
}

impl<T> AsyncUsbError<T> {
    pub const fn new(error: UsbError) -> Self {
        Self {
            error,
            _phantom: PhantomData,
        }
    }
}

impl<T> Future for AsyncUsbError<T> {
    type Output = Result<T, UsbError>;

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        Poll::Ready(Err(self.error))
    }
}

#[derive(Clone, Copy)]
pub enum TransferDirection<T> {
    Read(*mut T),
    Write(*const T),
}
