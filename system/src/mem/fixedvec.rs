use core::ops::{Deref, DerefMut, Range};
use core::ptr;

#[repr(C)]
pub struct FixedVec<T, const N: usize> {
    data: [T; N],
    len: usize,
}

impl<T, const N: usize> FixedVec<T, N> {
    #[inline]
    pub const fn new(empty_value: T) -> Self
    where
        T: Copy,
    {
        Self {
            data: [empty_value; N],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, val: T) -> Result<(), T> {
        if self.len() < self.capacity() {
            unsafe {
                ptr::write((&mut self.data as *mut T).add(self.len()), val);
            }
            self.len += 1;
            Ok(())
        } else {
            Err(val)
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len() > 0 {
            self.len -= 1;
            unsafe { Some(ptr::read((&self.data as *const T).add(self.len()))) }
        } else {
            None
        }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.len = new_len;
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.data.len()
    }

    #[inline]
    #[track_caller]
    pub fn as_slice(&self) -> &[T] {
        let range = Range {
            start: 0,
            end: self.len(),
        };
        unsafe { self.data.get_unchecked(range) }
    }

    #[inline]
    #[track_caller]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let range = Range {
            start: 0,
            end: self.len(),
        };
        unsafe { self.data.get_unchecked_mut(range) }
    }
}

impl<T, const N: usize> Deref for FixedVec<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for FixedVec<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
