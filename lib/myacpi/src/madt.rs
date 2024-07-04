use super::*;
use core::mem::transmute;

/// Multiple APIC Description Table
#[repr(C, packed)]
#[allow(unused)]
pub struct Madt {
    hdr: AcpiHeader,
    local_apic_address: u32,
    flags: u32,
    // TODO:
}

unsafe impl AcpiTable for Madt {
    const TABLE_ID: TableId = TableId::MADT;
}

impl Madt {
    #[inline]
    pub const fn local_apic_address(&self) -> u32 {
        self.local_apic_address
    }

    #[inline]
    pub const fn has_8259(&self) -> bool {
        (self.flags & 0x0000_0001) != 0
    }

    #[inline]
    pub const fn raw_entries(&self) -> impl Iterator<Item = &EntryHeader> {
        MadtEntries {
            madt: self,
            index: 0,
        }
    }

    #[inline]
    pub fn entries<'a, T: RawEntry + 'a>(&'a self) -> impl Iterator<Item = &'a T> {
        self.raw_entries().filter_map(|v| v.assume())
    }

    #[inline]
    pub fn all_entries(&self) -> impl Iterator<Item = MadtEntry> {
        self.raw_entries().map(|v| MadtEntry::from_raw(v))
    }

    #[inline]
    pub fn local_apics(&self) -> impl Iterator<Item = &LocalApic> {
        self.entries::<LocalApic>().filter(|v| v.is_available())
    }

    #[inline]
    pub fn local_x2apics(&self) -> impl Iterator<Item = &LocalX2Apic> {
        self.entries::<LocalX2Apic>().filter(|v| v.is_available())
    }
}

impl Xsdt {
    pub fn local_apics(&self) -> impl Iterator<Item = &LocalApic> {
        self.find::<Madt>()
            .take(1)
            .flat_map(|v| v.entries::<LocalApic>())
            .filter(|v| v.is_available())
    }
}

struct MadtEntries<'a> {
    madt: &'a Madt,
    index: usize,
}

impl<'a> Iterator for MadtEntries<'a> {
    type Item = &'a EntryHeader;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = 44 + self.index;
        if offset >= self.madt.header().len() {
            None
        } else {
            let entry = unsafe {
                &*((self.madt as *const _ as *const c_void).add(offset) as *const EntryHeader)
            };
            self.index += entry.len();
            Some(entry)
        }
    }
}

/// Interrupt Controller Structure
#[repr(C)]
pub struct EntryHeader {
    entry_type: EntryType,
    len: u8,
}

impl EntryHeader {
    #[inline]
    pub const fn entry_type(&self) -> EntryType {
        self.entry_type
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn assume<T: RawEntry + Sized>(&self) -> Option<&T> {
        (self.entry_type() == T::ENTRY_TYPE).then(|| unsafe { transmute(self) })
    }
}

/// Interrupt Controller Structure Types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EntryType {
    /// Processor Local APIC
    LocalApic = 0,
    /// I/O APIC
    IoApic,
    /// Interrupt Source Override
    InterruptSourceOverride,
    /// Non-maskable Interrupt (NMI) Source
    NmiSource,
    /// Local APIC NMI
    LocalApicNmi,
    /// Local APIC Address Override
    LocalApicAddressOverride,
    /// I/O SAPIC
    IoSapic,
    /// Local SAPIC
    LocalSapic,
    /// Platform Interrupt Sources
    PlatformInterruptSources,
    /// Processor Local x2APIC
    LocalX2Apic,
    /// Local x2APIC NMI
    LocalX2ApicNmi,
    /// GIC CPU Interface (GICC)
    Gicc,
    /// GIC Distributor (GICD)
    Gicd,
    /// GIC MSI Frame
    GicMsiFrame,
    /// GIC Redistributor (GICR)
    GicRedistributor,
    /// GIC Interrupt Translation Service (ITS)
    GicInterruptTranslationService,
    /// Multiprocessor Wakeup
    MultiprocessorWakeup,
    /// Core Programmable Interrupt Controller (CORE PIC)
    CorePic,
    /// Legacy I/O Programmable Interrupt Controller (LIO PIC)
    LioPic,
    /// HyperTransport Programmable Interrupt Controller (HT PIC)
    HtPic,
    /// Extend I/O Programmable Interrupt Controller (EIO PIC)
    EioPic,
    /// MSI Programmable Interrupt Controller (MSI PIC)
    MsiPic,
    /// Bridge I/O Programmable Interrupt Controller (BIO PIC)
    BioPic,
    /// Low Pin Count Programmable Interrupt Controller (LPC PIC)
    LpcPic,
}

