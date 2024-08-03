use super::apic::*;
use crate::rt::{LegacyAppContext, RuntimeEnvironment};
use crate::system::{ProcessorCoreType, System};
use crate::task::scheduler::Scheduler;
use crate::*;
use bootprot::BootInfo;
use core::arch::asm;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::mem::size_of;
use core::sync::atomic::*;
use paste::paste;
use x86::cpuid::{cpuid, cpuid_count, Feature, NativeModelCoreType};
use x86::gpr::Rflags;
use x86::prot::*;

static mut SHARED_CPU: UnsafeCell<SharedCpu> = UnsafeCell::new(SharedCpu::new());

pub const KERNEL_CSEL: Selector = Selector::new(1, RPL0);
pub const KERNEL_DSEL: Selector = Selector::new(2, RPL0);
pub const LEGACY_CSEL: Selector = Selector::new(3, RPL3);
pub const LEGACY_DSEL: Selector = Selector::new(4, RPL3);
pub const USER_CSEL: Selector = Selector::new(5, RPL3);
pub const USER_DSEL: Selector = Selector::new(6, RPL3);
pub const SYSTEM_TSS: Selector = Selector::new(8, RPL0);

pub struct Cpu {
    apic_id: ApicId,
    core_type: ProcessorCoreType,

    tsc_base: AtomicU64,

    #[allow(dead_code)]
    gdt: Box<GlobalDescriptorTable>,
}

#[allow(dead_code)]
struct SharedCpu {
    max_cpuid_level_0: u32,
    max_cpuid_level_8: u32,
    smt_topology: u32,
    has_smt: AtomicBool,
    is_hybrid: AtomicBool,
    max_physical_address_bits: usize,
    max_virtual_address_bits: usize,
    vram_base: PhysicalAddress,
    vram_size_min: usize,
}

impl SharedCpu {
    const fn new() -> Self {
        Self {
            max_cpuid_level_0: 0,
            max_cpuid_level_8: 0,
            smt_topology: 0,
            has_smt: AtomicBool::new(false),
            is_hybrid: AtomicBool::new(false),
            max_physical_address_bits: 36,
            max_virtual_address_bits: 48,
            vram_base: PhysicalAddress::new(0),
            vram_size_min: 0,
        }
    }
}

impl Cpu {
    pub unsafe fn init(info: &BootInfo) {
        assert_call_once!();

        let shared = SHARED_CPU.get_mut();
        shared.vram_base = PhysicalAddress::new(info.vram_base);
        shared.vram_size_min =
            4 * (info.vram_stride as usize * info.screen_height as usize - 1).next_power_of_two();

        InterruptDescriptorTable::init();

        shared.max_cpuid_level_0 = cpuid(0).eax;
        shared.max_cpuid_level_8 = cpuid(0x8000_0000).eax;

        if shared.max_cpuid_level_0 >= 0x0B {
            if Feature::HYBRID.exists() {
                shared.is_hybrid.store(true, Ordering::SeqCst);
            }
            if shared.max_cpuid_level_0 >= 0x1F {
                let cpuid1f = cpuid(0x1F);
                if (cpuid1f.ecx & 0xFF00) == 0x0100 {
                    shared.smt_topology = (1 << (cpuid1f.eax & 0x1F)) - 1;
                }
            } else {
                let cpuid0b = cpuid(0x0B);
                if (cpuid0b.ecx & 0xFF00) == 0x0100 {
                    shared.smt_topology = (1 << (cpuid0b.eax & 0x1F)) - 1;
                }
            }
        }

        if shared.max_cpuid_level_8 >= 0x8000_0008 {
            let cpuid88 = cpuid(0x8000_0008);
            shared.max_physical_address_bits = (cpuid88.eax & 0xFF) as usize;
            shared.max_virtual_address_bits = ((cpuid88.eax >> 8) & 0xFF) as usize;
        }

        let apic_id = System::acpi()
            .unwrap()
            .local_apics()
            .next()
            .map(|v| v.apic_id())
            .unwrap_or(0);
        System::activate_cpu(Cpu::new(apic_id.into()));
    }

