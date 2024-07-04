//! Concurrent First In First Out

use crate::sync::semaphore::{AsyncSemaphore, Semaphore};
use crate::*;
use core::mem::{self, MaybeUninit};
use core::pin::Pin;
use core::{cell::UnsafeCell, sync::atomic::*};

pub struct EventQueue<T> {
    fifo: ConcurrentFifo<T>,
    sem: Semaphore,
}

impl<T> EventQueue<T> {
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self {
            fifo: ConcurrentFifo::with_capacity(capacity),
            sem: Semaphore::new(0),
        }
    }

    #[inline]
    pub fn post(&self, event: T) -> Result<(), T> {
        self.fifo.enqueue(event).map(|_| self.sem.signal())
    }

    #[inline]
    pub fn get_event(&self) -> Option<T> {
        self.fifo.dequeue()
    }

    #[inline]
    pub fn wait_event(&self) -> T {
        loop {
            match self.fifo.dequeue() {
                Some(v) => return v,
                None => (),
            }
            self.sem.wait();
        }
    }
}

unsafe impl<T: Send> Send for EventQueue<T> {}

unsafe impl<T: Send + Sync> Sync for EventQueue<T> {}

pub struct AsyncEventQueue<T> {
    fifo: ConcurrentFifo<T>,
    sem: Pin<Arc<AsyncSemaphore>>,
}

impl<T> AsyncEventQueue<T> {
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self {
            fifo: ConcurrentFifo::with_capacity(capacity),
            sem: AsyncSemaphore::new(0),
        }
    }

    #[inline]
    pub fn post(&self, event: T) -> Result<(), T> {
        self.fifo.enqueue(event).map(|_| self.sem.signal())
    }

    #[inline]
    pub fn get_event(&self) -> Option<T> {
        self.fifo.dequeue()
    }

    #[inline]
    pub async fn wait_event(&self) -> Option<T> {
        loop {
            match self.fifo.dequeue() {
                Some(v) => return Some(v),
                None => (),
            }
            self.sem.clone().wait().await;
        }
    }
}

unsafe impl<T: Send> Send for AsyncEventQueue<T> {}

unsafe impl<T: Send + Sync> Sync for AsyncEventQueue<T> {}

/// Concurrent First In First Out
pub struct ConcurrentFifo<T> {
    head: AtomicUsize,
    tail: AtomicUsize,
    mask: usize,
    one_lap: usize,
    data: *mut Slot<T>,
}

unsafe impl<T: Send> Send for ConcurrentFifo<T> {}

unsafe impl<T: Send> Sync for ConcurrentFifo<T> {}

impl<T: Sized> ConcurrentFifo<T> {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = (capacity + 1).next_power_of_two();
        let mask = capacity - 1;

        let data = {
            let mut boxed: Box<[Slot<T>]> = (0..capacity).map(|v| Slot::new(v)).collect();
            let ptr = boxed.as_mut_ptr();
            mem::forget(boxed);
            ptr
        };

        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            mask,
            one_lap: capacity,
            data,
        }
    }

    pub fn enqueue(&self, value: T) -> Result<(), T> {
        unsafe { without_interrupts!(self._enqueue(value)) }
    }

    pub fn dequeue(&self) -> Option<T> {
        unsafe { without_interrupts!(self._dequeue()) }
    }

    #[inline]
    fn _enqueue(&self, data: T) -> Result<(), T> {
        let mut spin = Hal::cpu().spin_wait();
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            if (tail + 1) & self.mask == self.head.load(Ordering::Relaxed) & self.mask {
                return Err(data);
            }
            let index = tail & self.mask;
            let slot = unsafe { &mut *self.data.add(index) };
            if slot.stamp() == tail {
                let new_tail = tail.wrapping_add(1);
                match self.tail.compare_exchange_weak(
                    tail,
                    new_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        slot.write(data, new_tail);
                        return Ok(());
                    }
                    Err(_) => (),
                }
            }
            spin.wait();
        }
    }

    #[inline]
    fn _dequeue(&self) -> Option<T> {
        let mut spin = Hal::cpu().spin_wait();
        loop {
            let head = self.head.load(Ordering::Relaxed);
            let index = head & self.mask;
            if index == self.tail.load(Ordering::Relaxed) & self.mask {
                return None;
            }
            let slot = unsafe { &*self.data.add(index) };
            let new_head = head.wrapping_add(1);
            if slot.stamp() == new_head {
                match self.head.compare_exchange_weak(
                    head,
                    new_head,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let data = slot.read();
                        slot.write_stamp(head + self.one_lap);
                        return Some(data);
                    }
                    Err(_) => (),
                }
            }
            spin.wait();
        }
    }
}

impl<T> Drop for ConcurrentFifo<T> {
    fn drop(&mut self) {
        // TODO:
        while let Some(t) = self._dequeue() {
            drop(t);
        }
        unsafe {
            let ptr = core::slice::from_raw_parts_mut(self.data, self.one_lap) as *mut [Slot<T>];
            drop(Box::from_raw(ptr));
        }
    }
}

struct Slot<T> {
    stamp: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Slot<T> {
    #[inline]
    fn new(stamp: usize) -> Self {
        Self {
            stamp: AtomicUsize::new(stamp),
            value: UnsafeCell::new(MaybeUninit::zeroed()),
        }
    }

    #[inline]
    fn stamp(&self) -> usize {
        self.stamp.load(Ordering::Acquire)
    }

    #[inline]
    fn write_stamp(&self, val: usize) {
        self.stamp.store(val, Ordering::Release);
    }

    #[inline]
    fn write(&mut self, val: T, stamp: usize) {
        self.value.get_mut().write(val);
        self.write_stamp(stamp);
    }

    #[inline]
    fn read(&self) -> T {
        fence(Ordering::SeqCst);
        unsafe { mem::transmute_copy(&*self.value.get()) }
    }
}
