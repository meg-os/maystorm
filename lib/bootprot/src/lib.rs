//! MEG-OS Boot Procotol

#![no_std]

use core::fmt;

#[repr(C)]
pub struct BootInfo {
    pub platform: PlatformType,
    pub color_mode: ColorMode,
    pub screen_width: u16,
    pub screen_height: u16,
    pub vram_stride: u16,
    pub vram_base: u64,
    pub master_page_table: u64,
    pub acpi_rsdptr: u64,
    pub dtb: u64,
    pub smbios: u64,
    pub kernel_base: u64,
    pub total_memory_size: u64,
    pub cmdline: u64,
    pub initrd_base: u32,
    pub initrd_size: u32,
    pub mmap_base: u32,
    pub mmap_len: u32,
    pub real_bitmap: [u32; 8],
    pub flags: BootFlags,
}

impl Default for BootInfo {
    #[inline]
    fn default() -> Self {
        Self {
            platform: Default::default(),
            color_mode: Default::default(),
            screen_width: Default::default(),
            screen_height: Default::default(),
            vram_stride: Default::default(),
            vram_base: Default::default(),
            master_page_table: Default::default(),
            acpi_rsdptr: Default::default(),
            dtb: Default::default(),
            smbios: Default::default(),
            kernel_base: Default::default(),
            total_memory_size: Default::default(),
            cmdline: Default::default(),
            initrd_base: Default::default(),
            initrd_size: Default::default(),
            mmap_base: Default::default(),
            mmap_len: Default::default(),
            real_bitmap: Default::default(),
            flags: Default::default(),
        }
    }
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformType {
    #[default]
    Unspecified = 0,
    /// IA32-Legacy NEC PC-98 Series Computer
    Nec98 = 1,
    /// IA32-Legacy IBM PC Compatible
    PcCompatible = 2,
    /// IA32-Legacy Fujitsu FM TOWNS
    FmTowns = 3,
    /// Native UEFI
    UefiNative = 4,
    /// Non native UEFI
    UefiBridged = 5,
}

impl fmt::Display for PlatformType {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PcCompatible => write!(f, "PC Compatible"),
            Self::Nec98 => write!(f, "PC-98"),
            Self::FmTowns => write!(f, "FM TOWNS"),
            Self::UefiNative => write!(f, "UEFI"),
            Self::UefiBridged => write!(f, "UEFI"),
            _ => write!(f, "Unknown"),
        }
    }
}

#[non_exhaustive]
#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorMode {
    #[default]
    Unspecified = 0,
    /// 8bit Indexed Color Mode
    Indexed8 = 8,
    /// 32bit Color (Little Endian B-G-R-A, VESA, UEFI)
    Argb32 = 32,
    // 32bit Color (Big Endian R-G-B-A)
    Abgr32 = 33,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BootFlags(u32);

impl BootFlags {
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
}

impl Default for BootFlags {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootMemoryMapDescriptor {
    pub base: u64,
    pub page_count: u32,
    pub mem_type: BootMemoryType,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootMemoryType {
    Available,
    AcpiReclaim,
    AcpiNonVolatile,
    Mmio,
    MmioPortSpace,
    OsLoaderCode,
    OsLoaderData,
    FirmwareCode,
    FirmwareData,
    Reserved,
    Unavailable,
}
