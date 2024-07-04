//! xHCI MMIO Registers

use super::*;
use crate::drivers::usb::*;
use crate::mem::{
    mmio::{MmioRegU32, MmioRegU64},
    MemoryManager,
};
use crate::*;
use core::ffi::c_void;
use core::mem::{size_of, transmute};
use core::num::{NonZeroU8, NonZeroUsize};
use core::slice;

/// xHC Capability Registers
#[repr(C)]
#[allow(dead_code)]
pub struct CapabilityRegisters {
    caplength: MmioRegU32,
    hcsparams1: MmioRegU32,
    hcsparams2: MmioRegU32,
    hcsparams3: MmioRegU32,
    hccparams1: MmioRegU32,
    dboff: MmioRegU32,
    rtsoff: MmioRegU32,
    hccparams2: MmioRegU32,
}

impl CapabilityRegisters {
    #[inline]
    pub fn length(&self) -> usize {
        (self.caplength.read_volatile() & 0xFF) as usize
    }

    #[inline]
    pub fn version(&self) -> (usize, usize, usize) {
        let ver = self.caplength.read_volatile() >> 16;
        let ver2 = (ver & 0x0F) as usize;
        let ver1 = ((ver >> 4) & 0x0F) as usize;
        let ver0 = (ver >> 8) as usize;
        (ver0, ver1, ver2)
    }

    #[inline]
    pub fn hcs_params1(&self) -> u32 {
        self.hcsparams1.read_volatile()
    }

    #[inline]
    pub fn hcs_params2(&self) -> u32 {
        self.hcsparams2.read_volatile()
    }

    #[inline]
    pub fn hcs_params3(&self) -> u32 {
        self.hcsparams3.read_volatile()
    }

    #[inline]
    pub fn hcc_params1(&self) -> HccParams1 {
        HccParams1::from_bits_retain(self.hccparams1.read_volatile())
    }

    #[inline]
    pub fn hcc_params2(&self) -> u32 {
        self.hccparams2.read_volatile()
    }

    #[inline]
    pub fn db_off(&self) -> usize {
        (self.dboff.read_volatile() & !0x03) as usize
    }

    #[inline]
    pub fn rts_off(&self) -> usize {
        (self.rtsoff.read_volatile() & !0x1F) as usize
    }

    #[inline]
    pub fn max_device_slots(&self) -> usize {
        (self.hcs_params1() & 0xFF) as usize
    }

    #[inline]
    pub fn max_interrups(&self) -> usize {
        ((self.hcs_params1() >> 8) & 0x3FF) as usize
    }

    #[inline]
    pub fn max_ports(&self) -> usize {
        ((self.hcs_params1() >> 24) & 0xFF) as usize
    }

    #[inline]
    pub fn max_scratchpad_size(&self) -> usize {
        let hcs_params2 = self.hcs_params2();
        (((hcs_params2 >> 27) & 0x1F) | (((hcs_params2 >> 21) & 0x1F) << 5)) as usize
    }

    #[inline]
    pub fn opr(&self) -> &'static OperationalRegisters {
        unsafe { transmute((self as *const _ as *const c_void).add(self.length())) }
    }

    #[inline]
    pub fn ports(&self) -> &'static [PortRegisters] {
        unsafe {
            let data: *const PortRegisters =
                transmute((self as *const _ as *const c_void).add(self.length() + 0x400));
            let len = self.max_ports();
            slice::from_raw_parts(data, len)
        }
    }

    #[inline]
    pub fn doorbells(&self) -> &'static [DoorbellRegister] {
        unsafe {
            let data: *const DoorbellRegister =
                transmute((self as *const _ as *const c_void).add(self.db_off()));
            let len = self.max_device_slots();
            slice::from_raw_parts(data, len)
        }
    }

    #[inline]
    pub fn rts(&self) -> &'static RuntimeRegisters {
        unsafe { transmute((self as *const _ as *const c_void).add(self.rts_off())) }
    }

    #[inline]
    pub fn xecp(&self) -> Option<NonZeroUsize> {
        let xecp = self.hcc_params1().xecp();
        if xecp > 0 {
            NonZeroUsize::new((self as *const _ as usize) + (xecp * 4))
        } else {
            None
        }
    }
}

my_bitflags! {
    /// Host Controller Capability Parameters 1
    pub struct HccParams1: u32 {}
}

