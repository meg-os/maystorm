//! **IA32_EFER**: Extended Feature Enables Register *(MSR C000_0080)*

use crate::msr::MSR;

/// **IA32_EFER**: Extended Feature Enables Register *(MSR C000_0080)*
#[repr(transparent)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct EFER(u64);

impl EFER {
    /// Enables the `syscall` and `sysret` instructions
    pub const SYSCALL: Self = Self(1 << 0);
    /// Activates long mode
    pub const LME: Self = Self(1 << 8);
    /// Indicates that long mode is active. THIS BIT CANNOT BE CHANGED MANUALLY.
    pub const LMA: Self = Self(1 << 10);
    /// Enables the no-execute page-protection feature
    pub const NXE: Self = Self(1 << 11);

    #[inline]
    pub const fn has(&self, other: Self) -> bool {
        (self.0 & other.0) != self.0
    }

    #[inline]
    pub unsafe fn enable(&self) {
        MSR::IA32_EFER.bit_set(self.0);
    }

    #[inline]
    pub unsafe fn disable(&self) {
        MSR::IA32_EFER.bit_clear(self.0);
    }

    #[inline]
    pub unsafe fn is_enabled(&self) -> bool {
        (MSR::IA32_EFER.read() & self.0) != 0
    }

    #[inline]
    pub unsafe fn is_disabled(&self) -> bool {
        !self.is_enabled()
    }

    #[inline]
    pub unsafe fn set(&self, value: bool) {
        if value {
            self.enable()
        } else {
            self.disable()
        }
    }
}
