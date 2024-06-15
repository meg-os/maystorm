use core::mem::transmute;
use paste::paste;

#[allow(unused_imports)]
use core::arch::asm;

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct DescriptorEntry(u64);

impl DescriptorEntry {
    pub const PRESENT: u64 = 0x8000_0000_0000;

    pub const BIG_DATA: u64 = 0x0040_0000_0000_0000;

    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    pub fn flat_code_segment(dpl: DPL, opr_size: DefaultOperandSize) -> DescriptorEntry {
        Self::code_segment(Linear32(0), Limit32::MAX, dpl, opr_size)
    }

    #[inline]
    pub fn code_segment(
        base: Linear32,
        limit: Limit32,
        dpl: DPL,
        opr_size: DefaultOperandSize,
    ) -> DescriptorEntry {
        DescriptorEntry(
            0x0000_1A00_0000_0000u64
                | base.as_segment_base()
                | limit.as_descriptor_entry()
                | Self::PRESENT
                | dpl.as_descriptor_entry()
                | opr_size.as_descriptor_entry(),
        )
    }

    #[inline]
    pub fn flat_data_segment(dpl: DPL) -> DescriptorEntry {
        Self::data_segment(Linear32(0), Limit32::MAX, dpl, true)
    }

    #[inline]
    pub fn data_segment(
        base: Linear32,
        limit: Limit32,
        dpl: DPL,
        is_big_data: bool,
    ) -> DescriptorEntry {
        DescriptorEntry(
            0x0000_1200_0000_0000u64
                | base.as_segment_base()
                | limit.as_descriptor_entry()
                | Self::PRESENT
                | if is_big_data { Self::BIG_DATA } else { 0 }
                | dpl.as_descriptor_entry(),
        )
    }

    #[inline]
    pub fn tss64(base: Linear64, limit: Limit16) -> DescriptorPair {
        let (base_low, base_high) = base.as_segment_base_pair();
        let low = DescriptorEntry(
            DescriptorType::Tss.as_descriptor_entry()
                | base_low
                | limit.as_descriptor_entry()
                | Self::PRESENT,
        );
        let high = DescriptorEntry(base_high);
        DescriptorPair::new(low, high)
    }

