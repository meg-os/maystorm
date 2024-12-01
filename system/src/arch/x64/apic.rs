//! Advanced Programmable Interrupt Controller

use super::cpu::*;
use super::hpet::*;
use super::page::PageManager;
use crate::mem::mmio::*;
use crate::mem::*;
use crate::sync::{semaphore::BinarySemaphore, spinlock::SpinMutex};
use crate::system::*;
use crate::task::scheduler::*;
use crate::*;
use core::cell::UnsafeCell;
use core::mem::{size_of, transmute, ManuallyDrop};
use core::ptr::addr_of;
use core::ptr::{copy_nonoverlapping, null_mut};
use core::sync::atomic::*;
use core::time::Duration;
use myacpi::madt;
use seq_macro::seq;
use x86::msr::MSR;
use x86::prot::InterruptVector;
use x86::prot::DPL0;

pub type AffinityBits = usize;
pub type AtomicAffinityBits = AtomicUsize;

/// Maximum number of supported cpu cores
const MAX_CPU: usize = 8 * size_of::<AffinityBits>();

const STACK_CHUNK_SIZE: usize = 0x4000;

/// Maximum number of supported IOAPIC's IRQ
const MAX_IOAPIC_IRQS: usize = 48;

/// Maximum number of supported MSI IRQ
const MAX_MSI: isize = 16;

#[allow(dead_code)]
const MAX_IRQ: usize = MAX_IOAPIC_IRQS + MAX_MSI as usize;

pub const IPI_INVALIDATE_TLB: InterruptVector = InterruptVector(0xEE);

pub const IPI_SCHEDULE: InterruptVector = InterruptVector(0xFC);

static mut APIC: UnsafeCell<Apic> = UnsafeCell::new(Apic::new());

const INVALID_PROCESSOR_INDEX: u8 = 0xFF;
static mut CURRENT_PROCESSOR_INDEXES: [u8; 256] = [INVALID_PROCESSOR_INDEX; 256];

type FnPrepareSipi =
    extern "C" fn(max_cpu: usize, stacks: *const *mut u8, start_ap: unsafe extern "C" fn());

static AP_STALLED: AtomicBool = AtomicBool::new(true);
static AP_BOOT_OK: AtomicBool = AtomicBool::new(false);

#[no_mangle]
unsafe extern "C" fn apic_start_ap() {
    let apic_id = LocalApic::init_ap();
    System::activate_cpu(Cpu::new(apic_id));

    AP_BOOT_OK.store(true, Ordering::SeqCst);

    // Waiting for TSC synchonization
    while AP_STALLED.load(Ordering::Relaxed) {
        Hal::cpu().spin_loop_hint();
    }

    for (index, cpu) in System::cpus().enumerate() {
        if cpu.apic_id() == apic_id {
            cpu.set_tsc_base(Cpu::rdtsc());
            MSR::IA32_TSC_AUX.write(index as u64);
            break;
        }
    }

    Hal::cpu().enable_interrupt();
    loop {
        // assert!(Hal::cpu().is_interrupt_enabled());
        Hal::cpu().wait_for_interrupt();
    }
}

/// Advanced Programmable Interrupt Controller
pub(super) struct Apic {
    master_apic_id: ApicId,
    ioapics: Vec<SpinMutex<IoApic>>,
    gsi_table: [GsiProps; 256],
    idt: [usize; Irq::MAX.0 as usize],
    idt_params: [usize; Irq::MAX.0 as usize],
    lapic_timer_value: u32,
    tlb_flush_bitmap: AtomicAffinityBits,
    ipi_mutex: BinarySemaphore,
}

impl Apic {
    const REDIR_MASK: u32 = 0x00010000;
    const MSI_DATA: u16 = 0xC000;
    const MSI_BASE: u64 = 0xFEE00000;

    const fn new() -> Self {
        Apic {
            master_apic_id: ApicId(0),
            ioapics: Vec::new(),
            gsi_table: [GsiProps::default(); 256],
            idt: [0; Irq::MAX.0 as usize],
            idt_params: [0; Irq::MAX.0 as usize],
            lapic_timer_value: 0,
            tlb_flush_bitmap: AtomicAffinityBits::new(0),
            ipi_mutex: BinarySemaphore::new(),
        }
    }

