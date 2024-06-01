//! Real Time Clock

use crate::system::System;
use crate::task::scheduler::*;
use crate::*;
use core::arch::asm;
use core::num::NonZeroU8;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use core::time::Duration;
use megstd::time::SystemTime;

static RTC: Rtc = Rtc::new();

pub(super) struct Rtc {
    base_secs: AtomicU64,
    offset: AtomicU64,
    century_index: AtomicU8,
}

impl Rtc {
    #[inline]
    const fn new() -> Self {
        Self {
            base_secs: AtomicU64::new(0),
            offset: AtomicU64::new(0),
            century_index: AtomicU8::new(0),
        }
    }

    pub unsafe fn init() {
        let shared = Self::shared();
        shared.century_index.store(
            System::acpi()
                .unwrap()
                .fadt()
                .century_index()
                .map(|v| v.get())
                .unwrap_or(0),
            Ordering::Release,
        );
        shared.base_secs.store(Self::read_time(), Ordering::Release);
        shared
            .offset
            .store(Timer::monotonic().as_nanos() as u64, Ordering::Release);
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        &RTC
    }

    #[inline]
    pub fn base_secs(&self) -> u64 {
        self.base_secs.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn offset(&self) -> u64 {
        self.offset.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn century_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.century_index.load(Ordering::Relaxed))
    }

    pub fn system_time() -> SystemTime {
        let shared = Self::shared();

        let nanos_per_sec = 1_000_000_000;
        let diff = Timer::monotonic().as_nanos() as u64 - shared.offset();
        let secs = shared.base_secs() + diff / nanos_per_sec;
        let nanos = (diff % nanos_per_sec) as u32;

        let duration = Duration::new(secs, nanos);

        SystemTime::UNIX_EPOCH.checked_add(duration).unwrap()
    }

    unsafe fn read_time() -> u64 {
        let time = without_interrupts!(loop {
            let time1 = Self::fetch_raw();
            let time2 = Self::fetch_raw();
            if time1 == time2 {
                break time1;
            }
        });
        let reg_b = Cmos::StatusB.read();
        let is_12h = (reg_b & 2) == 0;
        let is_bcd = (reg_b & 4) == 0;
        let sec = Self::fix_value(time.0, is_bcd) as u64;
        let min = Self::fix_value(time.1, is_bcd) as u64;
        let hour = Self::fix_hour(time.2, is_bcd, is_12h) as u64;
        let day = Self::fix_value(time.3, is_bcd);
        let month = Self::fix_value(time.4, is_bcd);
        let year =
            Self::fix_value(time.5, is_bcd) as u16 + Self::fix_value(time.6, is_bcd) as u16 * 100;

        let date = System::date_to_integer(year, month, day) as u64;

        sec + min * 60 + hour * 3600 + date * 86400
    }

    unsafe fn fetch_raw() -> (u8, u8, u8, u8, u8, u8, u8) {
        let sec = Cmos::Seconds.read();
        let min = Cmos::Minutes.read();
        let hour = Cmos::Hours.read();
        let day = Cmos::DayOfMonth.read();
        let month = Cmos::Month.read();
        let year = Cmos::Year.read();
        let century = match Self::shared().century_index() {
            Some(v) => Cmos::read_direct(v.get()),
            None => 0,
        };
        (sec, min, hour, day, month, year, century)
    }

    #[inline]
    fn bcd_to_dec(bcd: u8) -> u8 {
        ((bcd & 0xF0) >> 1) + ((bcd & 0xF0) >> 3) + (bcd & 0xf)
    }

    #[inline]
    fn fix_value(val: u8, is_bcd: bool) -> u8 {
        if is_bcd {
            Self::bcd_to_dec(val)
        } else {
            val
        }
    }

    #[inline]
    fn fix_hour(val: u8, is_bcd: bool, is_12h: bool) -> u8 {
        let pm = (val & 0x80) != 0;
        let mut val = val & 0x7F;
        if is_bcd {
            val = Self::bcd_to_dec(val);
        }
        if is_12h && val == 12 {
            val = 0;
        }
        if is_12h && pm {
            val += 12;
        }
        val
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
enum Cmos {
    Seconds = 0,
    SecondsAlarm,
    Minutes,
    MinutesAlarm,
    Hours,
    HoursAlarm,
    DayOfWeek,
    DayOfMonth,
    Month,
    Year,
    StatusA,
    StatusB,
    StatusC,
    StatusD,
    Diagnostic,
}

#[allow(dead_code)]
impl Cmos {
    #[inline]
    unsafe fn read(&self) -> u8 {
        unsafe { Self::read_direct(*self as u8) }
    }

    #[inline]
    unsafe fn write(&self, data: u8) {
        asm!("
            mov al, {0}
            out 0x70, al
            nop
            mov al, {1}
            out 0x71, al
            ", in(reg_byte) *self as u8, in(reg_byte) data, out("al") _);
    }

    #[inline]
    unsafe fn read_direct(index: u8) -> u8 {
        let mut result: u8;
        asm!("
            out 0x70, al
            nop
            in al, 0x71
            ", inlateout("al") index => result);
        result
    }
}
