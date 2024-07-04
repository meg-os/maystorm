use super::install_drivers;
use crate::sync::RwLock;
use crate::*;
use core::cell::UnsafeCell;
use core::fmt;
use core::num::NonZeroU8;
use core::ops::Add;

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciConfigAddress {
    bus: u8,
    dev: u8,
    fun: u8,
    register: u8,
}

impl PciConfigAddress {
    #[inline]
    pub const fn bus(bus: u8) -> Self {
        Self {
            bus,
            dev: 0,
            fun: 0,
            register: 0,
        }
    }

    #[inline]
    pub const fn dev(mut self, dev: u8) -> Self {
        self.dev = dev;
        self
    }

    #[inline]
    pub const fn fun(mut self, fun: u8) -> Self {
        self.fun = fun;
        self
    }

    #[inline]
    pub const fn register(mut self, register: u8) -> Self {
        self.register = register;
        self
    }

    #[inline]
    pub const fn get_bus(&self) -> u8 {
        self.bus
    }

    #[inline]
    pub const fn get_dev(&self) -> u8 {
        self.dev
    }

    #[inline]
    pub const fn get_fun(&self) -> u8 {
        self.fun
    }

    #[inline]
    pub const fn get_register(&self) -> u8 {
        self.register
    }
}

impl Add<u8> for PciConfigAddress {
    type Output = Self;

    fn add(self, rhs: u8) -> Self::Output {
        let register = self.get_register().wrapping_add(rhs);
        self.register(register)
    }
}

impl fmt::Debug for PciConfigAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}.{}",
            self.get_bus(),
            self.get_dev(),
            self.get_fun()
        )
    }
}

pub trait PciDriverRegistrar {
    fn instantiate(&self, device: &'static PciDevice) -> Option<Arc<dyn PciDriver>>;
}

pub trait PciDriver {
    /// Returns the PCI configuration address of this device instance.
    fn address(&self) -> PciConfigAddress;

    /// Returns the name of the device driver.
    fn name<'a>(&self) -> &'a str;

    /// Returns the current state of the device in a human-readable format.
    fn current_status(&self) -> String;
}

static mut PCI: UnsafeCell<Pci> = UnsafeCell::new(Pci::new());

#[allow(dead_code)]
pub struct Pci {
    devices: BTreeMap<PciConfigAddress, PciDevice>,
    registrars: Vec<Box<dyn PciDriverRegistrar>>,
    drivers: RwLock<BTreeMap<PciConfigAddress, Arc<dyn PciDriver>>>,
}

impl Pci {
    const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            registrars: Vec::new(),
            drivers: RwLock::new(BTreeMap::new()),
        }
    }

    #[inline]
    unsafe fn shared_mut<'a>() -> &'a mut Pci {
        PCI.get_mut()
    }

    #[inline]
    fn shared<'a>() -> &'a Pci {
        unsafe { &*PCI.get() }
    }

    pub unsafe fn init() {
        assert_call_once!();

        let shared = Self::shared_mut();
        install_drivers(&mut shared.registrars);

        let bus = 0;
        for dev in 0..32 {
            PciDevice::instantiate(bus, dev, 0);
        }

        for device in Self::devices() {
            for registrar in &shared.registrars {
                match registrar.instantiate(&device) {
                    Some(v) => {
                        shared.drivers.write().unwrap().insert(device.address(), v);
                    }
                    None => {}
                }
            }
        }
    }

    pub fn devices() -> impl Iterator<Item = &'static PciDevice> {
        Self::shared().devices.values().into_iter()
    }

    pub fn device_by_addr<'a>(addr: PciConfigAddress) -> Option<&'a PciDevice> {
        Self::shared().devices.get(&addr)
    }

    pub fn drivers() -> impl Iterator<Item = Arc<dyn PciDriver>> {
        Self::shared()
            .drivers
            .read()
            .unwrap()
            .values()
            .map(|v| v.clone())
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciVendorId(pub u16);

impl PciVendorId {
    pub const INVALID_0000: Self = Self(0x0000);
    pub const INVALID_FFFF: Self = Self(0xFFFF);

    pub const VIRTIO: Self = Self(0x1AF4);

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.0 != Self::INVALID_0000.0 && self.0 != Self::INVALID_FFFF.0
    }
}

impl fmt::Debug for PciVendorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VEN_{:04x}", self.0))
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciDeviceId(pub u16);

