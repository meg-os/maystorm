use super::*;
use crate::drivers::pci::PciBar;
use crate::*;
use core::mem::size_of;
use core::slice;

macro_rules! mmio_reg_declare {
    (
        $(
            $(#[$outer:meta])*
            $vis:vis struct $class:ident: $ty:ty;
        )*
    ) => {
        $(
            $(#[$outer])*
            #[repr(transparent)]
            $vis struct $class(core::cell::UnsafeCell<$ty>);

            impl $class {
                #[allow(dead_code)]
                #[inline]
                $vis fn read_volatile(&self) -> $ty {
                    unsafe {
                        self.0.get().read_volatile()
                    }
                }

                #[allow(dead_code)]
                #[inline]
                $vis fn write_volatile(&self, val: $ty) {
                    unsafe {
                        self.0.get().write_volatile(val);
                    }
                }
            }
        )*
    };
}

mmio_reg_declare! {

    pub struct MmioRegU8: u8;

    pub struct MmioRegU16: u16;

    pub struct MmioRegU32: u32;

    pub struct MmioRegU64: u64;
}

#[derive(Debug, Copy, Clone)]
pub struct MmioSlice {
    base: usize,
    size: usize,
}

impl MmioSlice {
    #[inline]
    pub unsafe fn from_phys(base: PhysicalAddress, size: usize) -> Option<Self> {
        MemoryManager::mmap(MemoryMapRequest::Mmio(base, size)).map(|va| Self {
            base: va.get(),
            size,
        })
    }

    #[inline]
    pub unsafe fn from_bar(bar: &PciBar) -> Option<Self> {
        if bar.is_mmio() {
            Self::from_phys(bar.base(), bar.size())
        } else {
            None
        }
    }

    #[inline]
    #[track_caller]
    fn effective_address<T: Sized>(&self, offset: usize) -> *const T {
        unsafe {
            slice::from_raw_parts(self.base as *const u8, self.size)
                [offset..offset + size_of::<T>()]
                .as_ptr() as *const T
        }
    }

    #[inline]
    #[track_caller]
    fn effective_address_mut<T: Sized>(&self, offset: usize) -> *mut T {
        unsafe {
            slice::from_raw_parts_mut(self.base as *mut u8, self.size)
                [offset..offset + size_of::<T>()]
                .as_mut_ptr() as *mut T
        }
    }

    #[inline]
    #[track_caller]
    pub fn read_u8(&self, offset: usize) -> u8 {
        unsafe { self.effective_address::<u8>(offset).read_volatile() }
    }

    #[inline]
    #[track_caller]
    pub fn write_u8(&self, offset: usize, value: u8) {
        unsafe {
            self.effective_address_mut::<u8>(offset)
                .write_volatile(value);
        }
    }

    #[inline]
    #[track_caller]
    pub fn read_u16(&self, offset: usize) -> u16 {
        unsafe { self.effective_address::<u16>(offset).read_volatile() }
    }

    #[inline]
    #[track_caller]
    pub fn write_u16(&self, offset: usize, value: u16) {
        unsafe {
            self.effective_address_mut::<u16>(offset)
                .write_volatile(value);
        }
    }

    #[inline]
    #[track_caller]
    pub fn read_u32(&self, offset: usize) -> u32 {
        unsafe { self.effective_address::<u32>(offset).read_volatile() }
    }

    #[inline]
    #[track_caller]
    pub fn write_u32(&self, offset: usize, value: u32) {
        unsafe {
            self.effective_address_mut::<u32>(offset)
                .write_volatile(value);
        }
    }

    #[inline]
    #[track_caller]
    pub fn read_u64(&self, offset: usize) -> u64 {
        unsafe { self.effective_address::<u64>(offset).read_volatile() }
    }

    #[inline]
    #[track_caller]
    pub fn write_u64(&self, offset: usize, value: u64) {
        unsafe {
            self.effective_address_mut::<u64>(offset)
                .write_volatile(value);
        }
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute<T: Sized>(&self, offset: usize) -> &'static T {
        unsafe { &*self.effective_address::<T>(offset) }
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute_mut<T: Sized>(&self, offset: usize) -> &'static mut T {
        unsafe { &mut *self.effective_address_mut::<T>(offset) }
    }
}
