use crate::task::scheduler::*;
use core::{sync::atomic::*, time::Duration};

/// Signalling Object
#[repr(transparent)]
pub struct SignallingObject {
    data: AtomicUsize,
}

impl SignallingObject {
    #[inline]
    pub const fn new() -> Self {
        Self::_new(None)
    }

    #[inline]
    const fn _new(t: Option<ThreadHandle>) -> Self {
        Self {
            data: AtomicUsize::new(Self::_from_t(t)),
        }
    }

    #[inline]
    const fn _from_t(val: Option<ThreadHandle>) -> usize {
        match val {
            Some(v) => v.as_usize(),
            None => 0,
        }
    }

    #[inline]
    const unsafe fn _into_t(val: usize) -> Option<ThreadHandle> {
        ThreadHandle::new(val)
    }

    #[inline]
    pub fn take(&self) -> Option<ThreadHandle> {
        unsafe { Self::_into_t(self.data.swap(0, Ordering::SeqCst)) }
    }

    #[inline]
    pub fn compare_and_swap(
        &self,
        current: Option<ThreadHandle>,
        new: Option<ThreadHandle>,
    ) -> Result<Option<ThreadHandle>, Option<ThreadHandle>> {
        match self.data.compare_exchange(
            Self::_from_t(current),
            Self::_from_t(new),
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(v) => Ok(unsafe { Self::_into_t(v) }),
            Err(v) => Err(unsafe { Self::_into_t(v) }),
        }
    }

    #[inline]
    pub fn wait_for<F>(&self, mut f: F)
    where
        F: FnMut() -> bool,
    {
        // TODO: wait queue
        const MAX_DELTA: u64 = 7;
        loop {
            if f() {
                return;
            }
            let mut delta = 0;
            loop {
                if self.sleep().is_ok() {
                    if f() {
                        return;
                    }
                    break;
                }
                Timer::sleep(Duration::from_millis(1 << delta));
                if delta < MAX_DELTA {
                    delta += 1;
                }
            }
        }
    }

    #[inline]
    fn sleep(&self) -> Result<(), ()> {
        let current = Scheduler::current_thread();
        self.compare_and_swap(None, current)
            .map(|_| Scheduler::sleep_thread())
            .map_err(|_| ())
    }

    #[inline]
    pub fn signal(&self) -> Option<()> {
        self.take().map(|thread| thread.wake())
    }
}

impl Default for SignallingObject {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
