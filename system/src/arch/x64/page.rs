//! 4-level paging (48bit)

use crate::{mem::*, *};
use bootprot::BootInfo;
use core::alloc::Layout;
use core::arch::asm;
use core::ffi::c_void;
use core::mem::transmute;
use core::num::NonZeroUsize;
use core::ops::AddAssign;
use core::sync::atomic::*;
use x86::msr::{MSR, PAT};

type PageTableRepr = u64;

/// Page Manager
pub struct PageManager;

impl PageManager {
    const PAGE_SIZE_4K: usize = 0x1000;
    const PAGE_SIZE_2M: usize = 0x200000;
    const PAGE_SIZE_M1: PageTableRepr = 0xFFF;

    const PAGE_USER_MIN: usize = 0x000;
    const PAGE_USER_MAX: usize = 0x0FF;
    const PAGE_DIRECT_MAP: usize = 0x140;
    const PAGE_HEAP_MIN: usize = 0x1FC;
    const PAGE_HEAP_MAX: usize = 0x1FD;
    const PAGE_RECURSIVE: usize = 0x1FE;

    const DIRECT_BASE: usize = PageLevel::MAX.addr(Self::PAGE_DIRECT_MAP);
    const SIZE_DIRECT_MAP: u64 = PageLevel::Level3.size_of_page();

    #[inline]
    pub unsafe fn init(_info: &BootInfo) {
        let base = Self::read_pdbr() & !Self::PAGE_SIZE_M1;
        let p = base as usize as *mut PageTableEntry;

        MSR::set_pat(PageAttribute::PREFERRED_PAT_SETTINGS);

        // FFFF_FF00_0000_0000 - FFFF_FF7F_FFFF_FFFF RECURSIVE PAGE TABLE AREA
        p.add(Self::PAGE_RECURSIVE)
            .write_volatile(PageTableEntry::new(
                PhysicalAddress::new(base),
                PageAttribute::NO_EXECUTE | PageAttribute::WRITE | PageAttribute::PRESENT,
            ));

        // FFFF_????_????_???? (TEMP) DIRECT MAPPING AREA
        {
            let mut pte = p.read_volatile();
            pte.insert(PageAttribute::NO_EXECUTE | PageAttribute::WRITE | PageAttribute::PRESENT);
            p.add(Self::PAGE_DIRECT_MAP).write_volatile(pte);
        }

        Self::invalidate_all_tlb();
    }

    #[inline]
    pub unsafe fn init_late() {
        // let base = (Self::read_pdbr() as usize) & !(Self::PAGE_SIZE_4K - 1);
        // let p = base as *const u64 as *mut PageTableEntry;
        // p.write_volatile(PageTableEntry::null());
        // Self::invalidate_all_tlb();
    }

