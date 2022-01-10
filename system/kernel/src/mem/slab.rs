use super::*;
use ::alloc::{boxed::Box, vec::Vec};
use core::{alloc::Layout, intrinsics::transmute, num::*, ptr::NonNull, sync::atomic::*};

type UsizeSmall = u16;

static SLABS: [SlabCache2; 8] = [
    SlabCache2::new(16),
    SlabCache2::new(32),
    SlabCache2::new(64),
    SlabCache2::new(128),
    SlabCache2::new(256),
    SlabCache2::new(512),
    SlabCache2::new(1024),
    SlabCache2::new(2048),
];

pub(super) struct SlabAllocator {
    _phantom: (),
}

impl SlabAllocator {
    pub unsafe fn new() -> Self {
        Self { _phantom: () }
    }

    pub fn alloc(&self, layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        let size = usize::max(layout.size(), layout.align());
        if size > UsizeSmall::MAX as usize {
            return Err(AllocationError::Unsupported);
        }
        let size = size as UsizeSmall;
        for slab in &SLABS {
            if size <= slab.block_size {
                return slab.alloc();
            }
        }
        return Err(AllocationError::Unsupported);
    }

    pub fn free(&self, base: NonZeroUsize, layout: Layout) -> Result<(), DeallocationError> {
        let size = usize::max(layout.size(), layout.align());
        if size > UsizeSmall::MAX as usize {
            return Err(DeallocationError::Unsupported);
        }
        let size = size as UsizeSmall;
        for slab in &SLABS {
            if size <= slab.block_size {
                return slab.free(base);
            }
        }
        Err(DeallocationError::Unsupported)
    }

    #[allow(dead_code)]
    pub(super) fn free_memory_size(&self) -> usize {
        SLABS.iter().fold(0, |v, i| v + i.free_memory_size())
    }

    #[allow(dead_code)]
    pub(super) fn statistics(&self) -> Vec<(usize, usize, usize)> {
        let mut vec = Vec::with_capacity(SLABS.len());
        for slab in &SLABS {
            let count = slab.total_count();
            vec.push((slab.block_size(), count - slab.free_count(), count));
        }
        vec
    }
}

#[repr(C)]
pub struct Node16 {
    element: [u8; 16],
    next: AtomicUsize,
}