    #[inline]
    pub fn gate64(
        offset: Offset64,
        sel: Selector,
        dpl: DPL,
        ty: DescriptorType,
        ist: Option<InterruptStackTable>,
    ) -> DescriptorPair {
        let (offset_low, offset_high) = offset.as_gate_offset_pair();
        let low = DescriptorEntry(
            ty.as_descriptor_entry()
                | offset_low
                | sel.as_descriptor_entry()
                | ist.as_descriptor_entry()
                | dpl.as_descriptor_entry()
                | Self::PRESENT,
        );
        let high = DescriptorEntry(offset_high);

        DescriptorPair::new(low, high)
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_present(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    #[inline]
    pub const fn is_segment(&self) -> bool {
        (self.0 & 0x1000_0000_0000) != 0
    }

    #[inline]
    pub const fn is_code_segment(&self) -> bool {
        self.is_segment() && (self.0 & 0x0800_0000_0000) != 0
    }

    #[inline]
    pub const fn default_operand_size(&self) -> Option<DefaultOperandSize> {
        DefaultOperandSize::from_descriptor(*self)
    }

    #[inline]
    pub const fn dpl(&self) -> DPL {
        DPL::from_descriptor_entry(self.0)
    }
}

pub trait AsDescriptorEntry {
    fn as_descriptor_entry(&self) -> u64;
}

impl<T: AsDescriptorEntry> AsDescriptorEntry for Option<T> {
    fn as_descriptor_entry(&self) -> u64 {
        match self {
            Some(v) => v.as_descriptor_entry(),
            None => 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub struct DescriptorPair {
    pub low: DescriptorEntry,
    pub high: DescriptorEntry,
}

impl DescriptorPair {
    #[inline]
    pub const fn new(low: DescriptorEntry, high: DescriptorEntry) -> Self {
        DescriptorPair { low, high }
    }
}

/// Type of x86 Segment Limit
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit16(pub u16);

impl AsDescriptorEntry for Limit16 {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        self.0 as u64
    }
}

/// Type of x86 Segment Limit
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit32(pub u32);

impl Limit32 {
    pub const MAX: Self = Self(u32::MAX);
}

impl AsDescriptorEntry for Limit32 {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        let limit = self.0;
        if limit > 0xFFFF {
            0x0080_0000_0000_0000
                | ((limit as u64) >> 12) & 0xFFFF
                | ((limit as u64 & 0xF000_0000) << 20)
        } else {
            limit as u64
        }
    }
}

/// Type of 32bit Linear Address
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Linear32(pub u32);

impl Linear32 {
    #[inline]
    pub const fn as_segment_base(&self) -> u64 {
        ((self.0 as u64 & 0x00FF_FFFF) << 16) | ((self.0 as u64 & 0xFF00_0000) << 32)
    }
}

/// Type of 64bit Linear Address
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Linear64(pub u64);

impl Linear64 {
    #[inline]
    pub const fn as_segment_base_pair(&self) -> (u64, u64) {
        let low = Linear32(self.0 as u32).as_segment_base();
        let high = self.0 >> 32;
        (low, high)
    }
}

/// Type of 32bit Offset Address
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Offset32(pub u32);

impl Offset32 {
    #[inline]
    pub const fn as_gate_offset(&self) -> u64 {
        let offset = self.0 as u64;
        (offset & 0xFFFF) | (offset & 0xFFFF_0000) << 32
    }
}

/// Type of 64bit Offset Address
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Offset64(pub u64);

impl Offset64 {
    #[inline]
    pub const fn as_gate_offset_pair(&self) -> (u64, u64) {
        let low = Offset32(self.0 as u32).as_gate_offset();
        let high = self.0 >> 32;
        (low, high)
    }
}

/// Type of x86 Segment Selector
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selector(pub u16);

impl Selector {
    /// The NULL selector that does not contain anything
    pub const NULL: Selector = Selector(0);
    /// Indicates that this selector is an LDT selector
    const TI_LDT: u16 = 0x0004;

    /// Make a new selector from the specified index and RPL
    #[inline]
    pub const fn new(index: u16, rpl: RPL) -> Self {
        Selector((index << 3) | rpl.as_u16())
    }

    /// Make a new LDT selector from the specified index and RPL
    #[inline]
    pub const fn new_local(index: u16, rpl: RPL) -> Self {
        Selector((index << 3) | rpl.as_u16() | Self::TI_LDT)
    }

    /// Returns the requested privilege level in the selector
    #[inline]
    pub const fn rpl(self) -> RPL {
        RPL::from_u16(self.0)
    }

    /// Adjust RPL Field
    #[cfg(target_arch = "x86")]
    #[inline]
    pub fn arpl(self, rhs: RPL) -> Result<Selector, Selector> {
        let result: u16;
        let setnz: u8;
        unsafe {
            asm!("
                arpl {0:x}, {1:x}
                setnz {2}
                ", 
                inout(reg) self.as_u16() => result,
                in(reg) rhs.0 as u16,
                lateout(reg_byte) setnz,
            );
        }
        if setnz == 0 {
            return Ok(Selector(result));
        } else {
            return Err(self);
        }
    }

    /// Returns the index field in the selector
    #[inline]
    pub const fn index(self) -> usize {
        (self.0 >> 3) as usize
    }

    #[inline]
    pub const fn is_global(self) -> bool {
        !self.is_local()
    }

    #[inline]
    pub const fn is_local(self) -> bool {
        (self.0 & Self::TI_LDT) == Self::TI_LDT
    }

    #[inline]
    pub const fn as_u16(&self) -> u16 {
        self.0
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl AsDescriptorEntry for Selector {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        (self.0 as u64) << 16
    }
}

/// DPL, CPL, RPL and IOPL
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrivilegeLevel {
    /// Ring 0, Kernel mode
    Kernel = 0,
    /// Useless in 64bit mode
    _Ring1 = 1,
    /// Useless in 64bit mode
    _Ring2 = 2,
    /// Ring 3, User mode
    User = 3,
}

macro_rules! privilege_level_impl {
    ($( $(#[$meta:meta])* $vis:vis struct $class:ident ; )+) => {
        $(
            #[repr(transparent)]
            #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
            $(#[$meta])*
            $vis struct $class(PrivilegeLevel);

            impl $class {
                $vis const KERNEL: Self = Self(PrivilegeLevel::Kernel);
                $vis const USER: Self = Self(PrivilegeLevel::User);
            }

            paste! {
                $vis const [<$class 0>]: $class = $class::KERNEL;
                $vis const [<$class 3>]: $class = $class::USER;
            }

            impl From<PrivilegeLevel> for $class {
                #[inline]
                fn from(val: PrivilegeLevel) -> Self {
                    Self(val)
                }
            }

            impl From<$class> for PrivilegeLevel {
                #[inline]
                fn from(val: $class) -> Self {
                    val.0
                }
            }
        )*
    };
}

privilege_level_impl! {
    /// Current Priviledge Level
    pub struct CPL;

    /// Descriptor Priviledge Level
    pub struct DPL;

    /// Requested Priviledge Level
    pub struct RPL;

    /// I/O Priviledge Level (Historical use only)
    pub struct IOPL;
}

impl PrivilegeLevel {
    #[inline]
    pub const fn from_usize(value: usize) -> Self {
        match value & 3 {
            0 => PrivilegeLevel::Kernel,
            1 => PrivilegeLevel::_Ring1,
            2 => PrivilegeLevel::_Ring2,
            3 => PrivilegeLevel::User,
            _ => unreachable!(),
        }
    }
}

impl AsDescriptorEntry for DPL {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        (self.0 as u64) << 45
    }
}

impl DPL {
    #[inline]
    pub const fn from_descriptor_entry(val: u64) -> Self {
        Self(PrivilegeLevel::from_usize((val >> 45) as usize))
    }

    #[inline]
    pub fn as_rpl(self) -> RPL {
        RPL(self.0)
    }
}

impl RPL {
    #[inline]
    pub const fn from_u16(val: u16) -> Self {
        Self(PrivilegeLevel::from_usize(val as usize))
    }

    #[inline]
    pub const fn as_u16(self) -> u16 {
        self.0 as u16
    }
}

impl IOPL {
    #[inline]
    pub const fn from_flags(val: usize) -> IOPL {
        IOPL(PrivilegeLevel::from_usize(val >> 12))
    }

    #[inline]
    pub const fn into_flags(self) -> usize {
        (self.0 as usize) << 12
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorType {
    Null = 0,
    Tss = 9,
    TssBusy = 11,
    InterruptGate = 14,
    TrapGate = 15,
}

impl AsDescriptorEntry for DescriptorType {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        let ty = *self as u64;
        ty << 40
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterruptVector(pub u8);

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExceptionType {
    /// #DE
    DivideError = 0,
    /// #DB
    Debug = 1,
    /// NMI
    NonMaskable = 2,
    /// #BP
    Breakpoint = 3,
    /// #OF
    Overflow = 4,
    //Deprecated = 5,
    /// #UD
    InvalidOpcode = 6,
    /// #NM
    DeviceNotAvailable = 7,
    /// #DF
    DoubleFault = 8,
    //Deprecated = 9,
    /// #TS
    InvalidTss = 10,
    /// #NP
    SegmentNotPresent = 11,
    /// #SS
    StackException = 12,
    /// #GP
    GeneralProtection = 13,
    /// #PF
    PageFault = 14,
    //Unavailable = 15,
    /// #MF
    FloatingPointException = 16,
    /// #AC
    AlignmentCheck = 17,
    /// #MC
    MachineCheck = 18,
    /// #XM
    SimdException = 19,
    /// #VE
    Virtualization = 20,
    /// #CP
    ControlProtection = 21,
    //Reserved
    /// #SX
    Security = 30,
    //Reserved = 31,
    MAX = 32,
}

impl ExceptionType {
    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(self as u8)
    }

    #[inline]
    pub const unsafe fn from_vec(vec: InterruptVector) -> Self {
        transmute(vec.0)
    }

    #[inline]
    pub const fn has_error_code(&self) -> bool {
        match self {
            ExceptionType::DoubleFault
            | ExceptionType::InvalidTss
            | ExceptionType::SegmentNotPresent
            | ExceptionType::StackException
            | ExceptionType::GeneralProtection
            | ExceptionType::PageFault
            | ExceptionType::AlignmentCheck
            | ExceptionType::Security => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn mnemonic(&self) -> &'static str {
        match self {
            ExceptionType::DivideError => "#DE",
            ExceptionType::Debug => "#DB",
            ExceptionType::NonMaskable => "NMI",
            ExceptionType::Breakpoint => "#BP",
            ExceptionType::Overflow => "#OV",
            ExceptionType::InvalidOpcode => "#UD",
            ExceptionType::DeviceNotAvailable => "#NM",
            ExceptionType::DoubleFault => "#DF",
            ExceptionType::InvalidTss => "#TS",
            ExceptionType::SegmentNotPresent => "#NP",
            ExceptionType::StackException => "#SS",
            ExceptionType::GeneralProtection => "#GP",
            ExceptionType::PageFault => "#PF",
            ExceptionType::FloatingPointException => "#MF",
            ExceptionType::AlignmentCheck => "#AC",
            ExceptionType::MachineCheck => "#MC",
            ExceptionType::SimdException => "#XM",
            ExceptionType::Virtualization => "#VE",
            ExceptionType::Security => "#SX",
            _ => "",
        }
    }
}

impl From<ExceptionType> for InterruptVector {
    #[inline]
    fn from(ex: ExceptionType) -> Self {
        InterruptVector(ex as u8)
    }
}

#[repr(C, packed)]
#[derive(Default)]
pub struct TaskStateSegment64 {
    _reserved_1: u32,
    pub stack_pointer: [u64; 3],
    _reserved_2: [u32; 2],
    pub ist: [u64; 7],
    _reserved_3: [u32; 2],
    pub iomap_base: u16,
}

impl !Send for TaskStateSegment64 {}

impl TaskStateSegment64 {
    pub const OFFSET_RSP0: usize = 0x04;

    pub const LIMIT: u16 = 0x67;

    #[inline]
    pub const fn new() -> Self {
        Self {
            _reserved_1: 0,
            stack_pointer: [0; 3],
            _reserved_2: [0, 0],
            ist: [0; 7],
            _reserved_3: [0, 0],
            iomap_base: 0,
        }
    }

    #[inline]
    pub fn as_descriptor_pair(&self) -> DescriptorPair {
        DescriptorEntry::tss64(
            Linear64(self as *const _ as usize as u64),
            Limit16(Self::LIMIT),
        )
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DefaultOperandSize {
    Use16 = 0x0000_0000_0000_0000,
    Use32 = 0x0040_0000_0000_0000,
    Use64 = 0x0020_0000_0000_0000,
}

pub const USE16: DefaultOperandSize = DefaultOperandSize::Use16;

pub const USE32: DefaultOperandSize = DefaultOperandSize::Use32;

pub const USE64: DefaultOperandSize = DefaultOperandSize::Use64;

impl AsDescriptorEntry for DefaultOperandSize {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        *self as u64
    }
}

impl DefaultOperandSize {
    #[inline]
    pub const fn as_descriptor_entry(&self) -> u64 {
        *self as u64
    }

    #[inline]
    pub const fn from_descriptor(value: DescriptorEntry) -> Option<Self> {
        if value.is_code_segment() {
            let is_32 = (value.0 & USE32.as_descriptor_entry()) != 0;
            let is_64 = (value.0 & USE64.as_descriptor_entry()) != 0;
            match (is_32, is_64) {
                (false, false) => Some(USE16),
                (false, true) => Some(USE64),
                (true, false) => Some(USE32),
                (true, true) => None,
            }
        } else {
            None
        }
    }
}

impl TryFrom<DescriptorEntry> for DefaultOperandSize {
    type Error = ();
    fn try_from(value: DescriptorEntry) -> Result<Self, Self::Error> {
        Self::from_descriptor(value).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptStackTable {
    IST1 = 1,
    IST2,
    IST3,
    IST4,
    IST5,
    IST6,
    IST7,
}

macro_rules! ist_impl {
    ($( $ist:ident , )*) => {
        $(
            pub const $ist: InterruptStackTable = InterruptStackTable::$ist;
        )*
    };
}

ist_impl!(IST1, IST2, IST3, IST4, IST5, IST6, IST7,);

impl AsDescriptorEntry for InterruptStackTable {
    #[inline]
    fn as_descriptor_entry(&self) -> u64 {
        (*self as u64) << 32
    }
}
