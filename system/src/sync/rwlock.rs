//! A reader-writer lock like std::sync::RwLock

use self::rwlock_nb::SharedXorMutable;
use super::signal::SignallingObject;
use super::*;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// A reader-writer lock like std::sync::RwLock
pub struct RwLock<T: ?Sized> {
    count: SharedXorMutable,
    signal: SignallingObject,
    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    #[inline]
    pub const fn new(t: T) -> Self {
        Self {
            count: SharedXorMutable::new(),
            signal: SignallingObject::new(),
            data: UnsafeCell::new(t),
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T: ?Sized> RwLock<T> {
    #[inline]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        self.signal.wait_for(|| self.count.try_read().is_ok());
        Ok(RwLockReadGuard { lock: self })
    }

    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        if self.count.try_read().is_ok() {
            Ok(RwLockReadGuard { lock: self })
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    #[inline]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        self.signal.wait_for(|| self.count.try_write().is_ok());
        Ok(RwLockWriteGuard { lock: self })
    }

    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        if self.count.try_write().is_ok() {
            Ok(RwLockWriteGuard { lock: self })
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    // #[inline]
    // pub fn is_poisoned(&self) -> bool {
    //     // TODO: NOT YET IMPLEMENTED
    //     false
    // }

    // #[inline]
    // pub fn into_inner(self) -> LockResult<T>
    // where
    //     T: Sized,
    // {
    //     todo!()
    // }

    // #[inline]
    // pub fn get_mut(&mut self) -> LockResult<&mut T> {
    //     todo!()
    // }
}

impl<T: Default> Default for RwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> Drop for RwLock<T> {
    fn drop(&mut self) {
        todo!()
    }
}

impl<T> From<T> for RwLock<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.count.unlock_read();
        }
        if self.lock.count.is_neutral() {
            self.lock.signal.signal();
        }
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    // poison: poison::Guard,
}

impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.count.unlock_write();
        }
        if self.lock.count.is_neutral() {
            self.lock.signal.signal();
        }
    }
}