    pub unsafe fn init(madt: &madt::Madt) {
        if madt.has_8259() {
            // disable legacy PICs
            Cpu::out8(0xA1, 0xFF);
            Cpu::out8(0x21, 0xFF);
        }

        Hal::cpu().disable_interrupt();

        let shared = Self::shared_mut();

        // init Local Apic
        let mut lapics = madt.local_apics();
        let bsp = lapics.next().unwrap();
        shared.master_apic_id = bsp.apic_id().into();
        CURRENT_PROCESSOR_INDEXES[shared.master_apic_id.0 as usize] = 0;
        LocalApic::init(PhysicalAddress::new(madt.local_apic_address() as u64));

        MSR::IA32_TSC_AUX.write(0);

        // Define Default GSI table for ISA devices
        for irq in &[1, 12] {
            shared.gsi_table[*irq as usize] = GsiProps {
                global_irq: Irq(*irq),
                trigger: PackedTriggerMode(0),
            };
        }

        // import GSI table from ACPI
        for source in madt.entries::<madt::InterruptSourceOverride>() {
            let props = GsiProps {
                global_irq: Irq(source.global_system_interrupt() as u8),
                trigger: PackedTriggerMode(source.flags()),
            };
            shared.gsi_table[source.source() as usize] = props;
        }

        // Init IO Apics
        for acpi_ioapic in madt.entries::<madt::IoApic>() {
            shared
                .ioapics
                .push(SpinMutex::new(IoApic::new(acpi_ioapic)));
        }

        seq!(N in 1..64 {
            InterruptDescriptorTable::register(
                Irq(N).into(),
                handle_irq_~N as usize,
                DPL0,
            );
        });

        // then enable irq
        fence(Ordering::SeqCst);
        Hal::cpu().enable_interrupt();

        // Local APIC Timer
        let vec_latimer = Irq(0).as_vec();
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        if let Some(hpet_info) = System::acpi().unwrap().find_first::<myacpi::hpet::Hpet>() {
            // Use HPET
            Timer::set_timer(Box::new(Hpet::new(hpet_info)));

            let magic_number = 100;
            Timer::epsilon().repeat_until(|| Hal::cpu().spin_loop_hint());
            let timer = Timer::new(Duration::from_micros(100_0000 / magic_number));
            LocalApic::TimerInitialCount.write(u32::MAX);
            timer.repeat_until(|| Hal::cpu().spin_loop_hint());
            let count = LocalApic::TimerCurrentCount.read() as u64;
            shared.lapic_timer_value = ((u32::MAX as u64 - count) * magic_number / 1000) as u32;
        } else {
            panic!("No Reference Timer found");
        }
        InterruptDescriptorTable::register(vec_latimer, timer_handler as usize, DPL0);
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            shared.lapic_timer_value,
        );

        InterruptDescriptorTable::register(
            IPI_INVALIDATE_TLB,
            ipi_tlb_flush_handler as usize,
            DPL0,
        );

        InterruptDescriptorTable::register(IPI_SCHEDULE, ipi_schedule_handler as usize, DPL0);

        // Start SMP
        let sipi_vec = InterruptVector(MemoryManager::static_alloc_real().unwrap().get());
        let lapics = lapics.collect::<Vec<_>>();
        let max_cpu = usize::min(1 + lapics.len(), MAX_CPU);

        let mut idle_stacks = Vec::with_capacity(max_cpu);
        idle_stacks.push(null_mut());
        for _ in 0..max_cpu {
            let mut stack = ManuallyDrop::new(Vec::<u8>::with_capacity(STACK_CHUNK_SIZE));
            idle_stacks.push(stack.as_mut_ptr().add(STACK_CHUNK_SIZE));
        }

        // Prepare the payload to receive the startup IPI.
        let smpinit_src = include_bytes!("./smpinit.bin");
        let smpinit_dst = ((sipi_vec.0 as usize) << 12) as *mut u8;
        copy_nonoverlapping(smpinit_src.as_ptr(), smpinit_dst, smpinit_src.len());
        let prepare_sipi = transmute::<_, FnPrepareSipi>(smpinit_dst.add(8));
        prepare_sipi(max_cpu, idle_stacks.as_ptr(), apic_start_ap);

