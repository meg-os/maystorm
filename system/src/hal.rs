//! Hardware Abstraction Layer

use crate::{drivers::pci::PciConfigAddress, system::ProcessorIndex};
use core::{
    fmt,
    iter::Step,
    num::NonZeroU64,
    ops::{Add, BitAnd, BitOr, Mul, Not, Sub},
    sync::atomic::{AtomicUsize, Ordering},
};

pub use crate::arch::hal::{Hal, InterruptGuard, Spinlock};

impl !Send for InterruptGuard {}

impl !Sync for InterruptGuard {}

pub trait HalTrait {
    fn cpu() -> impl HalCpu;

    fn sync() -> impl HalSync;

    fn pci() -> impl HalPci;
}

pub trait HalCpu {
    fn current_processor_index(&self) -> ProcessorIndex;

    fn no_op(&self);

    fn spin_loop_hint(&self);

    unsafe fn wait_for_interrupt(&self);

    unsafe fn enable_interrupt(&self);

    unsafe fn disable_interrupt(&self);

    unsafe fn is_interrupt_enabled(&self) -> bool;

    #[inline]
    unsafe fn is_interrupt_disabled(&self) -> bool {
        !self.is_interrupt_enabled()
    }

    #[inline]
    unsafe fn set_interrupt_enabled(&self, enabled: bool) {
        if enabled {
            self.enable_interrupt();
        } else {
            self.disable_interrupt();
        }
    }

    #[must_use]
    unsafe fn interrupt_guard(&self) -> InterruptGuard;

    fn reset(&self) -> !;

    #[inline]
    fn stop(&self) -> ! {
        loop {
            unsafe {
                self.disable_interrupt();
                self.wait_for_interrupt();
            }
        }
    }

    #[must_use]
    fn spin_wait<'a, 'b>(&'a self) -> impl HalSpinLoopWait + 'b;

    fn broadcast_reschedule(&self);

    fn broadcast_invalidate_tlb(&self) -> Result<(), ()>;

    unsafe fn invoke_user(&self, start: usize, stack_pointer: usize) -> !;

    #[cfg(target_arch = "x86_64")]
    unsafe fn invoke_legacy(&self, ctx: &crate::rt::LegacyAppContext) -> !;
}

pub trait HalSync {
    #[inline]
    fn fetch_inc(&self, ptr: &AtomicUsize) -> usize {
        ptr.fetch_add(1, Ordering::SeqCst)
    }

    #[inline]
    fn compare_and_swap(&self, ptr: &AtomicUsize, current: usize, new: usize) -> (bool, usize) {
        match ptr.compare_exchange(current, new, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(v) => (true, v),
            Err(v) => (false, v),
        }
    }

    #[inline]
    fn fetch_update<F>(&self, ptr: &AtomicUsize, f: F) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>,
    {
        ptr.fetch_update(Ordering::SeqCst, Ordering::Relaxed, f)
    }

    #[inline]
    fn swap(&self, ptr: &AtomicUsize, val: usize) -> usize {
        ptr.swap(val, Ordering::SeqCst)
    }

    #[inline]
    fn fetch_set(&self, ptr: &AtomicUsize, position: usize) -> bool {
        let bit = 1 << position;
        (ptr.fetch_or(bit, Ordering::SeqCst) & bit) != 0
    }

    #[inline]
    fn fetch_reset(&self, ptr: &AtomicUsize, position: usize) -> bool {
        let bit = 1 << position;
        (ptr.fetch_and(!bit, Ordering::SeqCst) & bit) != 0
    }
}

pub trait HalPci {
    unsafe fn read_pci(&self, addr: PciConfigAddress) -> u32;

    unsafe fn write_pci(&self, addr: PciConfigAddress, value: u32);

    unsafe fn register_msi(&self, f: fn(usize) -> (), arg: usize) -> Result<(u64, u16), ()>;
}

pub trait HalSpinlock {
    fn is_locked(&self) -> bool;

    #[inline]
    fn is_unlocked(&self) -> bool {
        !self.is_locked()
    }

    #[must_use]
    fn try_lock(&self) -> bool;

    fn lock(&self);

    unsafe fn force_unlock(&self) -> Option<()>;

    #[inline]
    fn synchronized<F, R>(&self, f: F) -> R
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

pub trait HalSpinLoopWait {
    fn reset(&mut self);

