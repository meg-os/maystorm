//! Kernel Invocation for 32-bit UEFI running on x86-64 processors

use super::*;
use bootprot::PlatformType;
use core::arch::asm;
use core::ptr::addr_of_mut;
use x86::cpuid::*;
use x86::cr::*;
use x86::efer::EFER;
use x86::msr::MSR;

pub struct Invocation;

impl Invocation {
    #[inline]
    pub const fn new() -> Self {
        Self
    }
}

impl Invoke for Invocation {
    const INCOMPATIBILITY_MESSAGE: &str = "The Operating System you are attempting to launch requires an IvyBridge or later processor.";

    #[inline]
    fn is_compatible(&self) -> bool {
        let cpuid_8_0 = unsafe { cpuid(0x8000_0000) };
        (cpuid_8_0.eax >= 0x8000_0001) && Feature::LM.exists() && Feature::RDTSCP.exists()
    }

    unsafe fn invoke_kernel(
        &self,
        mut info: BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
        info.platform = PlatformType::UefiBridged;

        unsafe {
            // Disable paging before entering Long Mode.
            CR0::PG.disable();

            // For Intel processors, unlock NXE disable. (ex: Surface 3)
            if is_intel_processor() {
                MSR::IA32_MISC_ENABLE.bit_clear(2);
            }

            // Set up a GDT for Long Mode
            let gdt = &mut *addr_of_mut!(GDT);
            gdt.fix_up();
            gdt.load();

            // Set up a CR3 for Long Mode
            CR3::write(info.master_page_table as usize);

            // Enable NXE & LME
            EFER::NXE.enable();
            EFER::LME.enable();

            // Enable PAE
            CR4::PAE.enable();

            // Enter to Long Mode
            CR0::PG.enable();

            let params = [entry.0, new_sp.0 - 0x20];

            // Trampoline code to jump to 64-bit kernel
            asm!("
                                                // [bits 32]
                .byte 0x6a, 0x08                //      push 0x08
                .byte 0xe8, 0x08, 0, 0, 0       //      call _jmpf
                                                // [bits 64]
                .byte 0x48, 0x8b, 0x60, 0x08    //      mov rsp, [rax + 8]
                .byte 0xff, 0x10                //      call [rax + 0]
                .byte 0x0f, 0x0b                //      ud2
                                                // [bits 32]
                                                // _jmpf:
                .byte 0xff, 0x2c, 0x24          //      jmp far [esp]
                ",
                in("eax") &params,
                in("edi") &info,
                options(noreturn),
            );
        }
    }
}

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

#[repr(C, align(16))]
struct GlobalDescriptorTable {
    table: [u16; 12],
}

impl GlobalDescriptorTable {
    #[inline]
    const fn new() -> Self {
        Self {
            table: [
                0xFFFF, 0, 0, 0, // 00 NULL
                0xFFFF, 0x0000, 0x9A00, 0x00AF, // 08 DPL0 CODE64 FLAT
                0xFFFF, 0x0000, 0x9200, 0x00CF, // 10 DPL0 DATA FLAT
            ],
        }
    }

    #[inline]
    fn fix_up(&mut self) {
        let base = self.table.as_ptr() as usize;
        self.table[1] = (base & 0xFFFF) as u16;
        self.table[2] = ((base >> 16) & 0xFFFF) as u16;
    }

    #[inline]
    unsafe fn load(&'static self) {
        unsafe {
            asm!("lgdt [{0}]", in(reg) self);
        }
    }
}