        // start Application Processors
        for lapic in lapics.iter().take(max_cpu) {
            let apic_id = ApicId(lapic.apic_id());
            LocalApic::send_init_ipi(apic_id);
            Timer::new(Duration::from_millis(10)).repeat_until(|| Hal::cpu().wait_for_interrupt());

            AP_BOOT_OK.store(false, Ordering::SeqCst);
            LocalApic::send_startup_ipi(apic_id, sipi_vec);
            let deadline = Timer::new(Duration::from_millis(100));
            let mut wait = Hal::cpu().spin_wait();
            while deadline.is_alive() && !AP_BOOT_OK.load(Ordering::SeqCst) {
                wait.wait();
            }
            if !AP_BOOT_OK.load(Ordering::SeqCst) {
                panic!("SMP: Some application processors are not responding");
            }
        }

        drop(idle_stacks);
        // core::mem::forget(idle_stacks);

        for (index, cpu) in System::cpus().enumerate() {
            CURRENT_PROCESSOR_INDEXES[cpu.apic_id().0 as usize] = index as u8;
        }

        AP_STALLED.store(false, Ordering::SeqCst);
        System::cpu(ProcessorIndex(0)).set_tsc_base(Cpu::rdtsc());
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*(&*addr_of!(APIC)).get() }
    }

    #[inline]
    fn shared_mut<'a>() -> &'a mut Self {
        unsafe { &mut *(&*addr_of!(APIC)).get() }
    }

    pub unsafe fn register(irq: Irq, f: IrqHandler, val: usize) -> Result<(), ()> {
        let shared = Self::shared_mut();
        let props = shared.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;
        let trigger = props.trigger;
        if global_irq.0 == 0 {
            return Err(());
        }

        for ioapic in shared.ioapics.iter() {
            let mut ioapic = ioapic.lock();
            let local_irq = global_irq.0 - ioapic.global_int.0;
            if ioapic.global_int <= global_irq && local_irq < ioapic.entries {
                if shared.idt[global_irq.0 as usize] != 0 {
                    return Err(());
                }
                shared.idt[global_irq.0 as usize] = f as usize;
                shared.idt_params[global_irq.0 as usize] = val;
                let pair = Self::make_redirect_table_entry_pair(
                    global_irq.as_vec(),
                    trigger,
                    shared.master_apic_id,
                );
                ioapic.write(IoApicIndex::redir_table_high(local_irq), pair.1);
                ioapic.write(IoApicIndex::redir_table_low(local_irq), pair.0);
                return Ok(());
            }
        }
        Err(())
    }

    pub fn set_irq_enabled(irq: Irq, enabled: bool) -> Result<(), ()> {
        let shared = Self::shared();
        let props = shared.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;

        for ioapic in shared.ioapics.iter() {
            let mut ioapic = ioapic.lock();
            let local_irq = global_irq.0 - ioapic.global_int.0;
            if ioapic.global_int <= global_irq && local_irq < ioapic.entries {
                let mut value = ioapic.read(IoApicIndex::redir_table_low(local_irq * 2));
                if enabled {
                    value &= !Apic::REDIR_MASK;
                } else {
                    value |= Apic::REDIR_MASK;
                }
                ioapic.write(IoApicIndex::redir_table_low(local_irq * 2), value);
                return Ok(());
            }
        }
        Err(())
    }

    const fn make_redirect_table_entry_pair(
        vec: InterruptVector,
        trigger: PackedTriggerMode,
        apic_id: ApicId,
    ) -> (u32, u32) {
        (vec.0 as u32 | trigger.as_redir(), apic_id.as_u32() << 24)
    }

    #[inline]
    pub unsafe fn register_msi(f: fn(usize) -> (), arg: usize) -> Result<(u64, u16), ()> {
        let shared = Self::shared_mut();
        static NEXT_MSI: AtomicIsize = AtomicIsize::new(0);
        NEXT_MSI
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v < MAX_MSI {
                    Some(v + 1)
                } else {
                    None
                }
            })
            .map(|v| {
                let msi = Msi(v);
                let global_irq = msi.as_irq();
                shared.idt[global_irq.0 as usize] = f as usize;
                shared.idt_params[global_irq.0 as usize] = arg;
                fence(Ordering::SeqCst);
                let vec = msi.as_vec();
                let addr = Self::MSI_BASE;
                let data = Self::MSI_DATA | vec.0 as u16;
                (addr, data)
            })
            .map_err(|_| ())
    }

    #[inline]
    #[must_use]
    pub fn broadcast_invalidate_tlb() -> Result<(), ()> {
        let shared = Self::shared();
        match shared.ipi_mutex.synchronized(|| unsafe {
            Irql::IPI.raise(|| {
                let max_cpu = System::current_device().num_of_logical_cpus();
                if max_cpu < 2 {
                    return true;
                }
                shared.tlb_flush_bitmap.store(
                    (((1 as AffinityBits) << max_cpu) - 1)
                        & !((1 as AffinityBits) << Hal::cpu().current_processor_index().0),
                    Ordering::SeqCst,
                );

                LocalApic::broadcast_ipi(IPI_INVALIDATE_TLB);

                let mut hint = Hal::cpu().spin_wait();
                let deadline = Timer::new(Duration::from_millis(200));
                while deadline.is_alive() {
                    if shared.tlb_flush_bitmap.load(Ordering::Relaxed) == 0 {
                        break;
                    }
                    hint.wait();
                }

                shared.tlb_flush_bitmap.load(Ordering::Relaxed) == 0
            })
        }) {
            true => Ok(()),
            false => Err(()),
        }
    }

    #[inline]
    pub fn broadcast_reschedule() {
        LocalApic::broadcast_ipi(IPI_SCHEDULE);
    }

    #[inline]
    unsafe fn handle_irq(irq: Irq) {
        let shared = Self::shared();
        match shared.idt[irq.0 as usize] {
            0 => {
                let _ = irq.disable();
                panic!("IRQ {}: Unconfigured IRQ interrupt has occurred", irq.0);
            }
            entry => {
                let f: IrqHandler = transmute(entry);
                let param = shared.idt_params[irq.0 as usize];
                Irql::Device.raise(|| f(param));
                LocalApic::eoi();
            }
        }
    }
}