impl PciDeviceId {
    pub const INVALID_0000: Self = Self(0x0000);
    pub const INVALID_FFFF: Self = Self(0xFFFF);

    pub const VIRTIO_MIN: Self = Self(0x1000);
    pub const VIRTIO_MAX: Self = Self(0x107F);

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.0 != Self::INVALID_0000.0 && self.0 != Self::INVALID_FFFF.0
    }
}

impl fmt::Debug for PciDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("DEV_{:04x}", self.0))
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct PciDevice {
    addr: PciConfigAddress,
    vendor_id: PciVendorId,
    device_id: PciDeviceId,
    subsys_vendor_id: PciVendorId,
    subsys_device_id: PciDeviceId,
    class_code: PciClass,
    has_multi_func: bool,
    secondary_bus_number: Option<NonZeroU8>,
    bars: Box<[PciBar]>,
    capabilities: Box<[(PciCapabilityId, u8)]>,
}

impl PciDevice {
    unsafe fn instantiate(bus: u8, dev: u8, fun: u8) -> bool {
        let base = PciConfigAddress::bus(bus).dev(dev).fun(fun);

        let dev_ven = Hal::pci().read_pci(base);
        let vendor_id = PciVendorId(dev_ven as u16);
        let device_id = PciDeviceId((dev_ven >> 16) as u16);
        if !vendor_id.is_valid() || !device_id.is_valid() {
            return false;
        }

        let sta_cmd = Hal::pci().read_pci(base.register(1));
        let subsys = Hal::pci().read_pci(base.register(0x0B));
        let subsys_vendor_id = PciVendorId(subsys as u16);
        let subsys_device_id = PciDeviceId((subsys >> 16) as u16);
        let class_code = PciClass::from_pci(Hal::pci().read_pci(base.register(0x02)));
        let header_type = ((Hal::pci().read_pci(base.register(3)) >> 16) & 0xFF) as u8;
        let has_multi_func = (header_type & 0x80) != 0;
        let header_type = header_type & 0x7F;

        let secondary_bus_number = if header_type == 0x01 {
            // PCI to PCI bridge
            let val = Hal::pci().read_pci(base.register(6));
            let bus = (val >> 8) as u8;
            NonZeroU8::new(bus)
        } else {
            None
        };

        let bar_limit = match header_type {
            0x00 => 6,
            0x01 => 2,
            _ => 0,
        };
        let mut bars = Vec::with_capacity(bar_limit);
        let mut index = 0;
        while index < bar_limit {
            if let Some(bar) = PciBar::parse(base, index) {
                bars.push(bar);
                if bar.bar_type() == PciBarType::Mmio64 {
                    index += 1;
                }
            }
            index += 1;
        }

        let mut capabilities = Vec::new();
        if (sta_cmd & 0x0010_0000) != 0 {
            let mut cap_ptr = (Hal::pci().read_pci(base.register(0x0D)) & 0xFF) as u8;

            loop {
                let current_register = cap_ptr / 4;
                let cap_head = Hal::pci().read_pci(base.register(current_register));
                let cap_id = PciCapabilityId((cap_head & 0xFF) as u8);
                let next_ptr = ((cap_head >> 8) & 0xFF) as u8;

                capabilities.push((cap_id, current_register));

                if next_ptr == 0 {
                    break;
                } else {
                    cap_ptr = next_ptr;
                }
            }
        }

        let device = Self {
            addr: base,
            vendor_id,
            device_id,
            subsys_vendor_id,
            subsys_device_id,
            class_code,
            has_multi_func,
            secondary_bus_number,
            bars: bars.into_boxed_slice(),
            capabilities: capabilities.into_boxed_slice(),
        };
        Pci::shared_mut().devices.insert(base, device);

        if fun == 0 && has_multi_func {
            for fun in 1..8 {
                PciDevice::instantiate(bus, dev, fun);
            }
        }

        secondary_bus_number.map(|bus| {
            let bus = bus.get();
            for dev in 0..32 {
                PciDevice::instantiate(bus, dev, 0);
            }
        });

        true
    }

    #[inline]
    pub const fn address(&self) -> PciConfigAddress {
        self.addr
    }

    #[inline]
    pub fn ven_dev(&self) -> String {
        format!("PCI\\{:?}&{:?}", self.vendor_id, self.device_id,)
    }

