use super::fixedvec::FixedVec;
use super::slab::*;
use crate::arch::page::*;
use crate::sync::{fifo::EventQueue, semaphore::Semaphore, spinlock::SpinMutex};
use crate::system::System;
use crate::task::scheduler::*;
use crate::*;
use bootprot::*;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::mem::{size_of, transmute, MaybeUninit};
use core::num::*;
use core::ptr::{addr_of, addr_of_mut};
use core::slice;
use core::sync::atomic::*;

static mut MM: UnsafeCell<MemoryManager> = UnsafeCell::new(MemoryManager::new());

/// Memory Manager
pub struct MemoryManager {
    reserved_memory_size: usize,
    page_size_min: usize,
    lost_size: AtomicUsize,
    free_pages: AtomicUsize,
    n_fragments: AtomicUsize,
    mem_list: SpinMutex<FixedVec<MemFreePair, { Self::MAX_FREE_PAIRS }>>,
    slab: Option<Box<SlabAllocator>>,

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    real_bitmap: [u32; 8],

    fifo: MaybeUninit<EventQueue<Arc<AsyncMmapRequest>>>,
}

impl MemoryManager {
    const MAX_FREE_PAIRS: usize = 1024;
    pub const PAGE_SIZE_MIN: usize = 0x1000;

    const fn new() -> Self {
        Self {
            reserved_memory_size: 0,
            page_size_min: 0x1000,
            lost_size: AtomicUsize::new(0),
            free_pages: AtomicUsize::new(0),
            n_fragments: AtomicUsize::new(0),
            mem_list: SpinMutex::new(FixedVec::new(MemFreePair::empty())),
            slab: None,
            real_bitmap: [0; 8],
            fifo: MaybeUninit::uninit(),
        }
    }

    pub unsafe fn init_first(info: &BootInfo) {
        assert_call_once!();

        let shared = (&mut *addr_of_mut!(MM)).get_mut();

        let mm: &[BootMemoryMapDescriptor] =
            slice::from_raw_parts(info.mmap_base as usize as *const _, info.mmap_len as usize);
        let mut free_count = 0;

        let mut list = shared.mem_list.lock();
        for mem_desc in mm {
            if mem_desc.mem_type == BootMemoryType::Available {
                let size = mem_desc.page_count as usize * Self::PAGE_SIZE_MIN;
                list.push(MemFreePair::new(mem_desc.base.into(), size))
                    .unwrap();
                free_count += size;
            }
        }
        shared.n_fragments.store(list.len(), Ordering::Release);
        drop(list);

        shared.reserved_memory_size = info.total_memory_size as usize - free_count;
        shared.free_pages.store(free_count, Ordering::SeqCst);

        if cfg!(any(target_arch = "x86_64", target_arch = "x86")) {
            shared.real_bitmap = info.real_bitmap;
        }

        PageManager::init(info);

        shared.slab = Some(Box::new(SlabAllocator::new()));

        shared.fifo.write(EventQueue::new(100));
    }

    pub unsafe fn init_second() {
        PageManager::init_late();
        SpawnOption::with_priority(Priority::Realtime)
            .start(Self::_page_thread, 0, "Page Manager")
            .unwrap();
    }

    fn _page_thread(_args: usize) {
        let shared = Self::shared();
        let fifo = unsafe { shared.fifo.assume_init_ref() };
        loop {
            let event = fifo.wait_event();
            let result = unsafe { PageManager::mmap(event.request) };
            Hal::cpu().broadcast_invalidate_tlb().unwrap();
            event.result.store(result, Ordering::SeqCst);
            event.sem.signal();
        }
    }