    pub(super) unsafe fn new(apic_id: ApicId) -> Box<Self> {
        let gdt = GlobalDescriptorTable::new();
        InterruptDescriptorTable::load();

        // let shared = &*SHARED_CPU.get();

        let is_normal = if (apic_id.as_u32() & Self::shared().smt_topology) == 0 {
            true
        } else {
            Self::shared().has_smt.store(true, Ordering::SeqCst);
            false
        };
        let is_efficient = matches!(
            Cpu::native_model_core_type().unwrap_or(NativeModelCoreType::Performance),
            NativeModelCoreType::Efficient
        );
        let core_type = ProcessorCoreType::new(is_normal, is_efficient);

        // let mtrr_items = Mtrr::items().filter(|v| v.is_enabled).collect::<Vec<_>>();
        // let mut mtrr_new = Vec::new();
        // let mtrr_remain = Mtrr::count() - mtrr_items.len();
        // if mtrr_items
        //     .iter()
        //     .find(|v| v.matches(shared.vram_base) && v.mem_type == Mtrr::WC)
        //     .is_none()
        // {
        //     // Setting MTRR of VRAM to Write Combining improves drawing performance on some models. (expr)
        //     if mtrr_remain > 0
        //         && mtrr_items
        //             .iter()
        //             .find(|v| v.matches(shared.vram_base))
        //             .is_none()
        //     {
        //         // simply add
        //         mtrr_new.extend_from_slice(mtrr_items.as_slice());
        //         mtrr_new.push(MtrrItem {
        //             base: shared.vram_base,
        //             mask: !(shared.vram_size_min as u64 - 1),
        //             mem_type: Mtrr::WC,
        //             is_enabled: true,
        //         });
        //     } else if mtrr_remain > 0
        //         && shared.vram_base == PhysicalAddress::new(0xC000_0000)
        //         && mtrr_items
        //             .iter()
        //             .find(|v| {
        //                 v.base == shared.vram_base
        //                     && v.matches(PhysicalAddress::new(0xFFFF_FFFF))
        //                     && v.mem_type == Mtrr::UC
        //             })
        //             .is_some()
        //     {
        //         // Some Intel machines have the range C000_0000 to FFFF_FFFF set to UC
        //         mtrr_new = mtrr_items
        //             .into_iter()
        //             .filter(|v| !v.matches(shared.vram_base))
        //             .collect();
        //         mtrr_new.push(MtrrItem {
        //             base: shared.vram_base,
        //             mask: !0x1FFF_FFFF,
        //             mem_type: Mtrr::WC,
        //             is_enabled: true,
        //         });
        //         mtrr_new.push(MtrrItem {
        //             base: PhysicalAddress::new(0xE000_0000),
        //             mask: !0x1FFF_FFFF,
        //             mem_type: Mtrr::UC,
        //             is_enabled: true,
        //         });
        //     } else {
        //         // Unknown, giving up
        //     }
        //     if mtrr_new.len() > 0 {
        //         Mtrr::set_items(&mtrr_new);
        //     }
        // }

        Box::new(Cpu {
            apic_id,
            core_type,
            gdt,
            tsc_base: AtomicU64::new(0),
        })
    }

    #[inline]
    pub(super) fn set_tsc_base(&self, value: u64) {
        self.tsc_base.store(value, Ordering::Release);
    }

    #[inline]
    fn shared<'a>() -> &'a SharedCpu {
        unsafe { &*SHARED_CPU.get() }
    }

