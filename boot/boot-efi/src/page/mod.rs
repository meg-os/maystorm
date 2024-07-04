use bootprot::*;
use core::ops::*;
use uefi::table::boot::*;

cfg_match! {
    cfg(any(target_arch = "x86_64", target_arch = "x86")) => {
        mod amd64;
        pub use amd64::*;
    }
}

/// Virtual page size as defined by the UEFI specification (not the actual page size)
pub const UEFI_PAGE_SIZE: u64 = 0x1000;

type IntPtr = u64;
pub type PhysicalAddress = u64;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct VirtualAddress(pub IntPtr);

impl VirtualAddress {
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

impl Add<u32> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u32) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Add<usize> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self {
        VirtualAddress(self.0 - rhs as IntPtr)
    }
}

pub trait MemoryTypeHelper {
    fn is_available_at_runtime(&self) -> bool;
    fn is_countable(&self) -> bool;
    fn as_boot_memory_type(&self) -> BootMemoryType;
}

impl MemoryTypeHelper for MemoryType {
    #[inline]
    fn is_available_at_runtime(&self) -> bool {
        matches!(self.as_boot_memory_type(), BootMemoryType::Available)
    }

    #[inline]
    fn is_countable(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA
            | MemoryType::ACPI_RECLAIM => true,
            _ => false,
        }
    }

    #[inline]
    fn as_boot_memory_type(&self) -> BootMemoryType {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => BootMemoryType::Available,
            MemoryType::LOADER_CODE => BootMemoryType::OsLoaderCode,
            MemoryType::LOADER_DATA => BootMemoryType::OsLoaderData,
            MemoryType::ACPI_RECLAIM => BootMemoryType::AcpiReclaim,
            MemoryType::ACPI_NON_VOLATILE => BootMemoryType::AcpiNonVolatile,
            MemoryType::MMIO => BootMemoryType::Mmio,
            MemoryType::MMIO_PORT_SPACE => BootMemoryType::MmioPortSpace,
            MemoryType::RESERVED => BootMemoryType::Reserved,
            MemoryType::UNUSABLE => BootMemoryType::Unavailable,
            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::PAL_CODE => {
                BootMemoryType::FirmwareCode
            }
            _ => BootMemoryType::FirmwareData,
        }
    }
}

use myelf::SegmentFlags;
pub type MProtect = SegmentFlags;