    #[inline]
    fn shared() -> &'static Self {
        unsafe { &*(&*addr_of!(MM)).get() }
    }

    #[inline]
    pub unsafe fn mmap(request: MemoryMapRequest) -> Option<NonZeroUsize> {
        if Scheduler::is_enabled() {
            let fifo = &*Self::shared().fifo.as_ptr();
            let event = Arc::new(AsyncMmapRequest {
                request,
                result: AtomicUsize::new(0),
                sem: Semaphore::new(0),
            });
            match fifo.post(event.clone()) {
                Ok(_) => (),
                Err(_) => todo!(),
            }
            event.sem.wait();
            NonZeroUsize::new(event.result.load(Ordering::SeqCst))
        } else {
            NonZeroUsize::new(PageManager::mmap(request))
        }
    }

    #[inline]
    pub fn page_size_min(&self) -> usize {
        self.page_size_min
    }

    #[inline]
    pub fn reserved_memory_size() -> usize {
        let shared = Self::shared();
        shared.reserved_memory_size
    }

    #[inline]
    pub fn free_memory_size() -> usize {
        let shared = Self::shared();
        shared.free_pages.load(Ordering::Relaxed)
    }

    /// Allocate pages
    #[must_use]
    pub unsafe fn pg_alloc(layout: Layout) -> Option<NonNullPhysicalAddress> {
        if layout.align() > Self::PAGE_SIZE_MIN {
            return None;
        }
        let shared = Self::shared();
        let align_m1 = Self::PAGE_SIZE_MIN - 1;
        let size = (layout.size() + align_m1) & !(align_m1);

        let list = shared.mem_list.lock();
        for pair in list.as_slice() {
            match pair.alloc(size) {
                Ok(v) => {
                    shared.free_pages.fetch_sub(size, Ordering::Relaxed);
                    return NonNullPhysicalAddress::new(v);
                }
                Err(_) => (),
            }
        }

        None
    }

    pub unsafe fn pg_dealloc(base: PhysicalAddress, layout: Layout) {
        let shared = Self::shared();
        let align_m1 = Self::PAGE_SIZE_MIN - 1;
        let size = (layout.size() + align_m1) & !(align_m1);
        let new_entry = MemFreePair::new(base, size);
        shared.free_pages.fetch_add(size, Ordering::Relaxed);

        let mut list = shared.mem_list.lock();
        shared.n_fragments.store(list.len(), Ordering::Release);
        let mut needs_append = true;
        for pair in list.as_slice() {
            if pair.try_merge(new_entry).is_ok() {
                needs_append = false;
                break;
            }
        }
        if needs_append {
            match list.push(new_entry) {
                Ok(_) => (),
                Err(_) => {
                    shared.lost_size.fetch_add(size, Ordering::SeqCst);
                }
            }
        }

        list.sort_by(|a, b| {
            if a.size() > 0 && b.size() > 0 {
                a.base().cmp(&b.base())
            } else {
                b.size().cmp(&a.size())
            }
        });

        loop {
            let Some(last) = list.last() else {
                break;
            };
            if last.size() > 0 {
                break;
            }
            list.pop();
        }

        drop(list);
    }

    #[must_use]
    pub unsafe fn alloc_pages(size: usize) -> Option<NonNullPhysicalAddress> {
        let result = Self::pg_alloc(Layout::from_size_align_unchecked(size, Self::PAGE_SIZE_MIN));
        if let Some(p) = result {
            let p = p.get().direct_map::<c_void>();
            p.write_bytes(0, size);
        }
        result
    }

    #[inline]
    #[must_use]
    pub unsafe fn alloc_dma<T>(len: usize) -> Option<(PhysicalAddress, *mut T)> {
        Self::alloc_pages(size_of::<T>() * len).map(|v| {
            let pa = v.get();
            (pa, pa.direct_map())
        })
    }

    /// Allocate kernel memory
    #[must_use]
    pub unsafe fn zalloc(layout: Layout) -> Option<NonZeroUsize> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(layout) {
                Ok(result) => return Some(result),
                Err(AllocationError::Unsupported) => (),
                Err(_err) => return None,
            }
        }
        Self::zalloc2(layout)
    }

    #[must_use]
    pub unsafe fn zalloc2(layout: Layout) -> Option<NonZeroUsize> {
        Self::pg_alloc(layout)
            .and_then(|v| NonZeroUsize::new(v.get().direct_map::<c_void>() as usize))
    }

    /// Deallocate kernel memory
    pub unsafe fn zfree(
        base: Option<NonZeroUsize>,
        layout: Layout,
    ) -> Result<(), DeallocationError> {
        if let Some(base) = base {
            let base_ptr = base.get() as *mut u8;
            base_ptr.write_bytes(0xCC, layout.size());

            let shared = Self::shared();
            if let Some(slab) = &shared.slab {
                if slab.free(base, layout).is_ok() {
                    return Ok(());
                }
            }

            PhysicalAddress::direct_unmap(base_ptr)
                .map(|v| Self::pg_dealloc(v, layout))
                .ok_or(DeallocationError::InvalidArgument)
        } else {
            Ok(())
        }
    }

    /// Allocate a page on real memory
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    pub unsafe fn static_alloc_real() -> Option<NonZeroU8> {
        let max_real = 0xA0;
        let shared = Self::shared();
        for i in 1..max_real {
            let result =
                Hal::sync().fetch_reset(&*(shared.real_bitmap.as_ptr() as *const AtomicUsize), i);
            if result {
                return NonZeroU8::new(i as u8);
            }
        }
        None
    }

    pub fn statistics(sb: &mut String) {
        let shared = Self::shared();
        sb.reserve(4096);

        let lost_size = shared.lost_size.load(Ordering::Relaxed);
        let n_fragments = shared.n_fragments.load(Ordering::Relaxed);
        let total = shared.free_pages.load(Ordering::Relaxed);

        let mut max_free_area = 0;
        let list = shared.mem_list.lock();
        for pair in list.as_slice() {
            max_free_area = max_free_area.max(pair.size());
        }
        drop(list);

        writeln!(
            sb,
            "Total {} MB, Free Pages {}, Fragments {}, Max Free {} MB, Lost {} MB",
            System::current_device().total_memory_size() >> 20,
            total / Self::PAGE_SIZE_MIN,
            n_fragments,
            max_free_area >> 20,
            ((lost_size + 0xFFFFF) >> 20),
        )
        .unwrap();

        for chunk in shared.slab.as_ref().unwrap().statistics().chunks(4) {
            write!(sb, "Slab").unwrap();
            for item in chunk {
                write!(sb, " {:4}: {:4}/{:4}", item.0, item.1, item.2,).unwrap();
            }
            writeln!(sb, "").unwrap();
        }
    }

    pub fn get_memory_map(sb: &mut String) {
        let shared = Self::shared();
        sb.reserve(4096);

        let list = shared.mem_list.lock();
        for (index, pair) in list.as_slice().iter().enumerate() {
            writeln!(
                sb,
                "MEM: {:2} {:08x}-{:08x} ({:08x})",
                index,
                pair.base(),
                pair.base() + pair.size(),
                pair.size(),
            )
            .unwrap();
        }
        drop(list);
    }
}

