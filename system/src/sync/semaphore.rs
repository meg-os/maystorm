// Semaphore

use super::fifo::ConcurrentFifo;
use super::signal::SignallingObject;
use crate::*;
use core::marker::PhantomData;
use core::pin::Pin;
use core::sync::atomic::*;
use core::task::{Context, Poll, Waker};
use futures_util::Future;

/// counting semaphore
pub struct Semaphore {
    value: AtomicUsize,
    signal: SignallingObject,
}

impl Semaphore {
    #[inline]
    pub const fn new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            signal: SignallingObject::new(),
        }
    }

    #[inline]
    pub fn estimated_value(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        Hal::sync()
            .fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    #[inline]
    pub fn wait(&self) {
        self.signal.wait_for(|| self.try_lock());
    }

    #[inline]
    pub fn signal(&self) {
        let _ = Hal::sync().fetch_inc(&self.value);
        let _ = self.signal.signal();
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.wait();
        let result = f();
        self.signal();
        result
    }
}

/// binary semaphore
pub struct BinarySemaphore {
    value: AtomicBool,
    signal: SignallingObject,
}

impl BinarySemaphore {
    #[inline]
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(false),
            signal: SignallingObject::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        self.value
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        self.signal.wait_for(|| self.try_lock())
    }

    #[inline]
    pub unsafe fn force_unlock(&self) -> Option<()> {
        self.value
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .map(|_| {
                let _ = self.signal.signal();
            })
            .ok()
    }

    #[inline]
    #[track_caller]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let result = f();
        unsafe {
            self.force_unlock();
        }
        result
    }
}

impl Default for BinarySemaphore {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct AsyncSemaphore {
    value: AtomicUsize,
    fifo: ConcurrentFifo<Waker>,
}

impl AsyncSemaphore {
    #[inline]
    pub fn new(value: usize) -> Pin<Arc<Self>> {
        Arc::pin(Self {
            value: AtomicUsize::new(value),
            fifo: ConcurrentFifo::with_capacity(16),
        })
    }

    #[inline]
    pub fn with_capacity(value: usize, capacity: usize) -> Pin<Arc<Self>> {
        Arc::pin(Self {
            value: AtomicUsize::new(value),
            fifo: ConcurrentFifo::with_capacity(capacity),
        })
    }

    #[inline]
    pub fn estimated_value(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        Hal::sync()
            .fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    #[inline]
    #[must_use]
    pub fn wait(self: Pin<Arc<Self>>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(AsyncSemaphoreObserver { sem: self.clone() })
    }

    #[inline]
    #[must_use]
    pub fn wait_ok<T: 'static>(
        self: &Pin<Arc<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), T>>>> {
        Box::pin(AsyncSemaphoreResultObserver {
            sem: self.clone(),
            _phantom: PhantomData,
        })
    }

    #[must_use]
    pub fn poll(&self, cx: &mut Context<'_>) -> bool {
        let result = self.try_lock();
        if !result {
            self.fifo.enqueue(cx.waker().clone()).unwrap();
        }
        result
    }

    #[inline]
    pub fn signal(&self) {
        let _ = Hal::sync().fetch_inc(&self.value);
        if let Some(waker) = self.fifo.dequeue() {
            waker.wake_by_ref();
        }
    }
}

pub struct AsyncSemaphoreObserver {
    sem: Pin<Arc<AsyncSemaphore>>,
}

impl Future for AsyncSemaphoreObserver {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.sem.poll(cx) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub struct AsyncSemaphoreResultObserver<T> {
    sem: Pin<Arc<AsyncSemaphore>>,
    _phantom: PhantomData<T>,
}

impl<T> Future for AsyncSemaphoreResultObserver<T> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.sem.poll(cx) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