    #[inline]
    pub fn is_hybrid() -> bool {
        let shared = Self::shared();
        shared.is_hybrid.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn native_model_core_type() -> Option<NativeModelCoreType> {
        if Self::is_hybrid() {
            let cpuid_1a = unsafe { cpuid_count(0x1A, 0) };
            NativeModelCoreType::from_u8((cpuid_1a.eax >> 24) as u8)
        } else {
            None
        }
    }

    #[inline]
    pub fn physical_address_mask() -> u64 {
        let shared = Self::shared();
        (1 << shared.max_physical_address_bits) - 1
    }

    #[inline]
    pub fn virtual_address_mask() -> usize {
        let shared = Self::shared();
        (1 << shared.max_virtual_address_bits) - 1
    }

    #[inline]
    pub(super) const fn apic_id(&self) -> ApicId {
        self.apic_id
    }

    #[inline]
    pub const fn physical_id(&self) -> usize {
        self.apic_id().as_u32() as usize
    }

    #[inline]
    pub const fn processor_type(&self) -> ProcessorCoreType {
        self.core_type
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn out8(port: u16, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value);
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn in8(port: u16) -> u8 {
        let mut result: u8;
        asm!("in al, dx", in("dx") port, lateout("al") result);
        result
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn out16(port: u16, value: u16) {
        asm!("out dx, ax", in("dx") port, in("ax") value);
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn in16(port: u16) -> u16 {
        let mut result: u16;
        asm!("in ax, dx", in("dx") port, lateout("ax") result);
        result
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn out32(port: u16, value: u32) {
        asm!("out dx, eax", in("dx") port, in("eax") value);
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) unsafe fn in32(port: u16) -> u32 {
        let mut result: u32;
        asm!("in eax, dx", in("dx") port, lateout("eax") result);
        result
    }

    #[inline]
    pub(super) fn rdtsc() -> u64 {
        let eax: u32;
        let edx: u32;
        unsafe {
            asm!("rdtsc",
                lateout("edx") edx,
                lateout("eax") eax,
                options(nomem, nostack)
            );
        }
        eax as u64 + edx as u64 * 0x10000_0000
    }

    #[inline]
    pub(super) fn rdtscp() -> (u64, u32) {
        let eax: u32;
        let edx: u32;
        let ecx: u32;
        unsafe {
            asm!("rdtscp",
                lateout("eax") eax,
                lateout("ecx") ecx,
                lateout("edx") edx,
                options(nomem, nostack),
            );
        }
        (eax as u64 + edx as u64 * 0x10000_0000, ecx)
    }

    /// Launch the user mode application.
    pub(super) unsafe fn invoke_user(start: usize, stack_pointer: usize) -> ! {
        Hal::cpu().disable_interrupt();

        let gdt = GlobalDescriptorTable::current();

        let rsp: u64;
        asm!("mov {0}, rsp", out(reg) rsp);
        gdt.tss.stack_pointer[0] = rsp;

        // The initial value of the Rflags register is interrupt allowed.
        let rflags = Rflags::IF;

        // Reproduce the stack at the time of pseudo interrupt, return with an IRETQ, and transition to user mode.
        asm!("
            mov ds, {new_ss:e}
            mov es, {new_ss:e}
            mov fs, {new_ss:e}
            mov gs, {new_ss:e}
            push {new_ss}
            push {new_sp}
            push {new_fl}
            push {new_cs}
            push {new_ip}
            xor eax, eax
            xor ebx, ebx
            xor ecx, ecx
            xor edx, edx
            xor ebp, ebp
            xor esi, esi
            xor edi, edi
            xor r8, r8
            xor r9, r9
            xor r10, r10
            xor r11, r11
            xor r12, r12
            xor r13, r13
            xor r14, r14
            xor r15, r15
            iretq
            ",
            new_ss = in (reg) USER_DSEL.as_usize(),
            new_cs = in (reg) USER_CSEL.as_usize(),
            new_sp = in (reg) stack_pointer as usize,
            new_ip = in (reg) start as usize,
            new_fl = in (reg) rflags.bits(),
            options(noreturn));
    }

    /// Launch the 32-bit legacy mode application.
    pub(super) unsafe fn invoke_legacy(ctx: &LegacyAppContext) -> ! {
        Hal::cpu().disable_interrupt();

        // Prepare GDT for 32-bit user mode.
        let gdt = GlobalDescriptorTable::current();
        gdt.set_item(
            LEGACY_CSEL,
            DescriptorEntry::code_segment(
                Linear32(ctx.base_of_code),
                Limit32(ctx.size_of_code - 1),
                DPL3,
                USE32,
            ),
        )
        .unwrap();
        gdt.set_item(
            LEGACY_DSEL,
            DescriptorEntry::data_segment(
                Linear32(ctx.base_of_data),
                Limit32(ctx.size_of_data - 1),
                DPL3,
                true,
            ),
        )
        .unwrap();

        let rsp: u64;
        asm!("mov {0}, rsp", out(reg) rsp);
        gdt.tss.stack_pointer[0] = rsp;

        gdt.reload();

        // The initial value of the Rflags register is interrupt allowed.
        let rflags = Rflags::IF;

        // Reproduce the stack at the time of pseudo interrupt, return with an IRETQ, and transition to user mode.
        asm!("
            mov ds, {new_ss:e}
            mov es, {new_ss:e}
            mov fs, {new_ss:e}
            mov gs, {new_ss:e}
            push {new_ss}
            push {new_sp}
            push {new_fl}
            push {new_cs}
            push {new_ip}
            xor eax, eax
            xor ebx, ebx
            xor ecx, ecx
            xor edx, edx
            xor ebp, ebp
            xor esi, esi
            xor edi, edi
            iretq
            ",
            new_ss = in (reg) LEGACY_DSEL.as_usize(),
            new_cs = in (reg) LEGACY_CSEL.as_usize(),
            new_sp = in (reg) ctx.stack_pointer as usize,
            new_ip = in (reg) ctx.start as usize,
            new_fl = in (reg) rflags.bits(),
            options(noreturn));
    }
}

/// CPU specific context data
#[repr(C, align(64))]
pub struct CpuContextData {
    _regs: [u64; ContextIndex::Max as usize],
}

macro_rules! context_index {
    { $( $name:ident , )* } => {
        $(
            paste! {
                pub const [<CTX_ $name>] : usize = ContextIndex::$name.to_offset();
            }
        )*
    };
}

impl CpuContextData {
    pub const SIZE_OF_CONTEXT: usize = 1024;
    pub const SIZE_OF_STACK: usize = 0x10000;

    context_index! { RSP, RBP, RBX, R12, R13, R14, R15, USER_CS_DESC, USER_DS_DESC, TSS_RSP0, FPU, }
    pub const CTX_DS: usize = ContextIndex::Segs.to_offset() + 0;
    pub const CTX_ES: usize = ContextIndex::Segs.to_offset() + 2;
    pub const CTX_FS: usize = ContextIndex::Segs.to_offset() + 4;
    pub const CTX_GS: usize = ContextIndex::Segs.to_offset() + 6;

    #[inline]
    pub const fn new() -> Self {
        Self {
            _regs: [0; ContextIndex::Max as usize],
        }
    }

    #[inline]
    pub unsafe fn init(&mut self, new_sp: *mut c_void, start: usize, arg: usize) {
        asm!("
            sub {new_sp}, 0x18
            mov [{new_sp}], {new_thread}
            mov [{new_sp} + 0x08], {start}
            mov [{new_sp} + 0x10], {arg}
            mov [{self} + {CTX_RSP}], {new_sp}
            xor {temp:e}, {temp:e}
            mov [{self} + {CTX_USER_CS}], {temp}
            mov [{self} + {CTX_USER_DS}], {temp}
            ",
            self = in(reg) self,
            new_sp = in(reg) new_sp,
            start = in(reg) start,
            arg = in(reg) arg,
            new_thread = in(reg) Self::_new_thread,
            temp = out(reg) _,
            CTX_RSP = const Self::CTX_RSP,
            CTX_USER_CS = const Self::CTX_USER_CS_DESC,
            CTX_USER_DS = const Self::CTX_USER_DS_DESC,
        );
    }

    #[inline]
    pub unsafe fn switch(&self, other: &Self) {
        let gdt = GlobalDescriptorTable::current();
        Self::_switch(self, other, gdt);
    }

    #[naked]
    unsafe extern "C" fn _switch(
        current: *const Self,
        other: *const Self,
        gdt: *mut GlobalDescriptorTable,
    ) {
        asm!(
            "
            mov [rdi + {CTX_RSP}], rsp
            mov [rdi + {CTX_RBP}], rbp
            mov [rdi + {CTX_RBX}], rbx
            mov [rdi + {CTX_R12}], r12
            mov [rdi + {CTX_R13}], r13
            mov [rdi + {CTX_R14}], r14
            mov [rdi + {CTX_R15}], r15
            mov [rdi + {CTX_DS}], ds
            mov [rdi + {CTX_ES}], es
            mov [rdi + {CTX_FS}], fs
            mov [rdi + {CTX_GS}], gs
            fxsave [rdi + {CTX_FPU}]

            mov rax, [rsi + {CTX_USER_CS}]
            xchg rax, [rdx + {USER_CS_IDX} * 8]
            mov [rdi + {CTX_USER_CS}], rax
        
            mov rax, [rsi + {CTX_USER_DS}]
            xchg rax, [rdx + {USER_DS_IDX} * 8]
            mov [rdi + {CTX_USER_DS}], rax

            mov rax, [rsi + {CTX_TSS_RSP0}]
            xchg rax, [rdx + {OFFSET_TSS} + {TSS_OFF_RSP0}]
            mov [rdi + {CTX_TSS_RSP0}], rax

            fxrstor [rsi + {CTX_FPU}]
            mov rsp, [rsi + {CTX_RSP}]
            mov rbp, [rsi + {CTX_RBP}]
            mov rbx, [rsi + {CTX_RBX}]
            mov r12, [rsi + {CTX_R12}]
            mov r13, [rsi + {CTX_R13}]
            mov r14, [rsi + {CTX_R14}]
            mov r15, [rsi + {CTX_R15}]
            mov ds, [rsi + {CTX_DS}]
            mov es, [rsi + {CTX_ES}]
            mov fs, [rsi + {CTX_FS}]
            mov gs, [rsi + {CTX_GS}]

            xor eax, eax
            xor ecx, ecx
            xor edx, edx
            xor esi, esi
            xor edi, edi
            xor r8, r8
            xor r9, r9
            xor r10, r10
            xor r11, r11
            ret
            ",
            CTX_RSP = const Self::CTX_RSP,
            CTX_RBP = const Self::CTX_RBP,
            CTX_RBX = const Self::CTX_RBX,
            CTX_R12 = const Self::CTX_R12,
            CTX_R13 = const Self::CTX_R13,
            CTX_R14 = const Self::CTX_R14,
            CTX_R15 = const Self::CTX_R15,
            CTX_FPU = const Self::CTX_FPU,
            CTX_TSS_RSP0 = const Self::CTX_TSS_RSP0,
            OFFSET_TSS = const GlobalDescriptorTable::OFFSET_TSS,
            TSS_OFF_RSP0 = const TaskStateSegment64::OFFSET_RSP0,
            CTX_DS = const Self::CTX_DS,
            CTX_ES = const Self::CTX_ES,
            CTX_FS = const Self::CTX_FS,
            CTX_GS = const Self::CTX_GS,
            CTX_USER_CS = const Self::CTX_USER_CS_DESC,
            CTX_USER_DS = const Self::CTX_USER_DS_DESC,
            USER_CS_IDX = const LEGACY_CSEL.index(),
            USER_DS_IDX = const LEGACY_DSEL.index(),
            options(noreturn)
        );
    }

    #[naked]
    unsafe extern "C" fn _new_thread() {
        asm!(
            "
            fninit
            mov eax, 0x00001F80
            push rax
            ldmxcsr [rsp]
            pop rax
            pxor xmm0, xmm0
            pxor xmm1, xmm1
            pxor xmm2, xmm2
            pxor xmm3, xmm3
            pxor xmm4, xmm4
            pxor xmm5, xmm5
            pxor xmm6, xmm6
            pxor xmm7, xmm7
            pxor xmm8, xmm8
            pxor xmm9, xmm9
            pxor xmm10, xmm10
            pxor xmm11, xmm11
            pxor xmm12, xmm12
            pxor xmm13, xmm13
            pxor xmm14, xmm14
            pxor xmm15, xmm15

            call {setup_new_thread}

            sti
            pop rax
            pop rdi
            call rax
            ",
            setup_new_thread = sym task::scheduler::setup_new_thread,
            options(noreturn)
        );
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ContextIndex {
    USER_CS_DESC = 2,
    USER_DS_DESC,
    RSP,
    RBP,
    RBX,
    R12,
    R13,
    R14,
    R15,
    TSS_RSP0,
    Segs,
    FPU = 32,
    Max = (Self::FPU as usize) + (512 / size_of::<usize>()),
}

impl ContextIndex {
    #[inline]
    pub const fn to_offset(&self) -> usize {
        size_of::<usize>() * (*self as usize)
    }
}

#[repr(C, align(16))]
pub struct GlobalDescriptorTable {
    table: [DescriptorEntry; Self::NUM_ITEMS],
    tss: TaskStateSegment64,
}

impl !Send for GlobalDescriptorTable {}

impl GlobalDescriptorTable {
    pub const NUM_ITEMS: usize = 16;
    pub const OFFSET_TSS: usize = 8 * Self::NUM_ITEMS;

    #[inline]
    unsafe fn new() -> Box<Self> {
        let mut gdt = Box::new(GlobalDescriptorTable {
            table: [DescriptorEntry::null(); Self::NUM_ITEMS],
            tss: TaskStateSegment64::new(),
        });

        gdt.set_item(KERNEL_CSEL, DescriptorEntry::flat_code_segment(DPL0, USE64))
            .unwrap();
        gdt.set_item(KERNEL_DSEL, DescriptorEntry::flat_data_segment(DPL0))
            .unwrap();

        gdt.set_item(USER_CSEL, DescriptorEntry::flat_code_segment(DPL3, USE64))
            .unwrap();
        gdt.set_item(USER_DSEL, DescriptorEntry::flat_data_segment(DPL3))
            .unwrap();

        let tss_pair = gdt.tss.as_descriptor_pair();
        let tss_index = SYSTEM_TSS.index();
        gdt.table[tss_index] = tss_pair.low;
        gdt.table[tss_index + 1] = tss_pair.high;

        gdt.reload();
        asm!("
            mov {temp}, rsp
            push {new_ss:r}
            push {temp}
            pushfq
            push {new_cs:r}
            .byte 0xE8, 2, 0, 0, 0, 0xEB, 0x02, 0x48, 0xCF
            mov ds, {new_ss:e}
            mov es, {new_ss:e}
            mov fs, {new_ss:e}
            mov gs, {new_ss:e}
            ", 
            temp = out(reg) _,
            new_ss = in(reg) KERNEL_DSEL.as_usize(),
            new_cs = in(reg) KERNEL_CSEL.as_usize(),
        );

        asm!("ltr {0:x}", in(reg) SYSTEM_TSS.0);

        gdt
    }

    #[inline]
    pub unsafe fn item(&self, selector: Selector) -> Option<&DescriptorEntry> {
        let index = selector.index();
        self.table.get(index)
    }

    #[inline]
    pub unsafe fn item_mut(&mut self, selector: Selector) -> Option<&mut DescriptorEntry> {
        let index = selector.index();
        self.table.get_mut(index)
    }

    #[inline]
    pub unsafe fn set_item(
        &mut self,
        selector: Selector,
        desc: DescriptorEntry,
    ) -> Result<(), SetDescriptorError> {
        let index = selector.index();
        if selector.rpl() != desc.dpl().as_rpl() {
            return Err(SetDescriptorError::PriviledgeMismatch);
        }
        self.table
            .get_mut(index)
            .map(|v| *v = desc)
            .ok_or(SetDescriptorError::OutOfIndex)
    }

    #[inline]
    pub unsafe fn current<'a>() -> &'a mut GlobalDescriptorTable {
        let gdt: usize;
        asm!("
            sub rsp, 16
            sgdt [rsp + 6]
            mov {0}, [rsp + 8]
            add rsp, 16
            ", out(reg) gdt
        );
        &mut *(gdt as *mut GlobalDescriptorTable)
    }

    /// Reload GDT
    unsafe fn reload(&self) {
        asm!("
            push {0}
            push {1}
            lgdt [rsp + 6]
            add rsp, 16
            ", in(reg) &self.table, in(reg) ((self.table.len() * 8 - 1) << 48));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetDescriptorError {
    OutOfIndex,
    PriviledgeMismatch,
}

static mut IDT: UnsafeCell<InterruptDescriptorTable> =
    UnsafeCell::new(InterruptDescriptorTable::new());

#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    table: [DescriptorEntry; Self::MAX * 2],
}

impl !Send for InterruptDescriptorTable {}

macro_rules! register_exception {
    ($mnemonic:ident) => {
        paste! {
            Self::register(
                ExceptionType::$mnemonic.as_vec(),
                [<exc_ $mnemonic>] as usize,
                DPL0,
            );
        }
    };
}

impl InterruptDescriptorTable {
    const MAX: usize = 256;

    const fn new() -> Self {
        InterruptDescriptorTable {
            table: [DescriptorEntry::null(); Self::MAX * 2],
        }
    }

    #[inline]
    unsafe fn init() {
        register_exception!(DivideError);
        register_exception!(Breakpoint);
        register_exception!(InvalidOpcode);
        register_exception!(DeviceNotAvailable);
        register_exception!(DoubleFault);
        register_exception!(GeneralProtection);
        register_exception!(PageFault);
        register_exception!(MachineCheck);
        register_exception!(SimdException);

        {
            // Haribote OS Supports
            let vec = InterruptVector(0x40);
            Self::register(vec, cpu_int40_handler as usize, DPL3);
        }
    }

    #[inline]
    unsafe fn load() {
        let idt = &*IDT.get();
        asm!("
            push {0}
            push {1}
            lidt [rsp + 6]
            add rsp, 16
            ", in(reg) &(idt.table), in(reg) ((idt.table.len() * 8 - 1) << 48));
    }

    #[track_caller]
    pub unsafe fn register(vec: InterruptVector, offset: usize, dpl: DPL) {
        let table_offset = vec.0 as usize * 2;
        let idt = IDT.get_mut();
        if !idt.table[table_offset].is_null() {
            panic!("IDT entry #{} is already in use", vec.0);
        }
        let pair = DescriptorEntry::gate64(
            Offset64(offset as u64),
            KERNEL_CSEL,
            dpl,
            if dpl == DPL0 {
                DescriptorType::InterruptGate
            } else {
                DescriptorType::TrapGate
            },
            None,
        );
        idt.table[table_offset + 1] = pair.high;
        idt.table[table_offset] = pair.low;
        fence(Ordering::SeqCst);
    }
}

#[allow(dead_code)]
#[repr(C)]
pub(super) struct X64ExceptionContext {
    _mxcsr: u64,
    cr2: u64,
    _gs: u64,
    _fs: u64,
    _es: u64,
    _ds: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    _vector: u64,
    _error_code: u64,
    rip: u64,
    _cs: u64,
    rflags: Rflags,
    rsp: u64,
    _ss: u64,
}

impl X64ExceptionContext {
    #[inline]
    pub const fn cs(&self) -> Selector {
        Selector(self._cs as u16)
    }

    #[inline]
    pub const fn ds(&self) -> Selector {
        Selector(self._ds as u16)
    }

    #[inline]
    pub const fn es(&self) -> Selector {
        Selector(self._es as u16)
    }

    #[inline]
    pub const fn fs(&self) -> Selector {
        Selector(self._fs as u16)
    }

    #[inline]
    pub const fn gs(&self) -> Selector {
        Selector(self._gs as u16)
    }

    #[inline]
    pub const fn ss(&self) -> Selector {
        Selector(self._ss as u16)
    }

    #[inline]
    pub const fn error_code(&self) -> u16 {
        self._error_code as u16
    }

    #[inline]
    pub const fn vector(&self) -> InterruptVector {
        InterruptVector(self._vector as u8)
    }

    #[inline]
    pub const fn mxcsr(&self) -> u32 {
        self._mxcsr as u32
    }
}

static GLOBAL_EXCEPTION_LOCK: Spinlock = Spinlock::new();

unsafe extern "C" fn handle_default_exception(ctx: &X64ExceptionContext) {
    let is_user = GLOBAL_EXCEPTION_LOCK.synchronized(|| {
        let is_user = Scheduler::current_personality().is_some();
        let stdout = if is_user {
            System::stdout()
        } else {
            System::log()
        };
        stdout.set_attribute(0x0F);

        let cs_desc = GlobalDescriptorTable::current().item(ctx.cs()).unwrap();
        let ex = ExceptionType::from_vec(ctx.vector());

        match cs_desc.default_operand_size() {
            Some(USE16) | Some(USE32) => {
                let mask32 = u32::MAX as u64;
                match ex {
                    ExceptionType::PageFault => {
                        writeln!(
                            stdout,
                            "\n#### PAGE FAULT {:04x} {:08x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ctx.error_code(),
                            ctx.cr2 & mask32,
                            ctx.cs().0,
                            ctx.rip & mask32,
                            ctx.ss().0,
                            ctx.rsp & mask32,
                        )
                        .unwrap();
                    }
                    _ => {
                        writeln!(
                            stdout,
                            "\n#### EXCEPTION {:?} ({}) err {:04x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ex,
                            ex.mnemonic(),
                            ctx.error_code(),
                            ctx.cs().0,
                            ctx.rip & mask32,
                            ctx.ss().0,
                            ctx.rsp & mask32,
                        )
                        .unwrap();
                    }
                }

                println!(
                    "EAX {:08x} EBX {:08x} ECX {:08x} EDX {:08x} EFLAGS {:08x}",
                    ctx.rax & mask32,
                    ctx.rbx & mask32,
                    ctx.rcx & mask32,
                    ctx.rdx & mask32,
                    ctx.rflags.bits(),
                );
                println!(
                    "EBP {:08x} ESI {:08x} EDI {:08x} DS {:04x} ES {:04x} FS {:04x} GS {:04x}",
                    ctx.rbp & mask32,
                    ctx.rsi & mask32,
                    ctx.rdi & mask32,
                    ctx.ds().0,
                    ctx.es().0,
                    ctx.fs().0,
                    ctx.gs().0,
                );
            }
            _ => {
                // use64
                let va_mask = 0xFFFF_FFFF_FFFF;
                match ex {
                    ExceptionType::PageFault => {
                        match ctx.cr2 >> 47 {
                            0x0_0000 | 0x1_FFFF => {
                                // Canonical Address
                                writeln!(
                                    stdout,
                                    "\n#### PAGE FAULT {:04x} {:012x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                    ctx.error_code(),
                                    ctx.cr2 & 0xFFFF_FFFF_FFFF,
                                    ctx.cs().0,
                                    ctx.rip & va_mask,
                                    ctx.ss().0,
                                    ctx.rsp & va_mask,
                                )
                                    .unwrap();
                                    }
                            _ => {
                                // Non Canonical Address (BUG?)
                                writeln!(
                                    stdout,
                                    "\n#### PAGE FAULT {:04x} {:016x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                    ctx.error_code(),
                                    ctx.cr2,
                                    ctx.cs().0,
                                    ctx.rip & va_mask,
                                    ctx.ss().0,
                                    ctx.rsp & va_mask,
                                )
                                    .unwrap();
                                    }
                        }
                    }
                    ExceptionType::SimdException => {
                        writeln!(
                            stdout,
                            "\n#### SIMD EXCEPTION {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                            ctx.mxcsr(),
                            ctx.cs().0,
                            ctx.rip & va_mask,
                            ctx.ss().0,
                            ctx.rsp & va_mask,
                        )
                            .unwrap();
                        }
                    _ => {
                        if ex.has_error_code() {
                            writeln!(
                                stdout,
                                "\n#### EXCEPTION {:?} ({}) err {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                ex,
                                ex.mnemonic(),
                                ctx.error_code(),
                                ctx.cs().0,
                                ctx.rip & va_mask,
                                ctx.ss().0,
                                ctx.rsp & va_mask,
                            )
                            .unwrap();
                        } else {
                            writeln!(
                                stdout,
                                "\n#### EXCEPTION {:?} ({}) rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                ex,
                                ex.mnemonic(),
                                ctx.cs().0,
                                ctx.rip & va_mask,
                                ctx.ss().0,
                                ctx.rsp & va_mask,
                            )
                            .unwrap();
                        }
                    }
                }

                writeln!(
                    stdout,
                    "rax {:016x} rsi {:016x} r11 {:016x} fl {:08x}
rbx {:016x} rdi {:016x} r12 {:016x} ds {:04x}
rcx {:016x} r8  {:016x} r13 {:016x} es {:04x}
rdx {:016x} r9  {:016x} r14 {:016x} fs {:04x}
rbp {:016x} r10 {:016x} r15 {:016x} gs {:04x}",
                    ctx.rax,
                    ctx.rsi,
                    ctx.r11,
                    ctx.rflags.bits(),
                    ctx.rbx,
                    ctx.rdi,
                    ctx.r12,
                    ctx.ds().0,
                    ctx.rcx,
                    ctx.r8,
                    ctx.r13,
                    ctx.es().0,
                    ctx.rdx,
                    ctx.r9,
                    ctx.r14,
                    ctx.fs().0,
                    ctx.rbp,
                    ctx.r10,
                    ctx.r15,
                    ctx.gs().0,
                )
                .unwrap();
            }
        }

        stdout.set_attribute(0x00);
        is_user
    });

    if is_user {
        RuntimeEnvironment::exit(1);
    } else {
        panic!("Unhandled Exception in kernel mode");
    }
}

macro_rules! exception_handler {
    ($mnemonic:ident, $handler:ident) => {
        paste! {
            #[naked]
            #[allow(non_snake_case)]
            unsafe extern "C" fn [<exc_ $mnemonic>]() {
                asm!("
                push ${exno}
                push rax
                push rcx
                push rdx
                push rbx
                push rbp
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15
                mov eax, ds
                push rax
                mov ecx, es
                push rcx

                // To avoid push fs/gs bugs
                .byte 0x0F, 0xA0
                .byte 0x0F, 0xA8

                mov rax, cr2
                push rax
                xor eax, eax
                push rax
                stmxcsr [rsp]
                mov rbp, rsp
                and rsp, 0xfffffffffffffff0
                cld
            
                mov rdi, rbp
                call {handler}

                lea rsp, [rbp + 8 * 6]
                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rbp
                pop rbx
                pop rdx
                pop rcx
                pop rax
                add rsp, 16
                iretq
                ",
                exno = const ExceptionType::$mnemonic.as_vec().0 as usize,
                handler = sym $handler,
                options(noreturn));
            }
        }
    };
}

macro_rules! exception_handler_noerr {
    ($mnemonic:ident, $handler:ident) => {
        paste! {
            #[naked]
            #[allow(non_snake_case)]
            unsafe extern "C" fn [<exc_ $mnemonic>]() {
                asm!("
                push 0
                push ${exno}
                push rax
                push rcx
                push rdx
                push rbx
                push rbp
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15
                mov eax, ds
                push rax
                mov ecx, es
                push rcx

                // To avoid push fs/gs bugs
                .byte 0x0F, 0xA0
                .byte 0x0F, 0xA8

                mov rax, cr2
                push rax
                xor eax, eax
                push rax
                stmxcsr [rsp]
                mov rbp, rsp
                and rsp, 0xfffffffffffffff0
                cld
            
                mov rdi, rbp
                call {handler}

                lea rsp, [rbp + 8 * 6]
                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rbp
                pop rbx
                pop rdx
                pop rcx
                pop rax
                add rsp, 16
                iretq
                ",
                exno = const ExceptionType::$mnemonic.as_vec().0 as usize,
                handler = sym $handler,
                options(noreturn));
            }
        }
    };
}

exception_handler_noerr!(DivideError, handle_default_exception);
exception_handler_noerr!(Breakpoint, handle_default_exception);
exception_handler_noerr!(InvalidOpcode, handle_default_exception);
exception_handler_noerr!(DeviceNotAvailable, handle_default_exception);
exception_handler!(DoubleFault, handle_default_exception);
exception_handler!(GeneralProtection, handle_default_exception);
exception_handler!(PageFault, handle_default_exception);
exception_handler_noerr!(SimdException, handle_default_exception);
exception_handler_noerr!(MachineCheck, handle_default_exception);

// Haribote OS System call Emulation
#[naked]
unsafe extern "C" fn cpu_int40_handler() {
    asm!(
        "
    push rbp
    sub rsp, 24
    mov rbp, rsp
    mov [rbp], eax
    mov [rbp + 4], ecx
    mov [rbp + 8], edx
    mov [rbp + 12], ebx
    mov [rbp + 16], esi
    mov [rbp + 20], edi
    mov eax, [rbp + 32]
    mov [rbp + 28], eax
    and rsp, 0xfffffffffffffff0
    cld

    mov rdi, rbp
    call hoe_syscall

    mov eax, [rbp]
    mov ecx, [rbp + 4]
    mov edx, [rbp + 8]
    mov ebx, [rbp + 12]
    mov esi, [rbp + 16]
    mov edi, [rbp + 20]
    mov r8d, [rbp + 24]
    lea rsp, [rbp + 8 * 4]
    mov ebp, r8d
    iretq
    ",
        options(noreturn)
    );
}

#[repr(C)]
pub struct LegacySyscallContext {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub eip: u32,
}
