//! A reader-writer lock like std::sync::RwLock

use super::*;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

/// A reader-writer lock like std::sync::RwLock
pub struct RwLockNb<T: ?Sized> {
    count: SharedXorMutable,
    data: UnsafeCell<T>,
}

impl<T> RwLockNb<T> {
    #[inline]
    pub const fn new(val: T) -> Self {
        Self {
            count: SharedXorMutable::new(),
            data: UnsafeCell::new(val),
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for RwLockNb<T> {}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLockNb<T> {}

impl<T: ?Sized> RwLockNb<T> {
    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            if self.count.try_read().is_ok() {
                Ok(RwLockReadGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            if self.count.try_write().is_ok() {
                Ok(RwLockWriteGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    #[inline]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        let data = self.data.into_inner();
        // TODO: poison
        Ok(data)
    }

    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        // TODO: poison
        Ok(self.data.get_mut())
    }
}

impl<T: Default> Default for RwLockNb<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for RwLockNb<T> {
    #[inline]
    fn from(val: T) -> Self {
        Self::new(val)
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLockNb<T>,
}

impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

impl<'rwlock, T: ?Sized> RwLockReadGuard<'rwlock, T> {
    unsafe fn new(lock: &'rwlock RwLockNb<T>) -> LockResult<RwLockReadGuard<'rwlock, T>> {
        Ok(Self { lock })
    }
}

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
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#?}", self.deref())
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLockNb<T>,
}

impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

impl<'rwlock, T: ?Sized> RwLockWriteGuard<'rwlock, T> {
    unsafe fn new(lock: &'rwlock RwLockNb<T>) -> LockResult<RwLockWriteGuard<'rwlock, T>> {
        Ok(Self { lock })
    }
}

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
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#?}", self.deref())
    }
}

/// Used inside synchronization mechanisms such as rwlock
#[repr(transparent)]
pub struct SharedXorMutable(AtomicUsize);

impl SharedXorMutable {
    const DEFAULT_VALUE: usize = 0;
    const LOCK_WRITE: usize = 0b0001;
    const LOCK_READ: usize = 0b0010;

    #[inline]
    pub const fn new() -> Self {
        Self(AtomicUsize::new(Self::DEFAULT_VALUE))
    }

    #[inline]
    pub fn is_neutral(&self) -> bool {
        self.0.load(Ordering::Relaxed) == Self::DEFAULT_VALUE
    }

    #[inline]
    pub fn try_write(&self) -> Result<(), WriteError> {
        self.0
            .compare_exchange_weak(
                Self::DEFAULT_VALUE,
                Self::LOCK_WRITE,
                Ordering::SeqCst,
                Ordering::Relaxed,
            )
            .map(|_| ())
            .map_err(|_| WriteError::WouldBlock)
    }

    pub fn try_read(&self) -> Result<(), ReadError> {
        loop {
            let current = self.0.load(Ordering::Relaxed);
            if (current & Self::LOCK_WRITE) != 0 {
                return Err(ReadError::WouldBlock);
            }
            let new_value = current
                .checked_add(Self::LOCK_READ)
                .ok_or(ReadError::TooManyReader)?;
            match self.0.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => {}
            }
        }
    }

    pub unsafe fn unlock_write(&self) {
        self.0.store(Self::DEFAULT_VALUE, Ordering::Release);
    }

    pub unsafe fn unlock_read(&self) {
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |value| {
                value.checked_sub(Self::LOCK_READ)
            });
    }
}

pub enum WriteError {
    WouldBlock,
}

pub enum ReadError {
    WouldBlock,
    TooManyReader,
}
