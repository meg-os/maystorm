use super::*;
use core::alloc::{GlobalAlloc, Layout};
use core::cmp;
use core::num::NonZeroUsize;
use core::ptr::{self, null_mut};

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new();

pub struct CustomAlloc;

impl CustomAlloc {
    const fn new() -> Self {
        CustomAlloc {}
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MemoryManager::zalloc(layout)
            .map(|v| v.get() as *mut u8)
            .unwrap_or(null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = MemoryManager::zfree(NonZeroUsize::new(ptr as usize), layout);
    }

    // unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8;

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: the caller must ensure that `new_layout` is greater than zero.
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            // SAFETY: the previously allocated block cannot overlap the newly allocated block.
            // The safety contract for `dealloc` must be upheld by the caller.
            unsafe {
                ptr::copy_nonoverlapping(ptr, new_ptr, cmp::min(layout.size(), new_size));
                self.dealloc(ptr, layout);
            }
        }
        new_ptr
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