#[non_exhaustive]
pub enum MadtEntry<'a> {
    /// Processor Local APIC
    LocalApic(&'a LocalApic),
    /// I/O APIC
    IoApic(&'a IoApic),
    /// Interrupt Source Override
    InterruptSourceOverride(&'a InterruptSourceOverride),
    /// Processor Local x2APIC
    LocalX2Apic(&'a LocalX2Apic),

    Other(&'a EntryHeader),
}

impl<'a> MadtEntry<'a> {
    fn from_raw(raw: &'a EntryHeader) -> MadtEntry<'a> {
        if let Some(lapic) = raw.assume::<LocalApic>() {
            Self::LocalApic(lapic)
        } else if let Some(ioapic) = raw.assume::<IoApic>() {
            Self::IoApic(ioapic)
        } else if let Some(iso) = raw.assume::<InterruptSourceOverride>() {
            Self::InterruptSourceOverride(iso)
        } else if let Some(lapic) = raw.assume::<LocalX2Apic>() {
            Self::LocalX2Apic(lapic)
        } else {
            Self::Other(raw)
        }
    }
}

pub unsafe trait RawEntry {
    const ENTRY_TYPE: EntryType;
}

/// Processor Local APIC Structure
#[repr(C, packed)]
pub struct LocalApic {
    _hdr: EntryHeader,
    uid: u8,
    apic_id: u8,
    flags: u32,
}

unsafe impl RawEntry for LocalApic {
    const ENTRY_TYPE: EntryType = EntryType::LocalApic;
}

impl LocalApic {
    #[inline]
    pub const fn uid(&self) -> u8 {
        self.uid
    }

    #[inline]
    pub const fn apic_id(&self) -> u8 {
        self.apic_id
    }

    #[inline]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[inline]
    pub const fn status(&self) -> ApicStatus {
        unsafe { transmute(self.flags & 0x0000_0003) }
    }

    #[inline]
    pub const fn is_available(&self) -> bool {
        match self.status() {
            ApicStatus::Enabled => true,
            _ => false,
        }
    }
}

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApicStatus {
    /// This processor is unusable.
    Unusable = 0,
    /// This processor is ready for use.
    Enabled = 1,
    /// The system hardware supports enabling this processor while the OS is running.
    Usable = 2,
    // _Reserved,
}

/// I/O APIC
#[repr(C, packed)]
pub struct IoApic {
    _hdr: EntryHeader,
    apic_id: u8,
    _reserved: u8,
    io_apic_address: u32,
    gsi_base: u32,
}

unsafe impl RawEntry for IoApic {
    const ENTRY_TYPE: EntryType = EntryType::IoApic;
}

impl IoApic {
    #[inline]
    pub const fn apic_id(&self) -> u8 {
        self.apic_id
    }

    #[inline]
    pub const fn io_apic_address(&self) -> u32 {
        self.io_apic_address
    }

    #[inline]
    pub const fn gsi_base(&self) -> u32 {
        self.gsi_base
    }
}

/// Interrupt Source Override
#[repr(C, packed)]
pub struct InterruptSourceOverride {
    _hdr: EntryHeader,
    bus: u8,
    source: u8,
    global_system_interrupt: u32,
    flags: u16,
}

unsafe impl RawEntry for InterruptSourceOverride {
    const ENTRY_TYPE: EntryType = EntryType::InterruptSourceOverride;
}

impl InterruptSourceOverride {
    #[inline]
    pub const fn bus(&self) -> u8 {
        self.bus
    }

    #[inline]
    pub const fn source(&self) -> u8 {
        self.source
    }

    #[inline]
    pub const fn global_system_interrupt(&self) -> u32 {
        self.global_system_interrupt
    }

    #[inline]
    pub const fn flags(&self) -> u16 {
        self.flags
    }
}

/// Processor Local x2APIC Structure
#[repr(C, packed)]
pub struct LocalX2Apic {
    _hdr: EntryHeader,
    _reserved: [u8; 2],
    apic_id: u32,
    flags: u32,
    uid: u32,
}

unsafe impl RawEntry for LocalX2Apic {
    const ENTRY_TYPE: EntryType = EntryType::LocalX2Apic;
}

impl LocalX2Apic {
    #[inline]
    pub const fn uid(&self) -> u32 {
        self.uid
    }

    #[inline]
    pub const fn apic_id(&self) -> u32 {
        self.apic_id
    }

    #[inline]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[inline]
    pub const fn status(&self) -> ApicStatus {
        unsafe { transmute(self.flags & 0x0000_0003) }
    }

    #[inline]
    pub const fn is_available(&self) -> bool {
        match self.status() {
            ApicStatus::Enabled => true,
            _ => false,
        }
    }
}