    #[inline]
    pub fn ven_dev_subsys(&self) -> String {
        format!(
            "PCI\\{:?}&{:?}&SUBSYS_{:04x}{:04x}",
            self.vendor_id, self.device_id, self.subsys_vendor_id.0, self.subsys_device_id.0,
        )
    }

    #[inline]
    pub fn ven_dev_cc(&self) -> String {
        format!(
            "PCI\\{:?}&{:?}&{:?}",
            self.vendor_id, self.device_id, self.class_code,
        )
    }

    #[inline]
    pub const fn vendor_id(&self) -> PciVendorId {
        self.vendor_id
    }

    #[inline]
    pub const fn device_id(&self) -> PciDeviceId {
        self.device_id
    }

    #[inline]
    pub const fn subsys_vendor_id(&self) -> PciVendorId {
        self.subsys_vendor_id
    }

    #[inline]
    pub const fn subsys_device_id(&self) -> PciDeviceId {
        self.subsys_device_id
    }

    #[inline]
    pub const fn class_code(&self) -> PciClass {
        self.class_code
    }

    #[inline]
    pub const fn has_multi_func(&self) -> bool {
        self.has_multi_func
    }

    #[inline]
    pub const fn secondary_bus_number(&self) -> Option<NonZeroU8> {
        self.secondary_bus_number
    }

    #[inline]
    pub fn bars(&self) -> impl Iterator<Item = &PciBar> {
        self.bars.iter()
    }

    /// Returns an array of capability ID and register offset pairs.
    #[inline]
    pub fn capabilities(&self) -> impl ExactSizeIterator<Item = &(PciCapabilityId, u8)> {
        self.capabilities.iter()
    }

    #[inline]
    pub unsafe fn register_msi(&self, f: fn(usize) -> (), arg: usize) -> Result<(), ()> {
        let Some(msi_reg) = self
            .capabilities()
            .find(|(id, _)| *id == PciCapabilityId::MSI)
            .map(|(_, offset)| *offset)
        else {
            return Err(());
        };
        let (msi_addr, msi_data) = match Hal::pci().register_msi(f, arg) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let base = self.addr.register(msi_reg);

        Hal::pci().write_pci(base + 1, msi_addr as u32);
        Hal::pci().write_pci(base + 2, (msi_addr >> 32) as u32);
        Hal::pci().write_pci(base + 3, msi_data as u32);
        Hal::pci().write_pci(base, (Hal::pci().read_pci(base) & 0xFF8FFFFF) | 0x00010000);

        // log!(
        //     "MSI {:08x} {:04x} {:016x} {:016x}",
        //     msi_addr,
        //     msi_data,
        //     f as usize,
        //     arg
        // );

        Ok(())
    }

    pub unsafe fn read_pci_command(&self) -> PciCommand {
        PciCommand::from_bits_retain(Hal::pci().read_pci(self.addr.register(1)))
    }

    pub unsafe fn write_pci_command(&self, val: PciCommand) {
        Hal::pci().write_pci(self.addr.register(1), val.bits());
    }

    pub unsafe fn set_pci_command(&self, val: PciCommand) {
        let base = self.addr.register(1);
        Hal::pci().write_pci(base, Hal::pci().read_pci(base) | val.bits());
    }

    pub unsafe fn clear_pci_command(&self, val: PciCommand) {
        let base = self.addr.register(1);
        Hal::pci().write_pci(base, Hal::pci().read_pci(base) & !val.bits());
    }
}

my_bitflags! {
    pub struct PciCommand: u32 {
        const IO_SPACE      = 0b0000_0000_0000_0001;
        const MEM_SPACE     = 0b0000_0000_0000_0010;
        const BUS_MASTER    = 0b0000_0000_0000_0100;

        const INT_DISABLE   = 0b0000_0100_0000_0000;
    }
}