    fn wait(&mut self);
}

#[macro_export]
macro_rules! without_interrupts {
    ( $f:expr ) => {{
        let flags = Hal::cpu().interrupt_guard();
        let result = { $f };
        drop(flags);
        result
    }};
}

#[cfg(target_pointer_width = "32")]
pub type PhysicalAddressRepr = u32;
#[cfg(target_pointer_width = "32")]
pub type NonZeroPhysicalAddressRepr = NonZeroU32;
#[cfg(target_pointer_width = "64")]
pub type PhysicalAddressRepr = u64;
#[cfg(target_pointer_width = "64")]
pub type NonZeroPhysicalAddressRepr = NonZeroU64;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(PhysicalAddressRepr);

impl PhysicalAddress {
    pub const NULL: Self = Self(0);

    #[inline]
    pub const fn new(val: PhysicalAddressRepr) -> Self {
        Self(val as PhysicalAddressRepr)
    }

    #[inline]
    pub const fn from_usize(val: usize) -> Self {
        Self(val as PhysicalAddressRepr)
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    pub const fn from_u32(val: u32) -> Self {
        Self(val as PhysicalAddressRepr)
    }

    #[inline]
    pub const fn from_u64(val: u64) -> Self {
        Self(val as PhysicalAddressRepr)
    }

    #[inline]
    pub const fn as_repr(&self) -> PhysicalAddressRepr {
        self.0 as PhysicalAddressRepr
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0 as u32
    }

    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Gets a pointer identical to the specified physical address.
    ///
    /// # Safety
    ///
    /// Pointers of this form may not map to some memory.
    #[inline]
    pub const unsafe fn identity_map<T>(&self) -> *mut T {
        self.0 as usize as *mut T
    }
}

impl Default for PhysicalAddress {
    #[inline]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Add<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs as PhysicalAddressRepr)
    }
}

impl Add<PhysicalAddressRepr> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: PhysicalAddressRepr) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = usize;

    #[inline]
    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        (self.0 - rhs.0) as usize
    }
}

impl Sub<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs as PhysicalAddressRepr)
    }
}

impl Mul<usize> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self::Output {
        Self(self.0 * rhs as PhysicalAddressRepr)
    }
}

impl Mul<PhysicalAddressRepr> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: PhysicalAddressRepr) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl BitAnd<PhysicalAddressRepr> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: PhysicalAddressRepr) -> Self::Output {
        Self(self.0 & rhs)
    }
}

impl BitAnd<PhysicalAddress> for PhysicalAddressRepr {
    type Output = Self;

    fn bitand(self, rhs: PhysicalAddress) -> Self::Output {
        self & rhs.0
    }
}

impl BitOr<PhysicalAddressRepr> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: PhysicalAddressRepr) -> Self::Output {
        Self(self.0 | rhs)
    }
}

impl Not for PhysicalAddress {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl From<PhysicalAddressRepr> for PhysicalAddress {
    #[inline]
    fn from(val: PhysicalAddressRepr) -> Self {
        Self::new(val)
    }
}

impl From<PhysicalAddress> for PhysicalAddressRepr {
    #[inline]
    fn from(val: PhysicalAddress) -> Self {
        val.as_repr()
    }
}

impl fmt::LowerHex for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl Step for PhysicalAddress {
    #[inline]
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        u64::steps_between(&start.0, &end.0)
    }

    #[inline]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u64::forward_checked(start.0, count).map(|v| PhysicalAddress(v))
    }

    #[inline]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u64::backward_checked(start.0, count).map(|v| PhysicalAddress(v))
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonNullPhysicalAddress(NonZeroPhysicalAddressRepr);

impl NonNullPhysicalAddress {
    #[inline]
    pub const fn get(&self) -> PhysicalAddress {
        PhysicalAddress(self.0.get())
    }

    #[inline]
    pub fn new(val: PhysicalAddress) -> Option<Self> {
        NonZeroPhysicalAddressRepr::new(val.as_repr()).map(Self)
    }

    #[inline]
    pub const unsafe fn new_unchecked(val: PhysicalAddress) -> Self {
        Self(NonZeroPhysicalAddressRepr::new_unchecked(val.as_repr()))
    }
}

impl From<NonNullPhysicalAddress> for PhysicalAddress {
    #[inline]
    fn from(val: NonNullPhysicalAddress) -> Self {
        val.get()
    }
}