    #[inline]
    #[track_caller]
    pub unsafe fn mmap(request: MemoryMapRequest) -> usize {
        match request {
            MemoryMapRequest::Mmio(base, len) => {
                let Some(len) = NonZeroUsize::new(len) else {
                    return 0;
                };
                let va = Self::direct_map(base);
                match Self::_map(
                    va,
                    len,
                    PageTableEntry::new(
                        base,
                        PageAttribute::NO_EXECUTE
                            | PageAttribute::PAT_UC
                            | PageAttribute::WRITE
                            | PageAttribute::PRESENT,
                    ),
                ) {
                    Ok(_) => va,
                    Err(_) => 0,
                }
            }
            MemoryMapRequest::Framebuffer(base, len) => {
                let Some(len) = NonZeroUsize::new(len) else {
                    return 0;
                };
                let va = Self::direct_map(base);
                match Self::_map(
                    va,
                    len,
                    PageTableEntry::new(
                        base,
                        PageAttribute::NO_EXECUTE
                            | PageAttribute::LARGE_2M
                            | PageAttribute::PAT_WC
                            | PageAttribute::WRITE
                            | PageAttribute::USER
                            | PageAttribute::PRESENT,
                    ),
                ) {
                    Ok(_) => va,
                    Err(_) => 0,
                }
            }
            MemoryMapRequest::Kernel(va, len, attr) => {
                if PageLevel::MAX.component(va) < Self::PAGE_HEAP_MIN
                    || PageLevel::MAX.component(va) >= Self::PAGE_HEAP_MAX
                    || PageLevel::MAX.component(len) >= (Self::PAGE_HEAP_MAX - Self::PAGE_HEAP_MIN)
                    || PageLevel::MAX.component(va + len) >= Self::PAGE_HEAP_MAX
                {
                    return 0;
                }
                let Some(len) = NonZeroUsize::new(len) else {
                    return 0;
                };
                let Some(pa) = MemoryManager::alloc_pages(len.get()).map(|v| v.get()) else {
                    return 0;
                };

                match Self::_map(va, len, PageTableEntry::new(pa, PageAttribute::from(attr))) {
                    Ok(_) => va,
                    Err(_) => 0,
                }
            }
            MemoryMapRequest::User(va, len, attr) => {
                if PageLevel::MAX.component(va) < Self::PAGE_USER_MIN
                    || PageLevel::MAX.component(va) > Self::PAGE_USER_MAX
                    || PageLevel::MAX.component(len) > (Self::PAGE_USER_MAX - Self::PAGE_USER_MIN)
                    || PageLevel::MAX.component(va + len) > Self::PAGE_USER_MAX
                {
                    return 0;
                }
                let Some(len) = NonZeroUsize::new(len) else {
                    return 0;
                };
                let Some(pa) = MemoryManager::alloc_pages(len.get()).map(|v| v.get()) else {
                    return 0;
                };

                let mut template = PageAttribute::from(attr);
                template.insert(PageAttribute::USER);
                template.set_avl(PageTableAvl::Reserved);
                // template.remove(PageAttributes::PRESENT);

                Self::_map(va, len, PageTableEntry::new(pa, template)).unwrap();
                va
            }
            MemoryMapRequest::MProtect(va, len, attr) => {
                let Some(len) = NonZeroUsize::new(len) else {
                    return 0;
                };

                Self::_mprotect(va, len, attr)
                    .map(|_| va)
                    .unwrap_or_default()
            }
        }
    }

    #[track_caller]
    unsafe fn _map(va: usize, len: NonZeroUsize, template: PageTableEntry) -> Result<(), usize> {
        if template.contains(PageAttribute::LARGE_2M) {
            // 2M Pages
            let page_size = Self::PAGE_SIZE_2M;
            let page_mask = page_size - 1;
            if (va & page_mask) != 0 {
                return Err(va);
            }
            let count = (len.get() + page_mask) / page_size;
            let mut template = template;
            let base_va = va;
            let mut va = va;
            for _ in 0..count {
                let mut parent_template = template;
                parent_template.insert(PageAttribute::PRESENT | PageAttribute::WRITE);
                for level in [PageLevel::Level4, PageLevel::Level3] {
                    Self::_map_table_if_needed(va, level, parent_template);
                }

                let pdte_ptr = PageLevel::Level2.pte_of(va);
                let pdte = pdte_ptr.read_volatile();
                if pdte.contains(PageAttribute::PRESENT) && !pdte.contains(PageAttribute::LARGE_2M)
                {
                    panic!(
                        "INVALID PDT {:016x} {:016x} {:016x} {}",
                        va, pdte.0, base_va, count
                    );
                }
                pdte_ptr.write_volatile(template);

                Self::invalidate_tlb(va);
                va += page_size;
                template += page_size;
            }
        } else {
            // 4K Pages
            let page_size = Self::PAGE_SIZE_4K;
            let page_mask = page_size - 1;
            if (va & page_mask) != 0 {
                return Err(va);
            }
            let count = (len.get() + page_mask) / page_size;
            let mut template = template;
            let base_va = va;
            let mut va = va;
            for _ in 0..count {
                let mut parent_template = template;
                parent_template.insert(PageAttribute::PRESENT | PageAttribute::WRITE);
                for level in [PageLevel::Level4, PageLevel::Level3, PageLevel::Level2] {
                    Self::_map_table_if_needed(va, level, parent_template);
                }

                let pdte = PageLevel::Level2.pte_of(va).read_volatile();
                if pdte.contains(PageAttribute::LARGE_2M) {
                    panic!(
                        "LARGE PDT {:016x} {:016x} {:016x} {}",
                        va, pdte.0, base_va, count
                    );
                }

                let pte_ptr = PageLevel::Level1.pte_of(va);
                pte_ptr.write_volatile(template);

                Self::invalidate_tlb(va);
                va += page_size;
                template += page_size;
            }
        }
        Ok(())
    }