impl HccParams1 {
    /// 64bit Addressing Capability
    pub const AC64: Self = Self(0b0000_0000_0000_0001);
    /// BW Negotiation Capability
    pub const BNC: Self = Self(0b0000_0000_0000_0010);
    /// Context Size
    pub const CSZ: Self = Self(0b0000_0000_0000_0100);
    /// Port Power Control
    pub const PPC: Self = Self(0b0000_0000_0000_1000);
    /// Port Indicators
    pub const PIND: Self = Self(0b0000_0000_0001_0000);
    /// Light HC Reset Capability
    pub const LHRC: Self = Self(0b0000_0000_0010_0000);
    /// Latency Tolerance Messaging Capability
    pub const LTC: Self = Self(0b0000_0000_0100_0000);
    /// No Secondary SID Support
    pub const NSS: Self = Self(0b0000_0000_1000_0000);
    /// Parse All Event Data
    pub const PAE: Self = Self(0b0000_0001_0000_0000);
    /// Stopped - Short Packet Capacility
    pub const SPC: Self = Self(0b0000_0010_0000_0000);
    /// Stopped EDTLA Capability
    pub const SEC: Self = Self(0b0000_0100_0000_0000);
    /// Contiguous Frame ID Capability
    pub const CFC: Self = Self(0b0000_1000_0000_0000);
    /// Maximum Primary Stream Array Size
    pub const MAX_PSA_SIZE: Self = Self(0b1111_0000_0000_0000);

    pub const XECP: Self = Self(0xFFFF_0000);
}

impl HccParams1 {
    #[inline]
    pub const fn max_psa_size(&self) -> usize {
        ((self.bits() & Self::MAX_PSA_SIZE.bits()) >> 12) as usize
    }

    #[inline]
    pub const fn xecp(&self) -> usize {
        ((self.bits() & Self::XECP.bits()) >> 16) as usize
    }
}

/// xHC Operational Registers
#[repr(C)]
#[allow(dead_code)]
pub struct OperationalRegisters {
    usbcmd: MmioRegU32,
    usbsts: MmioRegU32,
    pagesize: MmioRegU32,
    _rsrv1: [u32; 2],
    dnctrl: MmioRegU32,
    crcr: MmioRegU64,
    _rsrv2: [u32; 4],
    dcbaap: MmioRegU64,
    config: MmioRegU32,
}

impl OperationalRegisters {
    #[inline]
    pub fn page_size_raw(&self) -> u32 {
        self.pagesize.read_volatile() & 0xFFFF
    }

    #[inline]
    pub fn page_size(&self) -> usize {
        let bitmap = self.page_size_raw() & 0xFFFF;
        1usize << (12 + bitmap.trailing_zeros())
    }

    #[inline]
    pub fn read_cmd(&self) -> UsbCmd {
        UsbCmd::from_bits_retain(self.usbcmd.read_volatile())
    }

    #[inline]
    pub fn write_cmd(&self, val: UsbCmd) {
        self.usbcmd.write_volatile(val.bits());
    }

    #[inline]
    pub fn set_cmd(&self, val: UsbCmd) {
        self.write_cmd(self.read_cmd() | val);
    }

    #[inline]
    pub fn status(&self) -> UsbSts {
        UsbSts::from_bits_retain(self.usbsts.read_volatile())
    }

    #[inline]
    pub fn reset_status(&self, val: UsbSts) {
        self.usbsts.write_volatile(val.bits());
    }

    #[inline]
    pub fn set_crcr(&self, val: NonNullPhysicalAddress) {
        self.crcr.write_volatile(val.get().as_u64());
    }

    #[inline]
    pub fn dcbaap(&self) -> PhysicalAddress {
        self.dcbaap.read_volatile().into()
    }

    #[inline]
    pub fn set_dcbaap(&self, val: NonNullPhysicalAddress) {
        self.dcbaap.write_volatile(val.get().as_u64());
    }

    #[inline]
    pub unsafe fn set_config(&self, max_dev_slot: usize, u3e: bool, cie: bool) {
        let val = (max_dev_slot & 0xFF) as u32
            | if u3e { 0x100 } else { 0 }
            | if cie { 0x200 } else { 0 };
        self.config.write_volatile(val);
    }

    #[inline]
    pub fn device_notification_bitmap(&self) -> DeviceNotificationBitmap {
        DeviceNotificationBitmap::from_bits_retain(self.dnctrl.read_volatile())
    }

    #[inline]
    pub unsafe fn set_device_notification_bitmap(&self, bitmap: DeviceNotificationBitmap) {
        self.dnctrl.write_volatile(bitmap.bits());
    }
}

my_bitflags! {
    /// USBCMD: Usb Command Register
    #[allow(dead_code)]
    pub struct UsbCmd: u32 {}
}

impl UsbCmd {
    /// Run(1)/Stop(0)
    pub const RUN: Self = Self(0b0000_0000_0000_0001);
    /// Host Controller Reset
    pub const HCRST: Self = Self(0b0000_0000_0000_0010);
    /// Interrupt Enable
    pub const INTE: Self = Self(0b0000_0000_0000_0100);

