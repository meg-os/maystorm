// PS/2 Device Driver

use super::apic::*;
use crate::io::hid_mgr::*;
use crate::sync::atomic::AtomicWrapperU8;
use crate::task::scheduler::*;
use crate::*;
use core::arch::asm;
use core::mem::transmute;
use core::time::Duration;
use megstd::io::hid::*;

static PS2: Ps2 = Ps2::new();

pub(super) struct Ps2 {
    key_state: AtomicWrapperU8<Ps2KeyState>,

    mouse_phase: AtomicWrapperU8<Ps2MousePhase>,
    mouse_buf_lead: AtomicWrapperU8<MouseLeadByte>,
    mouse_buf_x: AtomicWrapperU8<Ps2Data>,
    mouse_state: MouseState,
}

#[allow(dead_code)]
impl Ps2 {
    const WRITE_TIMEOUT: u64 = 10_000;
    const READ_TIMEOUT: u64 = 100_000;

    const fn new() -> Self {
        Self {
            key_state: AtomicWrapperU8::empty(),
            mouse_phase: AtomicWrapperU8::empty(),
            mouse_buf_lead: AtomicWrapperU8::empty(),
            mouse_buf_x: AtomicWrapperU8::empty(),
            mouse_state: MouseState::empty(),
        }
    }

    pub unsafe fn init() -> Result<(), ()> {
        // NO PS/2 Controller
        match Self::wait_for_write(10) {
            Err(_) => return Err(()),
            Ok(_) => (),
        }

        Self::write_command(Ps2Command::DISABLE_FIRST_PORT);
        Self::send_command(Ps2Command::DISABLE_SECOND_PORT, 10).unwrap();

        for _ in 0..16 {
            let _ = Self::read_data();
        }

        Irq::LPC_PS2K.register(Self::irq_01, 0).unwrap();
        Irq::LPC_PS2M.register(Self::irq_12, 0).unwrap();

        Self::send_command(Ps2Command::WRITE_CONFIG, 10).unwrap();
        Self::send_data(Ps2Data(0x47), 10).unwrap();

        Self::send_command(Ps2Command::ENABLE_FIRST_PORT, 10).unwrap();
        Self::send_command(Ps2Command::ENABLE_SECOND_PORT, 10).unwrap();

        Self::send_data(Ps2Data::RESET_COMMAND, 10).unwrap();
        Timer::sleep(Duration::from_millis(100));
        Self::send_data(Ps2Data::ENABLE_SEND, 10).unwrap();

        Self::send_second_data(Ps2Data::RESET_COMMAND, 10).unwrap();
        Timer::sleep(Duration::from_millis(100));
        // Self::send_second_data(Ps2Data::SET_DEFAULT, 1).unwrap();
        Self::send_second_data(Ps2Data::ENABLE_SEND, 10).unwrap();

        Ok(())
    }

    fn read_data() -> Ps2Data {
        let mut al: u8;
        unsafe {
            asm!("in al, 0x60", lateout("al") al);
        }
        Ps2Data(al)
    }

    fn write_data(data: Ps2Data) {
        unsafe {
            asm!("out 0x60, al", in("al") data.0);
        }
    }

    fn read_status() -> Ps2Status {
        let mut al: u8;
        unsafe {
            asm!("in al, 0x64", lateout("al") al);
            Ps2Status::from_bits_retain(al)
        }
    }

    fn write_command(command: Ps2Command) {
        unsafe {
            asm!("out 0x64, al", in("al") command.0);
        }
    }

    fn wait_for_write(timeout: u64) -> Result<(), Ps2Error> {
        let mut spin_loop = Hal::cpu().spin_wait();
        let deadline = Timer::new(Duration::from_micros(Self::WRITE_TIMEOUT * timeout));
        while deadline.is_alive() {
            if Self::read_status().contains(Ps2Status::INPUT_FULL) {
                spin_loop.wait();
            } else {
                return Ok(());
            }
        }
        Err(Ps2Error::Timeout)
    }

    fn wait_for_read(timeout: u64) -> Result<(), Ps2Error> {
        let mut spin_loop = Hal::cpu().spin_wait();
        let deadline = Timer::new(Duration::from_micros(timeout * Self::READ_TIMEOUT));
        while deadline.is_alive() {
            if Self::read_status().contains(Ps2Status::OUTPUT_FULL) {
                return Ok(());
            } else {
                spin_loop.wait();
            }
        }
        Err(Ps2Error::Timeout)
    }

    // Wait for write, then command
    fn send_command(command: Ps2Command, timeout: u64) -> Result<(), Ps2Error> {
        Self::wait_for_write(timeout).and_then(|_| {
            Self::write_command(command);
            Ok(())
        })
    }