    #[inline]
    unsafe fn _map_table_if_needed(va: usize, level: PageLevel, template: PageTableEntry) {
        let pte = level.pte_of(va);
        if pte.read_volatile().page_exists() {
            (&mut *pte)
                .accept(template.access_rights())
                .map(|_| Self::invalidate_tlb(va));
        } else {
            let pa = MemoryManager::pg_alloc(Layout::from_size_align_unchecked(
                Self::PAGE_SIZE_4K,
                Self::PAGE_SIZE_4K,
            ))
            .unwrap()
            .get() as PhysicalAddress;
            let table = pa.direct_map::<c_void>();
            table.write_bytes(0, Self::PAGE_SIZE_4K);
            pte.write_volatile(PageTableEntry::new(
                pa as PhysicalAddress,
                PageAttribute::PRESENT | template.access_rights(),
            ));
            Self::invalidate_tlb(va);
        }
    }

    unsafe fn _mprotect(va: usize, len: NonZeroUsize, attr: MProtect) -> Result<(), usize> {
        let len = _round_up(len.get(), Self::PAGE_SIZE_4K - 1);

        for va in (va..va + len).step_by(Self::PAGE_SIZE_4K) {
            for level in [PageLevel::Level4, PageLevel::Level3, PageLevel::Level2] {
                let entry = &*level.pte_of(va);
                if !entry.page_exists() {
                    return Err(va);
                }
            }
        }

        let count = len / Self::PAGE_SIZE_4K;
        let mut new_attr = PageAttribute::from(attr);
        new_attr.remove(PageAttribute::LARGE_2M);
        let mut parent_template = new_attr;
        parent_template.insert(PageAttribute::WRITE);

        let mut va = va;
        for _ in 0..count {
            for level in [PageLevel::Level4, PageLevel::Level3, PageLevel::Level2] {
                let entry = &mut *level.pte_of(va);
                entry.accept(parent_template);
                Self::invalidate_tlb(va);
            }

            let pte = &mut *PageLevel::Level1.pte_of(va);
            pte.set_access_rights(new_attr);

            Self::invalidate_tlb(va);
            va += Self::PAGE_SIZE_4K;
        }

        Ok(())
    }

    #[inline]
    pub(super) unsafe fn invalidate_all_tlb() {
        Self::write_pdbr(Self::read_pdbr());
    }

    #[inline]
    unsafe fn invalidate_tlb(p: usize) {
        fence(Ordering::SeqCst);
        asm!("invlpg [{}]", in(reg) p);
    }

    #[inline]
    unsafe fn read_pdbr() -> PageTableRepr {
        let result: PageTableRepr;
        asm!("mov {}, cr3", out(reg) result);
        result
    }

    #[inline]
    unsafe fn write_pdbr(val: PageTableRepr) {
        asm!("mov cr3, {}", in(reg) val);
    }

    #[inline]
    pub(super) const fn direct_map(pa: PhysicalAddress) -> usize {
        Self::DIRECT_BASE + pa.as_usize()
    }

    #[inline]
    pub(super) fn direct_unmap(va: usize) -> Option<PhysicalAddress> {
        (va >= Self::DIRECT_BASE && ((va - Self::DIRECT_BASE) as u64) < Self::SIZE_DIRECT_MAP)
            .then(|| PhysicalAddress::from_usize(va - Self::DIRECT_BASE))
    }
}

