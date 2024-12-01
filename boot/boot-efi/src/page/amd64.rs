//! Minimal Page Manager

use super::*;
use crate::*;
use core::mem::size_of;
use core::ptr::{self, addr_of_mut};
use core::slice;
use core::sync::atomic::{AtomicU64, Ordering};
use uefi::boot::{allocate_pages, AllocateType};
use uefi::mem::memory_map::{MemoryMap, MemoryMapOwned};

const N_DIRECT_MAP_GIGA: usize = 4;
const MAX_REAL_MEMORY: u64 = 0x0000A_0000;

impl VirtualAddress {
    pub const fn index_of(&self, level: usize) -> usize {
        (self.0 >> (level * PageTableEntry::SHIFT_PER_LEVEL + PageTableEntry::SHIFT_PTE)) as usize
            & PageTableEntry::INDEX_MASK
    }
}

static mut PM: PageManager = PageManager::new();

pub struct PageManager {
    pub master_page_table: PhysicalAddress,
    pub static_start: AtomicU64,
    pub static_free: AtomicU64,
    pml2k: PageTableEntry,
}

impl PageManager {
    const fn new() -> Self {
        Self {
            master_page_table: 0,
            static_start: AtomicU64::new(0),
            static_free: AtomicU64::new(0),
            pml2k: PageTableEntry::empty(),
        }
    }

    /// First initialize before exit_boot_services
    pub unsafe fn init_first() -> Result<(), Status> {
        let max_address = 0xFFFF_0000;
        let page_size = 0x0020_0000;
        let count = page_size / UEFI_PAGE_SIZE;
        let page_base = match allocate_pages(
            AllocateType::MaxAddress(max_address),
            MemoryType::LOADER_DATA,
            count as usize,
        ) {
            Ok(v) => v,
            Err(err) => return Err(err.status()),
        };
        let shared = Self::shared();
        shared
            .static_start
            .store(page_base.as_ptr() as usize as u64, Ordering::Release);
        shared.static_free.store(page_size, Ordering::Release);
        Ok(())
    }

    /// Second initialize after exit_boot_services
    pub unsafe fn init_late(info: &mut BootInfo, mm: MemoryMapOwned) {
        let shared = Self::shared();
        let mm = mm.entries();

        let mm_len = mm.len();
        let buffer = Self::alloc_pages(
            (size_of::<BootMemoryMapDescriptor>() * mm_len + UEFI_PAGE_SIZE as usize - 1)
                / UEFI_PAGE_SIZE as usize,
        ) as usize as *const BootMemoryMapDescriptor
            as *mut BootMemoryMapDescriptor;
        info.mmap_base = buffer as usize as u32;
        let buffer = unsafe { slice::from_raw_parts_mut(buffer, mm_len) };
        let mut write_cursor = 0;
        let mut read_cursor = 0;

        let mut last_pa_4g = 0;
        let mut total_memory_size: u64 = 0;
        for mem_desc in mm {
            let mut has_to_copy = true;
            let page_base = mem_desc.phys_start;
            let page_size = mem_desc.page_count * UEFI_PAGE_SIZE;
            let last_pa = page_base + page_size;
            if mem_desc.ty.is_countable() {
                total_memory_size += page_size;
                if last_pa < u32::MAX.into() && last_pa > last_pa_4g {
                    last_pa_4g = last_pa;
                }
            }
            if mem_desc.ty.is_available_at_runtime() {
                if last_pa <= MAX_REAL_MEMORY {
                    let base = page_base / 0x1000;
                    let count = page_size / 0x1000;
                    let limit = core::cmp::min(base + count, 256);
                    for i in base..limit {
                        let index = i as usize / 32;
                        let bit = 1 << (i & 31);
                        info.real_bitmap[index] |= bit;
                    }
                    has_to_copy = false;
                }
            }
            let boot_mem_desc = BootMemoryMapDescriptor {
                base: page_base,
                page_count: mem_desc.page_count as u32,
                mem_type: mem_desc.ty.as_boot_memory_type(),
            };

            if has_to_copy {
                if write_cursor == 0 {
                    buffer[write_cursor] = boot_mem_desc;
                    write_cursor += 1;
                } else {
                    let prev_mem_desc = &buffer[read_cursor];
                    let prev_last_pa =
                        prev_mem_desc.base + prev_mem_desc.page_count as u64 * UEFI_PAGE_SIZE;

                    if prev_mem_desc.mem_type == BootMemoryType::Available
                        && boot_mem_desc.mem_type == BootMemoryType::Available
                        && prev_last_pa == boot_mem_desc.base
                    {
                        buffer[read_cursor].page_count += boot_mem_desc.page_count;
                    } else {
                        read_cursor = write_cursor;
                        buffer[write_cursor] = boot_mem_desc;
                        write_cursor += 1;
                    }
                }
            }
        }
        info.total_memory_size = total_memory_size;
        info.mmap_len = write_cursor as u32 + 1;

        // Minimal Paging
        let common_attributes = PageAttributes::from(MProtect::RWX);

        let cr3 = Self::alloc_pages(1);
        shared.master_page_table = cr3;
        info.master_page_table = cr3;
        let pml4 = PageTableEntry::from(cr3).table(1);

        // 0000_0000_0000_0000 - 0000_0000_FFFF_FFFF Identity Mapping (<4G)
        let pml3p = Self::alloc_pages(1);
        let pml3 = PageTableEntry::from(pml3p).table(1);
        pml4[0] = PageTableEntry::new(pml3p, common_attributes);

        let n_pages = N_DIRECT_MAP_GIGA;
        let pml2p = Self::alloc_pages(n_pages);
        let pml2 = PageTableEntry::from(pml2p).table(n_pages);
        for i in 0..n_pages {
            pml3[i] = PageTableEntry::new(
                pml2p + i as PhysicalAddress * PageTableEntry::NATIVE_PAGE_SIZE,
                common_attributes,
            );
        }
        let limit = ((last_pa_4g + PageTableEntry::LARGE_PAGE_SIZE - 1)
            / PageTableEntry::LARGE_PAGE_SIZE) as usize;
        for i in 0..limit {
            pml2[i] = PageTableEntry::new(
                i as PhysicalAddress * PageTableEntry::LARGE_PAGE_SIZE,
                common_attributes | PageAttributes::LARGE,
            );
        }

        // kernel memory
        let kernel_base = VirtualAddress(info.kernel_base);

        let pml3kp = Self::alloc_pages(1);
        let pml3k = PageTableEntry::from(pml3kp).table(1);
        pml4[kernel_base.index_of(4)] = PageTableEntry::new(pml3kp, common_attributes);

        let pml2kp = Self::alloc_pages(1);
        shared.pml2k = PageTableEntry::new(pml2kp, common_attributes);
        pml3k[kernel_base.index_of(3)] = shared.pml2k;
    }