    // TODO: and so on...
}

my_bitflags! {
    /// USBSTS: USB Status Register
    #[allow(dead_code)]
    pub struct UsbSts: u32 {}
}

impl UsbSts {
    /// HC Halted
    pub const HCH: Self = Self(0b0000_0000_0000_0001);

    /// Controller Not Ready
    pub const CNR: Self = Self(0b0000_1000_0000_0000);

    // TODO: and so on...
}

my_bitflags! {
    /// Device Notification
    pub struct DeviceNotificationBitmap: u32 {
    }
}

impl DeviceNotificationBitmap {
    pub const FUNCTION_WAKE: Self = Self(0b0000_0000_0000_0010);
}

/// xHC USB Port Register Set
#[repr(C)]
#[allow(dead_code)]
pub struct PortRegisters {
    portsc: MmioRegU32,
    portpmsc: MmioRegU32,
    portli: MmioRegU32,
    porthlpmc: MmioRegU32,
}

impl PortRegisters {
    #[inline]
    pub fn status(&self) -> PortSc {
        PortSc::from_bits_retain(self.portsc.read_volatile())
    }

    #[inline]
    pub fn set(&self, val: PortSc) {
        self.write((self.status() & PortSc::PRESERVE_MASK) | val);
    }

    #[inline]
    pub fn clear_changes(&self) {
        self.set(PortSc::ALL_CHANGE_BITS);
    }

    #[inline]
    pub fn power_off(&self) {
        self.write(
            ((self.status() & PortSc::PRESERVE_MASK) | PortSc::ALL_CHANGE_BITS) & !PortSc::PP,
        );
    }

    #[inline]
    pub fn write(&self, val: PortSc) {
        self.portsc.write_volatile(val.bits());
    }
}

my_bitflags! {
    /// Port Status and Control Register
    pub struct PortSc: u32 {}
}

impl PortSc {
    /// A magic word to preserve mask
    pub const PRESERVE_MASK: Self = Self(0x0E00C3E0);

    /// (ROS) Current Connect Status
    pub const CCS: Self = Self(0x0000_0001);
    /// (RW1CS) Port Enabled
    pub const PED: Self = Self(0x0000_0002);
    /// (RO) Over current Active
    pub const OCA: Self = Self(0x0000_0008);
    /// (RW1S) Port Reset
    pub const PR: Self = Self(0x0000_0010);
    /// (RWS) Port Link State
    pub const PLS: Self = Self(0x0000_01E0);
    /// (RWS) Port Power
    pub const PP: Self = Self(0x0000_0200);
    /// (ROW) Port Speed
    pub const SPEED: Self = Self(0x0000_3C00);
    /// (RWS) Port Indicator
    pub const PIC: Self = Self(0x0000_C000);
    /// (RW) Port Link State Write Strobe
    pub const LWS: Self = Self(0x0001_0000);

    pub const ALL_CHANGE_BITS: Self = Self(0x00FE_0000);
    /// (RW1CS) Connect Status Change
    pub const CSC: Self = Self(0x0002_0000);
    /// (RW1CS) Port Enabled/Disabled Change
    pub const PEC: Self = Self(0x0004_0000);
    /// (RW1CS) Warm Port Reset Change
    pub const WRC: Self = Self(0x0008_0000);
    /// (RW1CS) Over current Change
    pub const OCC: Self = Self(0x0010_0000);
    /// (RW1CS) Port Reset Change
    pub const PRC: Self = Self(0x0020_0000);
    /// (RW1CS) Port Link State Change
    pub const PLC: Self = Self(0x0040_0000);
    /// (RW1CS) Port Config Error Change
    pub const CEC: Self = Self(0x0080_0000);

    /// (RO) Cold Attach Status
    pub const CAS: Self = Self(0x0100_0000);
    /// (RWS) Wake on Connect Enable
    pub const WCE: Self = Self(0x0200_0000);
    /// (RWS) Wake on Disconnect Enable
    pub const WDE: Self = Self(0x0400_0000);
    /// (RWS) Wake on Over current Enable
    pub const WOE: Self = Self(0x0800_0000);
    /// (RO) Device Removable
    pub const DR: Self = Self(0x4000_0000);
    /// (RW1S) Warm Port Reset
    pub const WPR: Self = Self(0x8000_0000);

    #[inline]
    pub const fn is_connected_status_changed(&self) -> bool {
        self.contains(Self::CSC)
    }

    #[inline]
    pub const fn is_connected(&self) -> bool {
        self.contains(Self::CCS)
    }

    #[inline]
    pub const fn is_enabled(&self) -> bool {
        self.contains(Self::PED)
    }

    #[inline]
    pub const fn is_disabled(&self) -> bool {
        !self.is_enabled()
    }

