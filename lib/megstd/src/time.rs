//!

use core::time::Duration;

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
use crate::sys::syscall::*;

//const NSEC_PER_SEC: u64 = 1_000_000_000;

/// 1970-01-01 00:00:00 UTC
pub const UNIX_EPOCH: SystemTime = SystemTime(Duration::ZERO);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemTime(Duration);

impl SystemTime {
    /// 1970-01-01 00:00:00 UTC
    pub const UNIX_EPOCH: SystemTime = UNIX_EPOCH;

    cfg_match! {
        cfg(any(target_arch = "wasm32", target_arch = "wasm64")) => {
            #[inline]
            pub fn now() -> SystemTime {
                os_time_now()
            }
        }
        _ => {
            #[inline]
            pub fn now() -> SystemTime {
                // TODO:
                SystemTime(Duration::ZERO)
            }
        }
    }

    #[inline]
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        let now = SystemTime::now();
        self.0.checked_sub(now.0).ok_or(SystemTimeError(()))
    }

    #[inline]
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        self.0.checked_sub(earlier.0).ok_or(SystemTimeError(()))
    }

    pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_add(duration).map(|v| Self(v))
    }

    pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_sub(duration).map(|v| Self(v))
    }
}

#[derive(Debug)]
pub struct SystemTimeError(());

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Instant(Duration);

impl Instant {
    cfg_match! {
        cfg(any(target_arch = "wasm32", target_arch = "wasm64")) => {
            #[inline]
            pub fn now() -> Instant {
                Instant(os_time_monotonic())
            }
        }
        _ => {
            #[inline]
            pub fn now() -> Instant {
                // TODO:
                Instant(Duration::default())
            }
        }
    }

    /// # Panics
    /// Previous rust versions panicked when earlier was later than self. Currently this method saturates. Future versions may reintroduce the panic in some circumstances.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.0.checked_sub(earlier.0)
    }

    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    pub fn elapsed(&self) -> Duration {
        Self::now().duration_since(*self)
    }

    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_add(duration).map(|v| Self(v))
    }

    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_sub(duration).map(|v| Self(v))
    }
}