#[inline]
fn _round_up(value: usize, mask: usize) -> usize {
    (value + mask) & !mask
}

my_bitflags! {
    pub struct PageAttribute: PageTableRepr {
        const PRESENT       = 0x0000_0000_0000_0001;
        const WRITE         = 0x0000_0000_0000_0002;
        const USER          = 0x0000_0000_0000_0004;
        const PWT           = 0x0000_0000_0000_0008;
        const PCD           = 0x0000_0000_0000_0010;
        const ACCESS        = 0x0000_0000_0000_0020;
        const DIRTY         = 0x0000_0000_0000_0040;
        const PAT           = 0x0000_0000_0000_0080;
        const LARGE_2M      = 0x0000_0000_0000_0080;
        const GLOBAL        = 0x0000_0000_0000_0100;
        const AVL_MASK      = 0x0000_0000_0000_0E00;
        const LARGE_PAT     = 0x0000_0000_0000_1000;
        const NO_EXECUTE    = 0x8000_0000_0000_0000;
    }
}

impl PageAttribute {
    pub const ACCESS_RIGHTS: Self =
        Self::from_bits_retain(Self::WRITE.bits() | Self::USER.bits() | Self::NO_EXECUTE.bits());

    pub const PAT_000: Self = Self::empty();
    pub const PAT_001: Self = Self::PWT;
    pub const PAT_010: Self = Self::PCD;
    pub const PAT_011: Self = Self::from_bits_retain(Self::PAT_010.bits() | Self::PAT_001.bits());

    pub const PAT_WB: Self = Self::PAT_000;
    pub const PAT_WT: Self = Self::PAT_001;
    pub const PAT_WC: Self = Self::PAT_010;
    pub const PAT_UC: Self = Self::PAT_011;

    pub const PREFERRED_PAT_SETTINGS: [PAT; 8] = [
        PAT::WB,
        PAT::WT,
        PAT::WC,
        PAT::UC,
        PAT::WB,
        PAT::WT,
        PAT::WC,
        PAT::UC,
    ];
}

#[repr(u64)]
#[allow(dead_code)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PageTableAvl {
    Free = 0,
    Reserved = 1,
}

#[allow(dead_code)]
impl PageAttribute {
    pub const AVL_SHIFT: usize = 9;

    #[inline]
    const fn avl(self) -> PageTableAvl {
        unsafe { transmute((self.bits() & Self::AVL_MASK.bits()) >> Self::AVL_SHIFT) }
    }

    #[inline]
    fn set_avl(&mut self, avl: PageTableAvl) {
        *self = Self::from_bits_retain(
            (self.bits() & !Self::AVL_MASK.bits()) | ((avl as PageTableRepr) << Self::AVL_SHIFT),
        );
    }
}