    #[inline]
    pub const fn is_powered(&self) -> bool {
        self.contains(Self::PP)
    }

    #[inline]
    pub const fn has_changes(&self) -> bool {
        (self.bits() & Self::ALL_CHANGE_BITS.bits()) != 0
    }

    #[inline]
    pub const fn link_state_raw(&self) -> usize {
        ((self.bits() & Self::PLS.bits()) as usize) >> 5
    }

    #[inline]
    pub const fn speed_raw(&self) -> usize {
        ((self.bits() & Self::SPEED.bits()) as usize) >> 10
    }

    #[inline]
    pub const fn port_indicator_raw(&self) -> usize {
        ((self.bits() & Self::PIC.bits()) as usize) >> 14
    }

    #[inline]
    pub const fn speed(&self) -> Option<PSIV> {
        unsafe { transmute(self.speed_raw() as u8) }
    }

    #[inline]
    pub const fn link_state(&self) -> Option<Usb3LinkState> {
        unsafe { transmute(self.link_state_raw() as u8) }
    }

    #[inline]
    pub const fn is_usb2(&self) -> bool {
        match self.speed() {
            Some(PSIV::LS) | Some(PSIV::FS) | Some(PSIV::HS) => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_usb3(&self) -> bool {
        match self.speed() {
            Some(PSIV::LS) | Some(PSIV::FS) | Some(PSIV::HS) => false,
            Some(_) => true,
            _ => false,
        }
    }
}

/// xHC Runtime Registers
#[repr(C)]
pub struct RuntimeRegisters {
    mfindex: MmioRegU32,
    _rsrv1: [u32; 7],
    irs: [InterrupterRegisterSet; 1],
}

impl RuntimeRegisters {
    #[inline]
    pub fn mf_index(&self) -> u32 {
        self.mfindex.read_volatile() & 0x3FFF
    }

    #[inline]
    pub fn primary_irs(&self) -> &InterrupterRegisterSet {
        unsafe { self.irs.get_unchecked(0) }
    }

    #[inline]
    pub fn irs(&self, index: usize) -> Option<&InterrupterRegisterSet> {
        self.irs.get(index)
    }
}

/// xHC Interrupter Register Set
#[repr(C)]
#[allow(dead_code)]
pub struct InterrupterRegisterSet {
    iman: MmioRegU32,
    imod: MmioRegU32,
    erstsz: MmioRegU32,
    _rsrv: MmioRegU32,
    erstba: MmioRegU64,
    erdp: MmioRegU64,
}

impl InterrupterRegisterSet {
    pub const SIZE_EVENT_RING: usize = MemoryManager::PAGE_SIZE_MIN / size_of::<Trb>();

    pub unsafe fn init(&self, initial_dp: PhysicalAddress, len: usize) {
        let count = 1;
        let (base, erst) = MemoryManager::alloc_dma(count).unwrap();
        *erst = EventRingSegmentTableEntry::new(initial_dp, len as u16);
        self.erstsz.write_volatile(count as u32);
        self.erdp.write_volatile(initial_dp.as_u64());
        self.erstba.write_volatile(base.as_u64());
    }

    #[inline]
    pub fn set_iman(&self, val: u32) {
        self.iman.write_volatile(val);
    }

    pub fn dequeue_event<'a>(&'a self, event_cycle: &'a CycleBit) -> Option<&'a Trb> {
        let erdp = PhysicalAddress::from(self.erdp.read_volatile());
        let cycle = event_cycle.value();
        let event = unsafe { &*(erdp & !15).direct_map::<Trb>() };
        if event.cycle_bit() == cycle {
            let er_base = erdp & !0xFFF;
            let mut index = 1 + (erdp - er_base) / size_of::<Trb>();
            if index == InterrupterRegisterSet::SIZE_EVENT_RING {
                index = 0;
                event_cycle.toggle();
            }
            let new_erdp = er_base.as_u64() | (index * size_of::<Trb>()) as u64 | 8;
            self.erdp.write_volatile(new_erdp);

            Some(event)
        } else {
            None
        }
    }
}

/// xHC Doorbell Register
#[repr(transparent)]
pub struct DoorbellRegister(MmioRegU32);

impl DoorbellRegister {
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.read_volatile()
    }

    #[inline]
    pub fn set_raw(&self, val: u32) {
        self.0.write_volatile(val);
    }

    #[inline]
    pub fn target(&self) -> Option<DCI> {
        NonZeroU8::new((self.raw() & 0xFF) as u8).map(|v| DCI(v))
    }

    #[inline]
    pub fn set_target(&self, val: Option<DCI>) {
        self.set_raw(val.map(|v| v.0.get() as u32).unwrap_or_default());
    }
}
