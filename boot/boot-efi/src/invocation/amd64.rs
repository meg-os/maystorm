//! Kernel Invocation for x86-64

use super::*;
use core::arch::{asm, global_asm};
use core::ffi::c_void;
use x86::cpuid::*;
use x86::efer::EFER;
use x86::msr::MSR;

pub struct Invocation();

extern "C" {
    fn _invoke_kernel_stub() -> !;
    fn _end_invoke_kernel_stub() -> !;
}

global_asm!(
    "
.global _invoke_kernel_stub
.global _end_invoke_kernel_stub
_invoke_kernel_stub:
    mov cr3, r8
    .byte 0xEB, 0x00
    lea rsp, [r10 - 0x20]
    call r9
    ud2
_end_invoke_kernel_stub:
"
);

impl Invocation {
    #[inline]
    pub const fn new() -> Self {
        Self()
    }
}

impl Invoke for Invocation {
    const INCOMPATIBILITY_MESSAGE: &str = "The Operating System you are attempting to launch requires an IvyBridge or later processor.";

    #[inline]
    fn is_compatible(&self) -> bool {
        Feature::RDTSCP.exists()
    }

    unsafe fn invoke_kernel(
        &self,
        info: BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
        unsafe {
            // For Intel processors, unlock NXE disable. (ex: Surface 3)
            if is_intel_processor() {
                MSR::IA32_MISC_ENABLE.bit_clear(2);
            }

            // Enable NXE
            EFER::NXE.enable();

            // Jump to Trampoline Code to avoid problems over 4GB
            let base = _invoke_kernel_stub as usize;
            let end = _end_invoke_kernel_stub as usize;
            let count = end - base;
            let kernel_stub = (0x0800usize) as *mut c_void;
            kernel_stub.copy_from_nonoverlapping(base as *const _, count);

            asm!(
                "jmp rax",
                in("rax") kernel_stub,
                in("rcx") &info,
                in("rdx") 0,
                in("rsi") 0,
                in("rdi") &info,
                in("r8") info.master_page_table,
                in("r9") entry.0,
                in("r10") new_sp.0,
                options(noreturn)
            );
        }
    }
}
