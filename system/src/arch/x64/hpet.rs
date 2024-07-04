use super::apic::*;
use crate::mem::mmio::*;
use crate::task::scheduler::*;
use crate::*;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::time::Duration;

/// High Precision Event Timer
pub(super) struct Hpet {
    mmio: MmioSlice,
    main_cnt_period: u64,
    measure_div: u64,
}

impl Hpet {
    pub unsafe fn new(info: &myacpi::hpet::Hpet) -> Self {
        let mut hpet = Hpet {
            mmio: MmioSlice::from_phys(PhysicalAddress::new(info.base_address()), 0x1000).unwrap(),
            main_cnt_period: 0,
            measure_div: 0,
        };

        Irq::LPC_TIMER.register(Self::irq_handler, 0).unwrap();

        let id_reg = hpet.read(0);
        if (id_reg & 0x2000) == 0 {
            COUNTER_32BIT.store(true, Ordering::SeqCst);
        }
        hpet.main_cnt_period = id_reg >> 32;
        hpet.write(0x10, 0);
        hpet.write(0x20, 0); // Clear all interrupts
        hpet.write(0xF0, 0); // Reset MAIN_COUNTER_VALUE
        hpet.write(0x10, 0x03); // LEG_RT_CNF | ENABLE_CNF

        hpet.measure_div = 1000_000_000 / hpet.main_cnt_period;
        hpet.write(0x100, 0x0000_004C); // Tn_INT_ENB_CNF | Tn_TYPE_CNF | Tn_VAL_SET_CNF
        hpet.write(0x108, 1000_000_000_000 / hpet.main_cnt_period);

        // Disable other timers
        for i in 1..32 {
            hpet.write(0x100 + i * 0x20, 0);
        }

        hpet
    }

    #[inline]
    unsafe fn read(&self, offset: usize) -> u64 {
        self.mmio.read_u64(offset)
    }

    #[inline]
    unsafe fn write(&self, offset: usize, value: u64) {
        self.mmio.write_u64(offset, value);
    }

    /// IRQ of HPET
    fn irq_handler(_: usize) {
        HPET_TICK.fetch_add(1, Ordering::SeqCst);
    }

    #[inline]
    fn main_counter_value(&self) -> u64 {
        if COUNTER_32BIT.load(Ordering::Relaxed) {
            todo!()
        } else {
            unsafe { self.read(0xF0) }
        }
    }
}

static HPET_TICK: AtomicU64 = AtomicU64::new(0);
static COUNTER_32BIT: AtomicBool = AtomicBool::new(false);

impl TimerSource for Hpet {
    fn monotonic(&self) -> u64 {
        HPET_TICK.load(Ordering::Relaxed)
    }

    fn measure(&self) -> TimeSpec {
        if COUNTER_32BIT.load(Ordering::Relaxed) {
            TimeSpec((1000 * HPET_TICK.load(Ordering::Relaxed)) as isize)
        } else {
            TimeSpec((self.main_counter_value() / self.measure_div) as isize)
        }
    }

    fn from_duration(&self, val: Duration) -> TimeSpec {
        TimeSpec(val.as_micros() as isize)
    }

    fn into_duration(&self, val: TimeSpec) -> Duration {
        Duration::from_micros(val.0 as u64)
    }
}