struct AsyncMmapRequest {
    request: MemoryMapRequest,
    result: AtomicUsize,
    sem: Semaphore,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
struct MemFreePair(u64);

#[allow(dead_code)]
impl MemFreePair {
    const PAGE_SIZE: usize = 0x1000;

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn new(base: PhysicalAddress, size: usize) -> Self {
        let base = base.as_u64() / Self::PAGE_SIZE as u64;
        let size = (size / Self::PAGE_SIZE) as u64;
        Self(base | (size << 32))
    }

    #[inline]
    fn inner(&self) -> &AtomicU64 {
        unsafe { transmute(&self.0) }
    }

    #[inline]
    pub fn raw(&self) -> u64 {
        self.inner().load(Ordering::SeqCst)
    }

    #[inline]
    fn _split(data: u64) -> (PhysicalAddress, usize) {
        (
            PhysicalAddress::new(data & 0xFFFF_FFFF),
            (data >> 32) as usize,
        )
    }

    #[inline]
    pub fn base(&self) -> PhysicalAddress {
        Self::_split(self.raw()).0 * Self::PAGE_SIZE
    }

    #[inline]
    pub fn size(&self) -> usize {
        Self::_split(self.raw()).1 * Self::PAGE_SIZE
    }

    #[inline]
    pub fn try_merge(&self, other: Self) -> Result<(), ()> {
        let p = self.inner();
        p.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |data| {
            let (base0, size0) = Self::_split(data);
            let (base1, size1) = Self::_split(other.raw());
            let end0 = base0 + size0;
            let end1 = base1 + size1;
            if end0 == base1 {
                Some((base0.as_u64()) | (((size0 + size1) as u64) << 32))
            } else if end1 == base0 {
                Some((base1.as_u64()) | (((size0 + size1) as u64) << 32))
            } else {
                None
            }
        })
        .map(|_| ())
        .map_err(|_| ())
    }

    #[inline]
    pub fn alloc(&self, size: usize) -> Result<PhysicalAddress, ()> {
        let size = (size + Self::PAGE_SIZE - 1) / Self::PAGE_SIZE;
        let p = self.inner();
        p.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |data| {
            let (base, limit) = Self::_split(data);
            if limit < size {
                return None;
            }
            let new_size = limit - size;
            let new_data = ((base + size).as_u64()) | ((new_size as u64) << 32);
            Some(new_data)
        })
        .map(|data| Self::_split(data).0 * Self::PAGE_SIZE)
        .map_err(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MProtect {
    None = 0,
    Read,
    ReadWrite,
    ReadExec,
}

impl MProtect {
    #[inline]
    pub const fn can_read(&self) -> bool {
        match self {
            MProtect::None => false,
            _ => true,
        }
    }

    #[inline]
    pub const fn can_write(&self) -> bool {
        match self {
            MProtect::ReadWrite => true,
            _ => true,
        }
    }

    #[inline]
    pub const fn can_executable(&self) -> bool {
        match self {
            MProtect::ReadExec => true,
            _ => true,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AllocationError {
    Unexpected,
    OutOfMemory,
    InvalidArgument,
    Unsupported,
}

#[derive(Debug, Copy, Clone)]
pub enum DeallocationError {
    Unexpected,
    InvalidArgument,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryMapRequest {
    /// for MMIO (physical_address, length)
    Mmio(PhysicalAddress, usize),
    /// for Framebuffer (physical_address, length)
    Framebuffer(PhysicalAddress, usize),
    /// for Kernel Mode Heap (base, length, attr)
    Kernel(usize, usize, MProtect),
    /// To reserve heap for User Mode (base, length, attr)
    User(usize, usize, MProtect),
    /// To change page attributes (base, length, attr)
    MProtect(usize, usize, MProtect),
}