impl Node16 {
    #[inline]
    pub const fn new() -> Self {
        Self {
            element: [0; 16],
            next: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn next_raw(&self) -> usize {
        self.next.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn next(&self) -> Option<NonNull<Self>> {
        unsafe { transmute(self.next_raw()) }
    }

    #[inline]
    pub fn element_ptr(&self) -> NonZeroUsize {
        unsafe { NonZeroUsize::new_unchecked(self as *const _ as usize) }
    }
}

pub struct AtomicNode {
    next: AtomicUsize,
    element_ptr: AtomicUsize,
}

impl AtomicNode {
    #[inline]
    pub const fn new(element: NonZeroUsize) -> Self {
        Self {
            next: AtomicUsize::new(0),
            element_ptr: AtomicUsize::new(element.get()),
        }
    }

    #[inline]
    pub fn next_raw(&self) -> usize {
        self.next.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn next(&self) -> Option<NonNull<Self>> {
        unsafe { transmute(self.next_raw()) }
    }

    #[inline]
    pub fn element_ptr(&self) -> NonZeroUsize {
        unsafe { transmute(self.element_ptr.load(Ordering::Relaxed)) }
    }
}

struct SlabCache2 {
    block_size: UsizeSmall,
    total_count: AtomicUsize,
    free_count: AtomicUsize,
    free_ptr: AtomicUsize,
}

impl SlabCache2 {
    #[inline]
    const fn new(block_size: usize) -> Self {
        Self {
            block_size: block_size as UsizeSmall,
            total_count: AtomicUsize::new(0),
            free_count: AtomicUsize::new(0),
            free_ptr: AtomicUsize::new(0),
        }
    }

    #[inline]
    const fn block_size(&self) -> usize {
        self.block_size as usize
    }

    #[inline]
    fn total_count(&self) -> usize {
        self.total_count.load(Ordering::Relaxed)
    }

    #[inline]
    fn free_count(&self) -> usize {
        self.free_count.load(Ordering::Relaxed)
    }

    #[inline]
    fn free_memory_size(&self) -> usize {
        self.free_count() * self.block_size()
    }

    unsafe fn expand(&self) -> Result<(), AllocationError> {
        let block_size = self.block_size();
        let entry_size = if block_size == 16 { 32 } else { block_size };
        let entry_count = MemoryManager::PAGE_SIZE_MIN / entry_size;
        let alloc_size = entry_count * entry_size;

        let blob = MemoryManager::zalloc2(Layout::from_size_align_unchecked(
            alloc_size,
            MemoryManager::PAGE_SIZE_MIN,
        ))
        .ok_or(AllocationError::OutOfMemory)?;

        for i in 0..entry_count {
            self.free(NonZeroUsize::new_unchecked(blob.get() + i * entry_size))
                .map_err(|_| AllocationError::Unexpected)?;
        }
        self.total_count.fetch_add(entry_count, Ordering::AcqRel);

        Ok(())
    }

    fn alloc(&self) -> Result<NonZeroUsize, AllocationError> {
        let block_size = self.block_size();
        match block_size {
            16 => unsafe {
                loop {
                    let current = self.free_ptr.load(Ordering::Relaxed);
                    if let Some(node) = transmute::<usize, Option<NonNull<Node16>>>(current) {
                        let next = node.as_ref().next_raw();
                        match self.free_ptr.compare_exchange_weak(
                            current,
                            next,
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                self.free_count.fetch_sub(1, Ordering::Relaxed);
                                return Ok(node.as_ref().element_ptr());
                            }
                            Err(_) => (),
                        }
                    } else {
                        self.expand()?;
                    }
                }
            },
            _ => unsafe {
                loop {
                    let current = self.free_ptr.load(Ordering::Relaxed);
                    if let Some(mut node) = transmute::<usize, Option<NonNull<AtomicNode>>>(current)
                    {
                        let next = node.as_ref().next_raw();
                        match self.free_ptr.compare_exchange_weak(
                            current,
                            next,
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                self.free_count.fetch_sub(1, Ordering::Relaxed);
                                let node = Box::from_raw(node.as_mut());
                                return Ok(node.element_ptr());
                            }
                            Err(_) => (),
                        }
                    } else {
                        self.expand()?;
                    }
                }
            },
        }
    }

    fn free(&self, ptr: NonZeroUsize) -> Result<(), DeallocationError> {
        let block_size = self.block_size();
        match block_size {
            16 => unsafe {
                let new = transmute::<NonZeroUsize, NonNull<Node16>>(ptr);
                let mut current = self.free_ptr.load(Ordering::Relaxed);
                loop {
                    new.as_ref().next.store(current, Ordering::Release);
                    current = match self.free_ptr.compare_exchange_weak(
                        current,
                        new.as_ptr() as usize,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            self.free_count.fetch_add(1, Ordering::Relaxed);
                            return Ok(());
                        }
                        Err(v) => v,
                    };
                }
            },
            _ => unsafe {
                let new = Box::new(AtomicNode::new(ptr));
                let new_ptr = Box::into_raw(new);
                let mut current = self.free_ptr.load(Ordering::Relaxed);
                loop {
                    (&*new_ptr).next.store(current, Ordering::Release);
                    current = match self.free_ptr.compare_exchange_weak(
                        current,
                        new_ptr as usize,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            self.free_count.fetch_add(1, Ordering::Relaxed);
                            return Ok(());
                        }
                        Err(v) => v,
                    }
                }
            },
        }
    }
}