    // Wait for write, then data
    fn send_data(data: Ps2Data, timeout: u64) -> Result<(), Ps2Error> {
        Self::wait_for_write(timeout).and_then(|_| {
            Self::write_data(data);
            Ok(())
        })
    }

    // Send to second port (mouse)
    fn send_second_data(data: Ps2Data, timeout: u64) -> Result<(), Ps2Error> {
        Self::send_command(Ps2Command::WRITE_SECOND_PORT, timeout)
            .and_then(|_| Self::send_data(data, timeout))
    }

    // IRQ 01 PS/2 Keyboard
    fn irq_01(_: usize) {
        PS2.process_key_data(Self::read_data());
    }

    // IRQ 12 PS/2 Mouse
    fn irq_12(_: usize) {
        PS2.process_mouse_data(Self::read_data());
    }

    fn process_key_data(&self, data: Ps2Data) {
        if data == Ps2Data::SCAN_E0 {
            self.key_state.store(Ps2KeyState::PrefixE0);
        } else {
            let flags = if data.is_break() {
                KeyEventFlags::BREAK
            } else {
                KeyEventFlags::empty()
            };
            let mut scancode = data.scancode();
            match self.key_state.value() {
                Ps2KeyState::PrefixE0 => {
                    scancode |= 0x80;
                    self.key_state.store(Ps2KeyState::Default);
                }
                _ => (),
            }
            let usage = Usage(PS2_TO_HID[scancode as usize]);
            KeyEvent::new(usage, Modifier::empty(), flags).post();
        }
    }