    #[inline]
    fn shared<'a>() -> &'a mut Self {
        unsafe { &mut *addr_of_mut!(PM) }
    }

    fn alloc_pages(pages: usize) -> PhysicalAddress {
        let shared = Self::shared();
        let size = pages as u64 * PageTableEntry::NATIVE_PAGE_SIZE;
        unsafe {
            let result = shared.static_start.fetch_add(size, Ordering::SeqCst);
            shared
                .static_free
                .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |free| {
                    if free > size {
                        Some(free - size)
                    } else {
                        panic!("Out of memory");
                    }
                })
                .unwrap();
            let ptr = result as *const u8 as *mut u8;
            ptr::write_bytes(ptr, 0, size as usize);
            result
        }
    }

    fn va_set_l1<'a>(base: VirtualAddress) -> &'a mut PageTableEntry {
        let shared = Self::shared();
        let common_attributes = PageAttributes::from(MProtect::RWX);

        let page = (base.0 / PageTableEntry::LARGE_PAGE_SIZE) as usize & PageTableEntry::INDEX_MASK;
        let offset =
            (base.0 / PageTableEntry::NATIVE_PAGE_SIZE) as usize & PageTableEntry::INDEX_MASK;
        let pml2k = shared.pml2k.table(1);
        let mut pml1e = pml2k[page];
        if pml1e.is_empty() {
            pml1e = PageTableEntry::new(Self::alloc_pages(1), common_attributes);
            pml2k[page] = pml1e;
        }
        let pml1 = pml1e.table(1);

        &mut pml1[offset]
    }

    pub fn valloc(base: VirtualAddress, size: usize) -> *mut u8 {
        let common_attributes = PageAttributes::from(MProtect::READ | MProtect::WRITE);

        let size = Self::pages(size as u64, PageTableEntry::NATIVE_PAGE_SIZE) as u64;
        let blob = Self::alloc_pages(size as usize);

        for i in 0..size {
            let p = Self::va_set_l1(base + i * PageTableEntry::NATIVE_PAGE_SIZE);
            *p = PageTableEntry::new(
                blob + i * PageTableEntry::NATIVE_PAGE_SIZE,
                common_attributes,
            );
        }

        blob as usize as *mut u8
    }

    pub fn vprotect(base: VirtualAddress, size: usize, prot: MProtect) {
        let attributes = PageAttributes::from(prot);
        let size = Self::pages(size as u64, PageTableEntry::NATIVE_PAGE_SIZE) as u64;

        for i in 0..size {
            let p = Self::va_set_l1(base + i * PageTableEntry::NATIVE_PAGE_SIZE);
            p.set_attributes(attributes);
        }
    }

    #[inline]
    const fn ceil(base: PhysicalAddress, page: PhysicalAddress) -> PhysicalAddress {
        (base + page - 1) & !(page - 1)
    }

    #[inline]
    const fn pages(base: PhysicalAddress, page_size: PhysicalAddress) -> usize {
        (Self::ceil(base, page_size) / page_size) as usize
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageAttributes(u64);

impl PageAttributes {
    pub const PRESENT: Self = Self(0x0000_0000_0000_0001);
    pub const WRITE: Self = Self(0x0000_0000_0000_0002);
    pub const USER: Self = Self(0x0000_0000_0000_0004);
    pub const PWT: Self = Self(0x0000_0000_0000_0008);
    pub const PCD: Self = Self(0x0000_0000_0000_0010);
    pub const ACCESS: Self = Self(0x0000_0000_0000_0020);
    pub const DIRTY: Self = Self(0x0000_0000_0000_0040);
    pub const PAT: Self = Self(0x0000_0000_0000_0080);
    pub const LARGE: Self = Self(0x0000_0000_0000_0080);
    pub const GLOBAL: Self = Self(0x0000_0000_0000_0100);
    // pub const AVL: Self = Self(0x0000_0000_0000_0E00);
    pub const LARGE_PAT: Self = Self(0x0000_0000_0000_1000);
    pub const NO_EXECUTE: Self = Self(0x8000_0000_0000_0000);

    #[inline]
    pub fn from_bits_truncate(value: u64) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn bits(&self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
}

impl BitOr<Self> for PageAttributes {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<Self> for PageAttributes {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<MProtect> for PageAttributes {
    #[inline]
    fn from(prot: MProtect) -> Self {
        let mut value = PageAttributes::empty();
        if prot.contains(MProtect::READ) {
            value |= PageAttributes::PRESENT | PageAttributes::USER;
            if prot.contains(MProtect::WRITE) {
                value |= PageAttributes::WRITE;
            }
            if !prot.contains(MProtect::EXEC) {
                value |= PageAttributes::NO_EXECUTE;
            }
        }
        value
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
struct PageTableEntry {
    repr: u64,
}

#[allow(dead_code)]
impl PageTableEntry {
    const ADDRESS_BIT: u64 = 0x0000_FFFF_FFFF_F000;
    const NATIVE_PAGE_SIZE: u64 = 0x0000_1000;
    const N_NATIVE_PAGE_ENTRIES: usize = 512;
    const LARGE_PAGE_SIZE: u64 = 0x0020_0000;
    const INDEX_MASK: usize = 0x1FF;
    const MAX_PAGE_LEVEL: usize = 4;
    const SHIFT_PER_LEVEL: usize = 9;
    const SHIFT_PTE: usize = 3;

    const fn empty() -> Self {
        Self { repr: 0 }
    }

    fn is_empty(&self) -> bool {
        self.repr == 0
    }

    const fn new(base: PhysicalAddress, attr: PageAttributes) -> Self {
        Self {
            repr: (base & Self::ADDRESS_BIT) | attr.bits(),
        }
    }

    fn contains(&self, flags: PageAttributes) -> bool {
        (self.repr & flags.bits()) == flags.bits()
    }

    fn insert(&mut self, flags: PageAttributes) {
        self.repr |= flags.bits();
    }

    fn remove(&mut self, flags: PageAttributes) {
        self.repr &= !flags.bits();
    }

    fn frame_address(&self) -> PhysicalAddress {
        self.repr & Self::ADDRESS_BIT
    }

    fn attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.repr)
    }

    fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.repr = (pa & Self::ADDRESS_BIT) | (self.repr & !Self::ADDRESS_BIT);
    }

    fn set_attributes(&mut self, flags: PageAttributes) {
        self.repr = (self.repr & Self::ADDRESS_BIT) | (flags.bits() & !Self::ADDRESS_BIT);
    }

    fn table<'a>(&self, pages: usize) -> &'a mut [Self] {
        unsafe {
            slice::from_raw_parts_mut(
                self.frame_address() as usize as *const PageTableEntry as *mut PageTableEntry,
                pages * Self::N_NATIVE_PAGE_ENTRIES,
            )
        }
    }
}

impl From<PhysicalAddress> for PageTableEntry {
    #[inline]
    fn from(value: PhysicalAddress) -> Self {
        Self { repr: value }
    }
}
