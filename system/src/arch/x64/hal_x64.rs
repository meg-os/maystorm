use crate::arch::apic::Apic;
use crate::arch::cpu::Cpu;
use crate::arch::page::PageManager;
use crate::drivers::pci::PciConfigAddress;
use crate::hal::*;
use crate::system::ProcessorIndex;
use crate::*;
use core::arch::asm;
use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use x86::gpr::Rflags;

#[derive(Clone, Copy)]
pub struct Hal;

impl HalTrait for Hal {
    #[inline]
    fn cpu() -> impl HalCpu {
        CpuImpl
    }

    #[inline]
    fn sync() -> impl HalSync {
        HalSyncImpl
    }

    #[inline]
    fn pci() -> impl HalPci {
        HalPciImpl
    }
}

#[derive(Clone, Copy)]
struct CpuImpl;

impl HalCpu for CpuImpl {
    #[inline]
    fn current_processor_index(&self) -> ProcessorIndex {
        ProcessorIndex(Cpu::rdtscp().1 as usize)
    }

    #[inline]
    fn no_op(&self) {
        unsafe {
            asm!("nop", options(nomem, nostack));
        }
    }

    #[inline]
    fn spin_loop_hint(&self) {
        unsafe {
            asm!("pause", options(nomem, nostack));
        }
    }

    #[inline]
    unsafe fn wait_for_interrupt(&self) {
        asm!("hlt", options(nomem, nostack));
    }

    #[inline]
    unsafe fn enable_interrupt(&self) {
        asm!("sti", options(nomem, nostack));
    }

    #[inline]
    unsafe fn disable_interrupt(&self) {
        asm!("cli", options(nomem, nostack));
    }

    #[inline]
    unsafe fn is_interrupt_enabled(&self) -> bool {
        Rflags::read().contains(Rflags::IF)
    }

    fn reset(&self) -> ! {
        unsafe {
            Cpu::out8(0x0CF9, 0x06);

            asm!("out 0x92, al", in("al") 0x01 as u8, options(nomem, nostack));

            loop {
                asm!("hlt", options(nomem, nostack));
            }
        }
    }

    #[inline]
    unsafe fn interrupt_guard(&self) -> InterruptGuard {
        let mut rax: usize;
        asm!("
                pushfq
                cli
                pop {0}
                ", lateout(reg) rax);
        InterruptGuard(rax)
    }

    #[inline]
    fn spin_wait<'a, 'b>(&'a self) -> impl HalSpinLoopWait + 'b {
        SpinLoopWait::new()
    }

    #[inline]
    fn broadcast_reschedule(&self) {
        Apic::broadcast_reschedule();
    }

    #[inline]
    fn broadcast_invalidate_tlb(&self) -> Result<(), ()> {
        Apic::broadcast_invalidate_tlb()
    }

    #[inline]
    unsafe fn invoke_user(&self, start: usize, stack_pointer: usize) -> ! {
        Cpu::invoke_user(start, stack_pointer);
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    unsafe fn invoke_legacy(&self, ctx: &crate::rt::LegacyAppContext) -> ! {
        Cpu::invoke_legacy(ctx);
    }
}

#[must_use]
pub struct InterruptGuard(usize);

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if Rflags::from_bits_retain(self.0).contains(Rflags::IF) {
            unsafe {
                Hal::cpu().enable_interrupt();
            }
        }
    }
}

#[derive(Clone, Copy)]
struct HalSyncImpl;

impl HalSync for HalSyncImpl {
    #[inline]
    fn fetch_set(&self, ptr: &AtomicUsize, position: usize) -> bool {
        unsafe {
            let ptr = ptr as *const _ as *mut usize;
            let result: u8;
            asm!("
                    lock bts [{0}], {1}
                    setc {2}
                    ", in(reg) ptr, in(reg) position, out(reg_byte) result);
            result != 0
        }
    }

    #[inline]
    fn fetch_reset(&self, ptr: &AtomicUsize, position: usize) -> bool {
        unsafe {
            let ptr = ptr as *const _ as *mut usize;
            let result: u8;
            asm!("
                    lock btr [{0}], {1}
                    setc {2}
                    ", in(reg) ptr, in(reg) position, out(reg_byte) result);
            result != 0
        }
    }
}

#[derive(Clone, Copy)]
struct HalPciImpl;

impl HalPci for HalPciImpl {
    #[inline]
    unsafe fn read_pci(&self, addr: crate::drivers::pci::PciConfigAddress) -> u32 {
        without_interrupts!({
            Cpu::out32(0xCF8, addr.into());
            Cpu::in32(0xCFC)
        })
    }

    #[inline]
    unsafe fn write_pci(&self, addr: crate::drivers::pci::PciConfigAddress, value: u32) {
        without_interrupts!({
            Cpu::out32(0xCF8, addr.into());
            Cpu::out32(0xCFC, value);
        })
    }

    #[inline]
    unsafe fn register_msi(&self, f: fn(usize) -> (), arg: usize) -> Result<(u64, u16), ()> {
        Apic::register_msi(f, arg)
    }
}

impl Into<u32> for PciConfigAddress {
    #[inline]
    fn into(self) -> u32 {
        0x8000_0000
            | ((self.get_bus() as u32) << 16)
            | ((self.get_dev() as u32) << 11)
            | ((self.get_fun() as u32) << 8)
            | ((self.get_register() as u32) << 2)
    }
}

pub struct Spinlock {
    value: AtomicBool,
}

impl Spinlock {
    const LOCKED_VALUE: bool = true;
    const UNLOCKED_VALUE: bool = false;

    #[inline]
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(Self::UNLOCKED_VALUE),
        }
    }
}

impl HalSpinlock for Spinlock {
    #[inline]
    fn is_locked(&self) -> bool {
        self.value.load(Ordering::Relaxed) == Self::LOCKED_VALUE
    }

    #[inline]
    #[must_use]
    fn try_lock(&self) -> bool {
        self.value
            .compare_exchange(
                Self::UNLOCKED_VALUE,
                Self::LOCKED_VALUE,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    fn lock(&self) {
        while self
            .value
            .compare_exchange(
                Self::UNLOCKED_VALUE,
                Self::LOCKED_VALUE,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_err()
        {
            let mut spin_loop = SpinLoopWait::new();
            while self.value.load(Ordering::Acquire) {
                spin_loop.wait();
            }
        }
    }

    #[inline]
    unsafe fn force_unlock(&self) -> Option<()> {
        self.value
            .compare_exchange(
                Self::LOCKED_VALUE,
                Self::UNLOCKED_VALUE,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .map(|_| ())
            .ok()
    }
}

#[derive(Clone, Copy)]
struct SpinLoopWait(usize);

impl SpinLoopWait {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }
}

impl HalSpinLoopWait for SpinLoopWait {
    #[inline]
    fn reset(&mut self) {
        self.0 = 0;
    }

    fn wait(&mut self) {
        let count = self.0;
        for _ in 0..(1 << count) {
            Hal::cpu().spin_loop_hint();
        }
        if count < 6 {
            self.0 += 1;
        }
    }
}

impl PhysicalAddress {
    /// Gets the pointer corresponding to the specified physical address.
    #[inline]
    pub const fn direct_map<T>(&self) -> *mut T {
        PageManager::direct_map(*self) as *mut T
    }

    #[inline]
    pub fn direct_unmap<T>(ptr: *const T) -> Option<PhysicalAddress> {
        PageManager::direct_unmap(ptr as usize)
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:012x}", self.as_u64())
    }
}