impl From<MProtect> for PageAttribute {
    #[inline]
    fn from(value: MProtect) -> Self {
        match value {
            MProtect::None => PageAttribute::empty(),
            MProtect::Read => PageAttribute::PRESENT | PageAttribute::NO_EXECUTE,
            MProtect::ReadWrite => {
                PageAttribute::PRESENT | PageAttribute::WRITE | PageAttribute::NO_EXECUTE
            }
            MProtect::ReadExec => PageAttribute::PRESENT,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub(super) struct PageTableEntry(PageTableRepr);

#[allow(dead_code)]
impl PageTableEntry {
    pub const ADDRESS_BITS: PageTableRepr = 0x0000_FFFF_FFFF_F000;

    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn new(base: PhysicalAddress, attr: PageAttribute) -> Self {
        Self((base.as_u64() & Self::ADDRESS_BITS) | attr.bits())
    }

    #[inline]
    pub const fn repr(&self) -> PageTableRepr {
        self.0
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn page_exists(&self) -> bool {
        self.contains(PageAttribute::PRESENT)
    }

    #[inline]
    pub const fn contains(&self, flags: PageAttribute) -> bool {
        (self.0 & flags.bits()) == flags.bits()
    }

    #[inline]
    pub const fn insert(&mut self, flags: PageAttribute) {
        self.0 |= flags.bits();
    }

    #[inline]
    pub const fn remove(&mut self, flags: PageAttribute) {
        self.0 &= !flags.bits();
    }

    #[inline]
    pub const fn set(&mut self, flags: PageAttribute, value: bool) {
        match value {
            true => self.insert(flags),
            false => self.remove(flags),
        }
    }

    #[inline]
    pub const fn frame_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0 & Self::ADDRESS_BITS)
    }

    #[inline]
    pub const fn access_rights(&self) -> PageAttribute {
        PageAttribute::from_bits_retain(self.0 & PageAttribute::ACCESS_RIGHTS.bits())
    }

    #[inline]
    pub const fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.0 = (pa.as_u64() & Self::ADDRESS_BITS) | (self.0 & !Self::ADDRESS_BITS);
    }

    #[inline]
    pub const fn set_attributes(&mut self, flags: PageAttribute) {
        self.0 = (self.0 & Self::ADDRESS_BITS) | (flags.bits() & !Self::ADDRESS_BITS);
    }

    #[inline]
    pub fn set_access_rights(&mut self, new_attr: PageAttribute) {
        for flag in [
            PageAttribute::PRESENT,
            PageAttribute::WRITE,
            PageAttribute::NO_EXECUTE,
        ] {
            self.set(flag, new_attr.contains(flag));
        }
    }

    #[inline]
    pub fn accept(&mut self, new_attr: PageAttribute) -> Option<()> {
        let mut result = false;
        if self.contains(PageAttribute::NO_EXECUTE) && !new_attr.contains(PageAttribute::NO_EXECUTE)
        {
            self.remove(PageAttribute::NO_EXECUTE);
            result = true;
        }
        if !self.contains(PageAttribute::WRITE) && new_attr.contains(PageAttribute::WRITE) {
            self.insert(PageAttribute::WRITE);
            result = true;
        }
        result.then(|| ())
    }
}

impl AddAssign<usize> for PageTableEntry {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        let pa = self.frame_address() + rhs;
        self.set_frame_address(pa);
    }
}