/// PCI Base Address Register
#[derive(Debug, Clone, Copy)]
pub struct PciBar {
    base: PhysicalAddress,
    bar_type: PciBarType,
    scale: u8,
    is_prefetchable: bool,
    bar_index: PciBarIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciBarIndex(pub u8);

impl PciBar {
    /// Parse bar
    unsafe fn parse(config: PciConfigAddress, index: usize) -> Option<PciBar> {
        without_interrupts!({
            let reg = config.register(4 + index as u8);
            let raw = Hal::pci().read_pci(reg);
            if raw == 0 {
                return None;
            }
            let (bar_type, is_prefetchable) = if (raw & 1) == 0 {
                (
                    match raw & 0x06 {
                        0x00 => PciBarType::Mmio32,
                        0x02 => PciBarType::Mmio1MB,
                        0x04 => PciBarType::Mmio64,
                        _ => PciBarType::Reserved,
                    },
                    (raw & 0x08) == 0x08,
                )
            } else {
                (PciBarType::IsolatedIO, false)
            };

            let result = match bar_type {
                PciBarType::IsolatedIO | PciBarType::Mmio32 => {
                    let base = match bar_type {
                        PciBarType::IsolatedIO => PhysicalAddress::new(raw as u64 & 0xFFFF_FFFC),
                        _ => PhysicalAddress::new(raw as u64 & !0x0F),
                    };
                    let bias = bar_type.mask_bias() as u32;
                    Hal::pci().write_pci(reg, u32::MAX);
                    let scale = (Hal::pci().read_pci(reg) & bias).trailing_zeros() as u8;
                    Hal::pci().write_pci(reg, raw);
                    Some(Self {
                        bar_index: PciBarIndex(index as u8),
                        base,
                        bar_type,
                        scale,
                        is_prefetchable,
                    })
                }
                PciBarType::Mmio64 => {
                    let reg_h = reg + 1;
                    let raw_h = Hal::pci().read_pci(reg_h);
                    let base = PhysicalAddress::new(((raw_h as u64) << 32) | (raw as u64 & !0x0F));
                    let bias = bar_type.mask_bias();
                    Hal::pci().write_pci(reg, u32::MAX);
                    Hal::pci().write_pci(reg_h, u32::MAX);
                    let data = (Hal::pci().read_pci(reg) as u64)
                        | ((Hal::pci().read_pci(reg_h) as u64) << 32);
                    let scale = (data & bias).trailing_zeros() as u8;
                    Hal::pci().write_pci(reg, raw);
                    Hal::pci().write_pci(reg_h, raw_h);
                    Some(Self {
                        bar_index: PciBarIndex(index as u8),
                        base,
                        bar_type,
                        scale,
                        is_prefetchable,
                    })
                }
                PciBarType::Mmio1MB | PciBarType::Reserved => None,
            };

            result
        })
    }

    #[inline]
    pub const fn bar_index(&self) -> PciBarIndex {
        self.bar_index
    }

    #[inline]
    pub const fn base(&self) -> PhysicalAddress {
        self.base
    }

    #[inline]
    pub const fn size(&self) -> usize {
        1 << self.scale
    }

    #[inline]
    pub const fn bar_type(&self) -> PciBarType {
        self.bar_type
    }

    /// Returns whether or not this BAR is an x86 isolated IO.
    #[inline]
    pub const fn is_isolated_io(&self) -> bool {
        matches!(self.bar_type, PciBarType::IsolatedIO)
    }

    /// Returns whether or not this BAR is a memory-mapped IO.
    #[inline]
    pub const fn is_mmio(&self) -> bool {
        !self.is_isolated_io()
    }