pub type IrqHandler = fn(usize) -> ();

seq!(N in 1..64 {
    unsafe extern "x86-interrupt" fn handle_irq_~N () {
        Apic::handle_irq(Irq(N));
    }
});

unsafe extern "x86-interrupt" fn timer_handler() {
    LocalApic::eoi();
    Scheduler::reschedule();
}

unsafe extern "x86-interrupt" fn ipi_schedule_handler() {
    LocalApic::eoi();
    Scheduler::reschedule();
}

unsafe extern "x86-interrupt" fn ipi_tlb_flush_handler() {
    let shared = Apic::shared();
    PageManager::invalidate_all_tlb();
    Hal::sync().fetch_reset(
        &shared.tlb_flush_bitmap,
        Hal::cpu().current_processor_index().0,
    );
    LocalApic::eoi();
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(super) struct ApicId(pub u8);

impl ApicId {
    pub const BROADCAST: Self = Self(u8::MAX);

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl From<u8> for ApicId {
    #[inline]
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<u32> for ApicId {
    #[inline]
    fn from(val: u32) -> Self {
        Self(val as u8)
    }
}

impl From<usize> for ApicId {
    #[inline]
    fn from(val: usize) -> Self {
        Self(val as u8)
    }
}

/// Interrupt Request
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Irq(pub u8);

impl Irq {
    const BASE: InterruptVector = InterruptVector(0x80);
    const MAX: Irq = Irq(127);

    pub const LPC_TIMER: Irq = Irq(0);
    pub const LPC_PS2K: Irq = Irq(1);
    pub const LPC_COM2: Irq = Irq(3);
    pub const LPC_COM1: Irq = Irq(4);
    pub const LPC_FDC: Irq = Irq(6);
    pub const LPC_LPT: Irq = Irq(7);
    pub const LPC_RTC: Irq = Irq(8);
    pub const LPC_PS2M: Irq = Irq(12);
    pub const LPC_IDE1: Irq = Irq(14);
    pub const LPC_IDE2: Irq = Irq(15);

    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(Self::BASE.0 + self.0)
    }

    #[inline]
    pub unsafe fn register(self, f: IrqHandler, val: usize) -> Result<(), ()> {
        Apic::register(self, f, val)
    }

    #[inline]
    pub fn enable(self) -> Result<(), ()> {
        Apic::set_irq_enabled(self, true)
    }

    #[inline]
    pub fn disable(self) -> Result<(), ()> {
        Apic::set_irq_enabled(self, false)
    }
}

impl From<Irq> for InterruptVector {
    #[inline]
    fn from(irq: Irq) -> InterruptVector {
        irq.as_vec()
    }
}

/// Message Signaled Interrupts
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Msi(pub isize);

impl Msi {
    #[inline]
    const fn as_irq(self) -> Irq {
        Irq((MAX_IOAPIC_IRQS as isize + self.0) as u8)
    }

    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        self.as_irq().as_vec()
    }
}

impl From<Msi> for InterruptVector {
    #[inline]
    fn from(msi: Msi) -> Self {
        msi.as_vec()
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct GsiProps {
    global_irq: Irq,
    trigger: PackedTriggerMode,
}

impl GsiProps {
    #[inline]
    const fn default() -> Self {
        GsiProps {
            global_irq: Irq(0),
            trigger: PackedTriggerMode(0),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default)]
struct PackedTriggerMode(pub u16);

impl PackedTriggerMode {
    #[inline]
    const fn as_redir(self) -> u32 {
        (self.0 as u32) << 12
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApicPolarity {
    ActiveHigh = 0,
    ActiveLow = 1,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApicTriggerMode {
    Edge = 0,
    Level = 1,
}

impl ApicTriggerMode {
    #[inline]
    pub const fn as_redir(&self) -> u32 {
        (*self as u32) << 15
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ApicDeliveryMode {
    Fixed = 0,
    Lowest,
    SMI,
    _Reserved3,
    NMI,
    Init,
    StartUp,
    _Reserved7,
}

impl ApicDeliveryMode {
    #[inline]
    const fn as_redir(&self) -> u32 {
        (*self as u32) << 8
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ApicDestinationShorthand {
    NoShortHand = 0,
    _Self,
    AllIncludingSelf,
    AllExcludingSelf,
}

impl ApicDestinationShorthand {
    #[inline]
    const fn as_redir(&self) -> u32 {
        (*self as u32) << 18
    }
}

static LOCAL_APIC_PA: AtomicU64 = AtomicU64::new(0);
static mut LOCAL_APIC: Option<UnsafeCell<MmioSlice>> = None;

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LocalApic {
    Id = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    Eoi = 0xB0,
    SpuriousInterrupt = 0xF0,
    InterruptCommand = 0x300,
    InterruptCommandHigh = 0x310,
    LvtTimer = 0x320,
    LvtLint0 = 0x350,
    LvtLint1 = 0x360,
    LvtError = 0x370,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivideConfiguration = 0x3E0,
}

impl LocalApic {
    pub const IA32_APIC_BASE_MSR_BSP: u64 = 1 << 8;
    // pub const IA32_APIC_BASE_MSR_EXTD: u64 = 1 << 10;
    pub const IA32_APIC_BASE_MSR_EN: u64 = 1 << 11;

    #[inline]
    unsafe fn init(base: PhysicalAddress) {
        assert_call_once!();

        LOCAL_APIC_PA.store(base.as_u64(), Ordering::SeqCst);
        LOCAL_APIC = MmioSlice::from_phys(base, 0x1000).map(|v| UnsafeCell::new(v));

        MSR::IA32_APIC_BASE
            .write(base.as_u64() | Self::IA32_APIC_BASE_MSR_EN | Self::IA32_APIC_BASE_MSR_BSP);
    }

    unsafe fn init_ap() -> ApicId {
        let shared = Apic::shared();
        MSR::IA32_APIC_BASE
            .write(LOCAL_APIC_PA.load(Ordering::Relaxed) | Self::IA32_APIC_BASE_MSR_EN);

        let apicid = LocalApic::current_processor_id();

        LocalApic::SpuriousInterrupt.write(0x010F);

        let vec_latimer = Irq(0).as_vec();
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            shared.lapic_timer_value,
        );

        apicid
    }

    #[inline]
    #[track_caller]
    fn mmio() -> &'static MmioSlice {
        unsafe { &*(&*addr_of!(LOCAL_APIC)).as_ref().unwrap().get() }
    }

    #[inline]
    #[track_caller]
    fn read(self) -> u32 {
        Self::mmio().read_u32(self as usize)
    }

    #[inline]
    #[track_caller]
    fn write(self, val: u32) {
        Self::mmio().write_u32(self as usize, val);
    }

    #[inline]
    #[track_caller]
    fn eoi() {
        Self::Eoi.write(0);
    }

    #[inline]
    #[track_caller]
    fn set_timer_div(div: LocalApicTimerDivide) {
        Self::TimerDivideConfiguration.write(div as u32);
    }

    #[inline]
    #[track_caller]
    fn set_timer(mode: LocalApicTimerMode, vec: InterruptVector, count: u32) {
        Self::TimerInitialCount.write(count);
        Self::LvtTimer.write((vec.0 as u32) | mode as u32);
    }

    #[inline]
    #[track_caller]
    fn clear_timer() {
        Self::LvtTimer.write(Apic::REDIR_MASK);
    }

    #[inline]
    fn send_ipi(
        apic_id: ApicId,
        shorthand: ApicDestinationShorthand,
        trigger_mode: ApicTriggerMode,
        asserted: bool,
        delivery: ApicDeliveryMode,
        init_vec: InterruptVector,
    ) {
        unsafe {
            without_interrupts!({
                Self::InterruptCommandHigh.write((apic_id.0 as u32) << 24);
                Self::InterruptCommand.write(
                    shorthand.as_redir()
                        | trigger_mode.as_redir()
                        | ((asserted as u32) << 14)
                        | delivery.as_redir()
                        | init_vec.0 as u32,
                );
            })
        }
    }

    /// Send Init IPI
    #[inline]
    fn send_init_ipi(apic_id: ApicId) {
        Self::send_ipi(
            apic_id,
            ApicDestinationShorthand::NoShortHand,
            ApicTriggerMode::Edge,
            true,
            ApicDeliveryMode::Init,
            InterruptVector(0),
        );
    }

    /// Send Startup IPI
    #[inline]
    fn send_startup_ipi(apic_id: ApicId, init_vec: InterruptVector) {
        Self::send_ipi(
            apic_id,
            ApicDestinationShorthand::NoShortHand,
            ApicTriggerMode::Edge,
            true,
            ApicDeliveryMode::StartUp,
            init_vec,
        );
    }

    /// Broadcasts an inter-processor interrupt to all excluding self.
    #[inline]
    fn broadcast_ipi(vec: InterruptVector) {
        Self::send_ipi(
            ApicId::BROADCAST,
            ApicDestinationShorthand::AllExcludingSelf,
            ApicTriggerMode::Edge,
            true,
            ApicDeliveryMode::Fixed,
            vec,
        );
    }

    #[inline]
    fn current_processor_id() -> ApicId {
        ApicId((LocalApic::Id.read() >> 24) as u8)
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LocalApicTimerMode {
    OneShot = 0 << 17,
    Periodic = 1 << 17,
    TscDeadline = 2 << 17,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq)]
enum LocalApicTimerDivide {
    By1 = 0b1011,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
struct IoApicIndex(u8);

impl IoApicIndex {
    #[allow(dead_code)]
    const ID: IoApicIndex = IoApicIndex(0x00);
    const VER: IoApicIndex = IoApicIndex(0x01);
    const REDIR_BASE: IoApicIndex = IoApicIndex(0x10);

    #[inline]
    const fn redir_table_low(index: u8) -> Self {
        Self(Self::REDIR_BASE.0 + index * 2)
    }

    #[inline]
    const fn redir_table_high(index: u8) -> Self {
        Self(Self::REDIR_BASE.0 + index * 2 + 1)
    }
}

#[allow(dead_code)]
struct IoApic {
    mmio: MmioSlice,
    global_int: Irq,
    entries: u8,
    id: u8,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &madt::IoApic) -> Self {
        let mut ioapic = IoApic {
            mmio: MmioSlice::from_phys(
                PhysicalAddress::new(acpi_ioapic.io_apic_address() as u64),
                0x14,
            )
            .unwrap(),
            global_int: Irq(acpi_ioapic.gsi_base() as u8),
            entries: 0,
            id: acpi_ioapic.apic_id(),
        };
        let ver = ioapic.read(IoApicIndex::VER);
        ioapic.entries = 1 + (ver >> 16) as u8;
        ioapic
    }

    #[inline]
    fn read(&mut self, index: IoApicIndex) -> u32 {
        self.mmio.write_u8(0x00, index.0);
        self.mmio.read_u32(0x10)
    }

    #[inline]
    fn write(&mut self, index: IoApicIndex, data: u32) {
        self.mmio.write_u8(0x00, index.0);
        self.mmio.write_u32(0x10, data);
    }
}