    fn process_mouse_data(&self, data: Ps2Data) {
        match self.mouse_phase.value() {
            Ps2MousePhase::Ack => {
                if data == Ps2Data::ACK {
                    self.mouse_phase.next();
                }
            }
            Ps2MousePhase::Leading => {
                let lead = MouseLeadByte::from(data);
                if lead.is_valid() {
                    self.mouse_buf_lead.store(lead);
                    self.mouse_phase.next();
                }
            }
            Ps2MousePhase::X => {
                self.mouse_buf_x.store(data);
                self.mouse_phase.next();
            }
            Ps2MousePhase::Y => {
                let lead = self.mouse_buf_lead.value();
                let data_x = self.mouse_buf_x.value();
                let data_y = data;
                self.mouse_phase.next();

                fn movement(data: Ps2Data, sign: bool) -> i16 {
                    if sign {
                        ((data.0 as u16) | 0xFF00) as i16
                    } else {
                        data.0 as i16
                    }
                }
                let x = movement(data_x, lead.contains(MouseLeadByte::X_SIGN));
                let y = 0 - movement(data_y, lead.contains(MouseLeadByte::Y_SIGN));
                let report = MouseReport {
                    buttons: lead.into(),
                    x,
                    y,
                    wheel: 0,
                };
                self.mouse_state.process_relative_report(report);
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum Ps2Error {
    Unsupported,
    Timeout,
}

#[derive(Debug)]
enum Ps2KeyState {
    Default,
    PrefixE0,
}

impl Default for Ps2KeyState {
    fn default() -> Self {
        Self::Default
    }
}

impl From<u8> for Ps2KeyState {
    #[inline]
    fn from(value: u8) -> Self {
        unsafe { transmute(value) }
    }
}

impl From<Ps2KeyState> for u8 {
    #[inline]
    fn from(value: Ps2KeyState) -> Self {
        value as u8
    }
}

my_bitflags! {
    struct MouseLeadByte: u8 {
        const LEFT_BUTTON = 0b0000_0001;
        const RIGHT_BUTTON = 0b0000_0010;
        const MIDDLE_BUTTON = 0b0000_0100;
        const ALWAYS_ONE = 0b0000_1000;
        const X_SIGN = 0b0001_0000;
        const Y_SIGN = 0b0010_0000;
        const X_OVERFLOW = 0b0100_0000;
        const Y_OVERFLOW = 0b1000_0000;

        const BUTTONS = Self::LEFT_BUTTON.bits() | Self::RIGHT_BUTTON.bits() | Self::MIDDLE_BUTTON.bits();
    }
}

impl MouseLeadByte {
    fn is_valid(self) -> bool {
        self.contains(Self::ALWAYS_ONE)
            && !self.contains(Self::X_OVERFLOW)
            && !self.contains(Self::Y_OVERFLOW)
    }
}

impl From<Ps2Data> for MouseLeadByte {
    fn from(data: Ps2Data) -> Self {
        MouseLeadByte::from_bits_retain(data.0)
    }
}

impl Into<MouseButton> for MouseLeadByte {
    fn into(self) -> MouseButton {
        MouseButton::from_bits_retain(self.bits() & MouseLeadByte::BUTTONS.bits())
    }
}

#[derive(Debug, Copy, Clone)]
enum Ps2MousePhase {
    Ack,
    Leading,
    X,
    Y,
}

impl Ps2MousePhase {
    fn next(&self) -> Self {
        match *self {
            Ps2MousePhase::Ack => Ps2MousePhase::Leading,
            Ps2MousePhase::Leading => Ps2MousePhase::X,
            Ps2MousePhase::X => Ps2MousePhase::Y,
            Ps2MousePhase::Y => Ps2MousePhase::Leading,
        }
    }
}

impl AtomicWrapperU8<Ps2MousePhase> {
    fn next(&self) {
        self.store(self.value().next());
    }
}

impl Default for Ps2MousePhase {
    #[inline]
    fn default() -> Self {
        Self::Ack
    }
}

impl From<u8> for Ps2MousePhase {
    #[inline]
    fn from(value: u8) -> Self {
        unsafe { transmute(value) }
    }
}

impl From<Ps2MousePhase> for u8 {
    #[inline]
    fn from(value: Ps2MousePhase) -> Self {
        value as u8
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
struct Ps2Data(pub u8);

#[allow(dead_code)]
impl Ps2Data {
    const ACK: Ps2Data = Ps2Data(0xFA);
    const NAK: Ps2Data = Ps2Data(0xFE);
    const ECHO: Ps2Data = Ps2Data(0xEE);

    const RESET_COMMAND: Ps2Data = Ps2Data(0xFF);
    const ENABLE_SEND: Ps2Data = Ps2Data(0xF4);
    const DISABLE_SEND: Ps2Data = Ps2Data(0xF5);
    const SET_DEFAULT: Ps2Data = Ps2Data(0xF6);

    const SCAN_E0: Ps2Data = Ps2Data(0xE0);

    const fn is_break(self) -> bool {
        (self.0 & 0x80) != 0
    }

    const fn scancode(self) -> u8 {
        self.0 & 0x7F
    }
}

impl From<u8> for Ps2Data {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<Ps2Data> for u8 {
    #[inline]
    fn from(value: Ps2Data) -> Self {
        value.0
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
struct Ps2Command(pub u8);

#[allow(dead_code)]
impl Ps2Command {
    const WRITE_CONFIG: Ps2Command = Ps2Command(0x60);
    const DISABLE_SECOND_PORT: Ps2Command = Ps2Command(0xA7);
    const ENABLE_SECOND_PORT: Ps2Command = Ps2Command(0xA8);
    const DISABLE_FIRST_PORT: Ps2Command = Ps2Command(0xAD);
    const ENABLE_FIRST_PORT: Ps2Command = Ps2Command(0xAE);
    const WRITE_SECOND_PORT: Ps2Command = Ps2Command(0xD4);
}

my_bitflags! {
    pub struct Ps2Status: u8 {
        const OUTPUT_FULL   = 0b0000_0001;
        const INPUT_FULL    = 0b0000_0010;
        const SYSTEM_FLAG   = 0b0000_0100;
        const COMMAND       = 0b0000_1000;
        const TIMEOUT_ERROR = 0b0100_0000;
        const PARITY_ERROR  = 0b1000_0000;
    }
}

// PS2 scan code to HID usage table
static PS2_TO_HID: [u8; 256] = [
    0x00, 0x29, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A, 0x2B,
    0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C, 0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x28, 0xE0, 0x04, 0x16,
    0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F, 0x33, 0x34, 0x35, 0xE1, 0x31, 0x1D, 0x1B, 0x06, 0x19,
    0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0x55, 0xE2, 0x2C, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E,
    0x3F, 0x40, 0x41, 0x42, 0x43, 0x53, 0x47, 0x5F, 0x60, 0x61, 0x56, 0x5C, 0x5D, 0x5E, 0x57, 0x59,
    0x5A, 0x5B, 0x62, 0x63, 0, 0, 0, 0x44, 0x45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0x88, 0, 0, 0x87, 0, 0, 0, 0, 0, 0x8A, 0, 0x8B, 0, 0x89, 0, 0,
    // ----
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58, 0xE4,
    0, 0, 0x7F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x81, 0, 0x80, 0, 0, 0, 0, 0x54, 0, 0, 0xE6,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x4A, 0x52, 0x4B, 0, 0x50, 0, 0x4F, 0, 0x4D, 0x51,
    0x4E, 0x49, 0x4C, 0, 0, 0, 0, 0, 0, 0, 0xE3, 0xE7, 0x65, 0x66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