impl From<PhysicalAddress> for PageTableEntry {
    #[inline]
    fn from(value: PhysicalAddress) -> Self {
        Self(value.as_u64())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PageLevel {
    /// The lowest level of the Page Table.
    Level1 = 1,
    /// The official name is "Page Directory Table"
    Level2,
    /// The official name is "Page Directory Pointer Table"
    Level3,
    /// The top level page table in 4-level paging, officially named "Page Map Level 4 Table".
    Level4,
}

#[allow(dead_code)]
impl PageLevel {
    pub const MIN: Self = Self::Level1;
    pub const MAX: Self = Self::Level4;

    pub const MASK_MAX_VA: usize = 0x0000_FFFF_FFFF_FFFF;
    pub const MASK_PER_LEVEL: usize = 0x1FF;
    pub const BITS_PER_LEVEL: usize = 9;
    pub const FIRST_LEVEL_BITS: usize = 12;

    pub const RECURSIVE_LV1: usize = Self::Level4.addr(PageManager::PAGE_RECURSIVE);
    pub const RECURSIVE_LV2: usize =
        Self::RECURSIVE_LV1 + Self::Level3.addr(PageManager::PAGE_RECURSIVE);
    pub const RECURSIVE_LV3: usize =
        Self::RECURSIVE_LV2 + Self::Level2.addr(PageManager::PAGE_RECURSIVE);
    pub const RECURSIVE_LV4: usize =
        Self::RECURSIVE_LV3 + Self::Level1.addr(PageManager::PAGE_RECURSIVE);

    #[inline]
    pub const fn shift(&self) -> usize {
        Self::FIRST_LEVEL_BITS
            + Self::BITS_PER_LEVEL
                * match *self {
                    Self::Level1 => 0,
                    Self::Level2 => 1,
                    Self::Level3 => 2,
                    Self::Level4 => 3,
                }
    }

    #[inline]
    pub const fn size_of_page(&self) -> u64 {
        (1u64 << self.shift()).wrapping_sub(1)
    }

    /// Returns the component of the current level specified by linear address.
    #[inline]
    pub const fn component(&self, va: usize) -> usize {
        (va >> self.shift()) & Self::MASK_PER_LEVEL
    }

    #[inline]
    pub const fn addr(&self, component: usize) -> usize {
        ((component & Self::MASK_PER_LEVEL) << self.shift())
            + match *self {
                PageLevel::Level4 => {
                    if component >= 0x100 {
                        0xFFFF_0000_0000_0000
                    } else {
                        0
                    }
                }
                _ => 0,
            }
    }

    /// Returns the PageTableEntry corresponding to the current level of the specified linear address.
    #[inline]
    pub const unsafe fn pte_of(&self, va: usize) -> *mut PageTableEntry {
        let base = va & Self::MASK_MAX_VA;
        let pte = match *self {
            Self::Level1 => Self::RECURSIVE_LV1 + ((base >> self.shift()) << 3),
            Self::Level2 => Self::RECURSIVE_LV2 + ((base >> self.shift()) << 3),
            Self::Level3 => Self::RECURSIVE_LV3 + ((base >> self.shift()) << 3),
            Self::Level4 => Self::RECURSIVE_LV4 + ((base >> self.shift()) << 3),
        };
        pte as *mut PageTableEntry
    }
}

my_bitflags! {
    pub struct PageErrorCode: u16 {
        /// When set, the page fault was caused by a page-protection violation.
        /// When not set, it was caused by a non-present page.
        const PRESENT           = 0b0000_0000_0000_0001;
        /// When set, the page fault was caused by a write access.
        /// When not set, it was caused by a read access.
        const WRITE             = 0b0000_0000_0000_0010;
        /// When set, the page fault was caused while CPL = 3.
        /// This does not necessarily mean that the page fault was a privilege violation.
        const USER              = 0b0000_0000_0000_0100;
        /// When set, one or more page directory entries contain reserved bits which are set to 1.
        /// This only applies when the PSE or PAE flags in CR4 are set to 1.
        const RESERVED_BITS     = 0b0000_0000_0000_1000;
        /// When set, the page fault was caused by an instruction fetch.
        /// This only applies when the No-Execute bit is supported and enabled.
        const FETCH             = 0b0000_0000_0001_0000;
        /// When set, the page fault was caused by a protection-key violation.
        /// The PKRU register (for user-mode accesses) or PKRS MSR (for supervisor-mode accesses) specifies the protection key rights.
        const PROTECTION_KEY    = 0b0000_0000_0010_0000;
        /// When set, the page fault was caused by a shadow stack access.
        const SHADOW_STACK      = 0b0000_0000_0100_0000;
        /// When set, the fault was due to an SGX violation.
        /// The fault is unrelated to ordinary paging.
        const SGX               = 0b1000_0000_0000_0000;
    }
}

impl PageErrorCode {
    #[inline]
    pub fn is_page_present(&self) -> bool {
        self.contains(Self::PRESENT)
    }

    #[inline]
    pub fn could_not_read(&self) -> bool {
        !self.could_not_write() && !self.could_not_execute()
    }

    #[inline]
    pub fn could_not_write(&self) -> bool {
        self.contains(Self::WRITE)
    }

    #[inline]
    pub fn could_not_execute(&self) -> bool {
        self.contains(Self::FETCH)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageErrorKind {
    NotPresent,
    CannotExecute,
    CannotWrite,
    CannotRead,
}

impl PageErrorKind {
    pub fn from_err(code: PageErrorCode) -> Self {
        if code.is_page_present() {
            if code.could_not_execute() {
                Self::CannotExecute
            } else if code.could_not_write() {
                Self::CannotWrite
            } else {
                Self::CannotRead
            }
        } else {
            Self::NotPresent
        }
    }
}