    #[inline]
    pub const fn is_prefetchable(&self) -> bool {
        self.is_prefetchable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciBarType {
    /// Isolated I/O (x86)
    IsolatedIO,
    /// Any 32bit MMIO
    Mmio32,
    /// Under 1MB MMIO (obsoleted)
    Mmio1MB,
    /// Any 64bit MMIO
    Mmio64,
    /// Reserved
    Reserved,
}

impl PciBarType {
    #[inline]
    pub const fn mask_bias(&self) -> u64 {
        match *self {
            PciBarType::IsolatedIO => !0x03,
            PciBarType::Mmio32 | PciBarType::Mmio1MB | PciBarType::Mmio64 => !0x0F,
            PciBarType::Reserved => 0,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciCapabilityId(pub u8);

impl PciCapabilityId {
    pub const NULL: Self = Self(0x00);
    pub const PM: Self = Self(0x01);
    pub const AGP: Self = Self(0x02);
    pub const VOD: Self = Self(0x03);
    pub const SLOT_ID: Self = Self(0x04);
    pub const MSI: Self = Self(0x05);
    pub const HOT_SWAP: Self = Self(0x06);
    pub const PCI_X: Self = Self(0x07);
    pub const HYPER_TRANSPORT: Self = Self(0x08);
    pub const VENDOR_SPECIFIC: Self = Self(0x09);
    pub const COMPACT_PCI: Self = Self(0x0B);
    pub const HOT_PLUG: Self = Self(0x0B);
    pub const AGP_8X: Self = Self(0x0E);
    pub const PCI_EXPRESS: Self = Self(0x10);
    pub const MSI_X: Self = Self(0x11);
}

impl From<u8> for PciCapabilityId {
    fn from(raw: u8) -> Self {
        Self(raw)
    }
}

/// A type that defines the PCI class code and interface.
///
/// For example, the class code for XHCI (`0x0C_03_30`) is expressed as follows.
/// ```
/// let cc = PciClass::code(0x0C).sub(0x03).interface(0x30);
/// ```
///
/// To see if one class code is included in another class code or interface, compare the following
/// ```
/// let mask = PciClass::code(0x03).sub(0x00);
/// if cc.matches(mask) {
///   // code here
/// }
/// ```
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PciClass(pub u32);

impl PciClass {
    /// Makes an instance from the PCI class code register (0x02)
    #[inline]
    pub const fn from_pci(data: u32) -> Self {
        Self((data & 0xFF_FF_FF_00) | PciClassType::Interface as u32)
    }

    /// Makes an instance from class code.
    #[inline]
    pub const fn code(code: u8) -> Self {
        Self(((code as u32) << 24) | PciClassType::ClassCode as u32)
    }

    /// Chains subclasses to the class code.
    #[inline]
    pub const fn sub(self, sub: u8) -> Self {
        Self(self.0 & 0xFF_00_00_00 | ((sub as u32) << 16) | PciClassType::Subclass as u32)
    }

    /// Chains the programming interface to the class code and subclasses.
    #[inline]
    pub const fn interface(self, interface: u8) -> Self {
        Self(self.0 & 0xFF_FF_00_00 | ((interface as u32) << 8) | PciClassType::Interface as u32)
    }

    #[inline]
    const fn class_type(&self) -> PciClassType {
        PciClassType::from_raw(self.0 & 0xFF)
    }

    #[inline]
    pub const fn raw_data(&self) -> u32 {
        self.0 & self.class_type().mask()
    }

    #[inline]
    pub const fn data(&self) -> u32 {
        self.raw_data() >> 8
    }

    #[inline]
    pub const fn get_class_code(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[inline]
    pub const fn get_sub_class(&self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[inline]
    pub const fn get_interface(&self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Returns whether or not this instance matches the specified class code, subclass, or programming interface.
    #[inline]
    pub const fn matches(&self, other: Self) -> bool {
        match other.class_type() {
            PciClassType::Unspecified => false,
            _ => {
                if self.class_type().mask() < other.class_type().mask() {
                    false
                } else {
                    let mask = other.class_type().mask();
                    (self.0 & mask) == (other.0 & mask)
                }
            }
        }
    }
}

impl fmt::Debug for PciClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // f.debug_tuple("PciClass").field(&self.0).finish()
        match self.class_type() {
            PciClassType::Unspecified => f.write_str("PciClass::Unspecified"),
            PciClassType::ClassCode => {
                f.write_fmt(format_args!("CC_{:02x}", self.get_class_code()))
            }
            PciClassType::Subclass => f.write_fmt(format_args!(
                "CC_{:02x}{:02x}",
                self.get_class_code(),
                self.get_sub_class()
            )),
            PciClassType::Interface => f.write_fmt(format_args!(
                "CC_{:02x}{:02x}{:02x}",
                self.get_class_code(),
                self.get_sub_class(),
                self.get_interface()
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PciClassType {
    Unspecified = 0,
    ClassCode = 1,
    Subclass = 2,
    Interface = 3,
}

impl PciClassType {
    #[inline]
    pub const fn mask(&self) -> u32 {
        match *self {
            PciClassType::Unspecified => 0,
            PciClassType::ClassCode => 0xFF_00_00_00,
            PciClassType::Subclass => 0xFF_FF_00_00,
            PciClassType::Interface => 0xFF_FF_FF_00,
        }
    }

    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::ClassCode,
            2 => Self::Subclass,
            3 => Self::Interface,
            _ => Self::Unspecified,
        }
    }
}
