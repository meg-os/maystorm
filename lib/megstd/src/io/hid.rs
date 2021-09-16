//! Human Interface Device Manager

use alloc::vec::Vec;
use bitflags::*;
use core::num::NonZeroU8;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsagePage(pub u16);

impl UsagePage {
    pub const GENERIC_DESKTOP: Self = Self(0x0001);
    pub const KEYBOARD: Self = Self(0x0007);
    pub const LED: Self = Self(0x0008);
    pub const BUTTON: Self = Self(0x0009);
    pub const CONSUMER: Self = Self(0x000C);
    pub const DIGITIZERS: Self = Self(0x000D);
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidUsage(pub u32);

impl HidUsage {
    pub const NONE: Self = Self(0);

    pub const POINTER: Self = Self::generic(0x0001);
    pub const MOUSE: Self = Self::generic(0x0002);
    pub const JOYSTICK: Self = Self::generic(0x0004);
    pub const GAMEPAD: Self = Self::generic(0x0005);
    pub const KEYBOARD: Self = Self::generic(0x0006);
    pub const KEYPAD: Self = Self::generic(0x0007);
    pub const MULTI_AXIS_CONTROLLER: Self = Self::generic(0x0008);
    pub const TABLET_SYSTEM_CONTROLS: Self = Self::generic(0x0009);
    pub const WATER_COOLING_SYSTEM: Self = Self::generic(0x000A);
    pub const COMPUTER_CHASSIS_DEVICE: Self = Self::generic(0x000B);
    pub const WIRELESS_RADIO_CONTROLS: Self = Self::generic(0x000C);
    pub const PORTABLE_DEVICE: Self = Self::generic(0x000D);
    pub const SYSTEM_MULTI_AXIS_CONTROLLER: Self = Self::generic(0x000E);
    pub const SPATIAL_CONTROLLER: Self = Self::generic(0x000F);
    pub const ASSISTIVE_CONTROL: Self = Self::generic(0x0010);
    pub const DEVICE_DOCK: Self = Self::generic(0x0011);
    pub const DOCKABLE_DEVICE: Self = Self::generic(0x0012);
    pub const X: Self = Self::generic(0x0030);
    pub const Y: Self = Self::generic(0x0031);
    pub const Z: Self = Self::generic(0x0032);
    pub const RX: Self = Self::generic(0x0033);
    pub const RY: Self = Self::generic(0x0034);
    pub const RZ: Self = Self::generic(0x0035);
    pub const SLIDER: Self = Self::generic(0x0036);
    pub const DIAL: Self = Self::generic(0x0037);
    pub const WHEEL: Self = Self::generic(0x0038);
    pub const HAT_SWITCH: Self = Self::generic(0x0039);
    pub const COUNTED_BUFFER: Self = Self::generic(0x003A);
    pub const BYTE_COUNT: Self = Self::generic(0x003B);
    pub const MOTION_WAKEUP: Self = Self::generic(0x003C);
    pub const START: Self = Self::generic(0x003D);
    pub const SELECT: Self = Self::generic(0x003E);
    pub const VX: Self = Self::generic(0x0040);
    pub const VY: Self = Self::generic(0x0041);
    pub const VZ: Self = Self::generic(0x0042);
    pub const VBRX: Self = Self::generic(0x0043);
    pub const VBRY: Self = Self::generic(0x0044);
    pub const VBRZ: Self = Self::generic(0x0045);
    pub const VNO: Self = Self::generic(0x0046);
    pub const FEATURE_NOTIFICATION: Self = Self::generic(0x0047);
    pub const RESOLUTION_MULTIPLIER: Self = Self::generic(0x0048);
    pub const QX: Self = Self::generic(0x0049);
    pub const QY: Self = Self::generic(0x004A);
    pub const QZ: Self = Self::generic(0x004B);
    pub const QW: Self = Self::generic(0x004C);
    pub const SYSTEM_CONTROL: Self = Self::generic(0x0080);
    pub const SYSTEM_POWER_DOWN: Self = Self::generic(0x0081);
    pub const SYSTEM_SLEEP: Self = Self::generic(0x0082);
    pub const SYSTEM_WAKEUP: Self = Self::generic(0x0083);
    pub const SYSTEM_CONTEXT_MENU: Self = Self::generic(0x0084);
    pub const SYSTEM_MAIN_MENU: Self = Self::generic(0x0085);
    pub const SYSTEM_APP_MENU: Self = Self::generic(0x0086);
    pub const SYSTEM_MENU_HELP: Self = Self::generic(0x0087);
    pub const SYSTEM_MENU_EXIT: Self = Self::generic(0x0088);
    pub const SYSTEM_MENU_SELECT: Self = Self::generic(0x0089);
    pub const SYSTEM_MENU_RIGHT: Self = Self::generic(0x008A);
    pub const SYSTEM_MENU_LEFT: Self = Self::generic(0x008B);
    pub const SYSTEM_MENU_UP: Self = Self::generic(0x008C);
    pub const SYSTEM_MENU_DOWN: Self = Self::generic(0x008D);
    pub const SYSTEM_COLD_RESTART: Self = Self::generic(0x008E);
    pub const SYSTEM_WARM_RESTART: Self = Self::generic(0x008F);
    pub const DPAD_UP: Self = Self::generic(0x0090);
    pub const DPAD_DOWN: Self = Self::generic(0x0091);
    pub const DPAD_RIGHT: Self = Self::generic(0x0092);
    pub const DPAD_LEFT: Self = Self::generic(0x0093);
    pub const INDEX_TRIGGER: Self = Self::generic(0x0094);
    pub const PALM_TRIGGER: Self = Self::generic(0x0095);
    pub const THUMB_STICK: Self = Self::generic(0x0096);
    pub const SYSTEM_FUNCTION_SHIFT: Self = Self::generic(0x0097);
    pub const SYSTEM_FUNCTION_SHIFT_LOCK: Self = Self::generic(0x0098);
    pub const SYSTEM_FUNCTION_SHIFT_LOCK_INDICATOR: Self = Self::generic(0x0099);
    pub const SYSTEM_DISMISS_NOTIFICATION: Self = Self::generic(0x009A);
    pub const SYSTEM_DO_NOT_DISTURB: Self = Self::generic(0x009B);
    pub const SYSTEM_DOCK: Self = Self::generic(0x00A0);
    pub const SYSTEM_UNDOCK: Self = Self::generic(0x00A1);
    pub const SYSTEM_SETUP: Self = Self::generic(0x00A2);
    pub const SYSTEM_BREAK: Self = Self::generic(0x00A3);
    pub const SYSTEM_DEBUGGER_BREAK: Self = Self::generic(0x00A4);
    pub const APPLICATION_BREAK: Self = Self::generic(0x00A5);
    pub const APPLICATION_DEBUGGER_BREAK: Self = Self::generic(0x00A6);
    pub const SYSTEM_SPEAKER_MUTE: Self = Self::generic(0x00A7);
    pub const SYSTEM_HIBERNATE: Self = Self::generic(0x00A8);
    pub const SYSTEM_DISPLAY_INVERT: Self = Self::generic(0x00B0);
    pub const SYSTEM_DISPLAY_INTERNAL: Self = Self::generic(0x00B1);
    pub const SYSTEM_DISPLAY_EXTERNAL: Self = Self::generic(0x00B2);
    pub const SYSTEM_DISPLAY_BOTH: Self = Self::generic(0x00B3);
    pub const SYSTEM_DISPLAY_DUAL: Self = Self::generic(0x00B4);
    pub const SYSTEM_DISPLAY_TOGGLE_INT_EXT_MODE: Self = Self::generic(0x00B5);
    pub const SYSTEM_DISPLAY_SWAP_PRIMARY_SECONDARY: Self = Self::generic(0x00B6);
    pub const SYSTEM_DISPLAY_TOGGLE_LCD_AUTOSCALE: Self = Self::generic(0x00B7);
    pub const SENSOR_ZONE: Self = Self::generic(0x00C0);
    pub const RPM: Self = Self::generic(0x00C1);
    pub const COOLANT_LEVEL: Self = Self::generic(0x00C2);
    pub const COOLANT_CRITICAL_LEVEL: Self = Self::generic(0x00C2);
    pub const COOLANT_PUMP: Self = Self::generic(0x00C4);
    pub const CHASSIS_ENCLOSURE: Self = Self::generic(0x00C5);
    pub const WIRELESS_RADIO_BUTTON: Self = Self::generic(0x00C6);
    pub const WIRELESS_RADIO_LED: Self = Self::generic(0x00C7);
    pub const WIRELESS_RADIO_SLIDER_SWITCH: Self = Self::generic(0x00C8);
    pub const SYSTEM_DISPLAY_ROTATION_LOCK_BUTTON: Self = Self::generic(0x00C9);
    pub const SYSTEM_DISPLAY_ROTATION_SLIDER_SWITCH: Self = Self::generic(0x00CA);
    pub const CONTROL_ENABLE: Self = Self::generic(0x00CB);
    pub const DOCKABLE_DEVICE_UNIQUE_ID: Self = Self::generic(0x00D0);
    pub const DOCKABLE_DEVICE_VENDOR_ID: Self = Self::generic(0x00D1);
    pub const DOCKABLE_DEVICE_PRIMARY_USAGE_PAGE: Self = Self::generic(0x00D2);
    pub const DOCKABLE_DEVICE_PRIMARY_USAGE_ID: Self = Self::generic(0x00D3);
    pub const DOCKABLE_DEVICE_DOCKING_STATE: Self = Self::generic(0x00D4);
    pub const DOCKABLE_DEVICE_DISPLAY_OCCULUSION: Self = Self::generic(0x00D5);
    pub const DOCKABLE_DEVICE_OBJECT_TYPE: Self = Self::generic(0x00D6);

    pub const BUTTON_1: Self = Self::button(1);
    pub const BUTTON_2: Self = Self::button(2);
    pub const BUTTON_3: Self = Self::button(3);
    pub const BUTTON_4: Self = Self::button(4);
    pub const BUTTON_5: Self = Self::button(5);
    pub const BUTTON_6: Self = Self::button(6);
    pub const BUTTON_7: Self = Self::button(7);
    pub const BUTTON_8: Self = Self::button(8);

    pub const CONSUMER_CONTROL: Self = Self::consumer(0x0001);
    pub const NUMERIC_KEY_PAD: Self = Self::consumer(0x0002);
    pub const PROGRAMMABLE_BUTTONS: Self = Self::consumer(0x0003);
    pub const MICROPHONE: Self = Self::consumer(0x0004);
    pub const HEADPHONE: Self = Self::consumer(0x0005);
    pub const GRAPHIC_EQUALIZER: Self = Self::consumer(0x0006);
    pub const FUNCTION_BUTTONS: Self = Self::consumer(0x0036);
    pub const SELECTION: Self = Self::consumer(0x0080);
    pub const MEDIA_SELECTION: Self = Self::consumer(0x0087);
    pub const SELECT_DISC: Self = Self::consumer(0x00BA);
    pub const PLAYBACK_SPEED: Self = Self::consumer(0x00F1);
    pub const SPEAKER_SYSTEM: Self = Self::consumer(0x0160);
    pub const CHANNEL_LEFT: Self = Self::consumer(0x0161);
    pub const CHANNEL_RIGHT: Self = Self::consumer(0x0162);
    pub const CHANNEL_CENTER: Self = Self::consumer(0x0163);
    pub const CHANNEL_FRONT: Self = Self::consumer(0x0164);
    pub const CHANNEL_CENTER_FRONT: Self = Self::consumer(0x0165);
    pub const CHANNEL_SIDE: Self = Self::consumer(0x0166);
    pub const CHANNEL_SURROUND: Self = Self::consumer(0x0167);
    pub const CHANNEL_LOW_FREQUENCY_ENHANCEMENT: Self = Self::consumer(0x0168);
    pub const CHANNEL_TOP: Self = Self::consumer(0x0169);
    pub const CHANNEL_UNKNOWN: Self = Self::consumer(0x016A);
    pub const APPLICATION_LAUNCH_BUTTONS: Self = Self::consumer(0x0180);
    pub const GENERIC_GUI_APPLICATION_CONTROLS: Self = Self::consumer(0x0200);

    pub const DIGITIZER: Self = Self::digitizers(0x0001);
    pub const PEN: Self = Self::digitizers(0x0002);
    pub const LIGHT_PEN: Self = Self::digitizers(0x0003);
    pub const TOUCH_SCREEN: Self = Self::digitizers(0x0004);
    pub const TOUCH_PAD: Self = Self::digitizers(0x0005);
    pub const WHITEBOARD: Self = Self::digitizers(0x0006);
    pub const COORDINATE_MEASURING_MACHINE: Self = Self::digitizers(0x0007);
    pub const _3D_DIGITIZER: Self = Self::digitizers(0x0008);
    pub const STEREO_PLOTTER: Self = Self::digitizers(0x0009);
    pub const ARTICULATED_ARM: Self = Self::digitizers(0x000A);
    pub const ARMATURE: Self = Self::digitizers(0x000B);
    pub const MULTIPLE_POINT_DIGITIZER: Self = Self::digitizers(0x000C);
    pub const FREE_SPACE_WAND: Self = Self::digitizers(0x000D);
    pub const DEVICE_CONFIGURATION: Self = Self::digitizers(0x000E);
    pub const CAPACTIVE_HEAT_MAP_DIGITIZER: Self = Self::digitizers(0x000F);
    pub const STYLUS: Self = Self::digitizers(0x0020);
    pub const PUCK: Self = Self::digitizers(0x0021);
    pub const FINGER: Self = Self::digitizers(0x0022);
    pub const DEVICE_SETTINGS: Self = Self::digitizers(0x0023);
    pub const CHARACTER_GESTURE: Self = Self::digitizers(0x0024);
    pub const TABLET_FUNCTION_KEYS: Self = Self::digitizers(0x0039);
    pub const PROGRAM_CHANGE_KEYS: Self = Self::digitizers(0x003A);
    pub const GESTURE_CHARACTER_ENCODING: Self = Self::digitizers(0x0064);
    pub const PREFERRED_LINE_STYLE: Self = Self::digitizers(0x0070);
    pub const DIGITIZER_DIAGNOSTIC: Self = Self::digitizers(0x0080);
    pub const DIGITIZER_ERROR: Self = Self::digitizers(0x0081);
    pub const TRANSDUCER_SOFTWARE_INFO: Self = Self::digitizers(0x0090);
    pub const DEVICE_SUPPORTED_PROTOCOLS: Self = Self::digitizers(0x0093);
    pub const TRANSDUCER_SUPPORTED_PROTOCOLS: Self = Self::digitizers(0x0094);
    pub const SUPPORTED_REPORT_RATES: Self = Self::digitizers(0x00A0);

    pub const NUM_LOCK: Self = Self::led(0x0001);
    pub const CAPS_LOCK: Self = Self::led(0x0002);
    pub const SCROLL_LOCK: Self = Self::led(0x0003);
    pub const COMPOSE: Self = Self::led(0x0004);
    pub const KANA: Self = Self::led(0x0005);
    pub const POWER: Self = Self::led(0x0006);
    pub const SHIFT: Self = Self::led(0x0007);
    pub const DO_NOT_DISTURB: Self = Self::led(0x0008);
    pub const MUTE: Self = Self::led(0x0009);

    #[inline]
    pub const fn new(page: UsagePage, usage: u16) -> Self {
        Self(usage as u32 + (page.0 as u32) * 0x10000)
    }

    #[inline]
    pub const fn usage_page(&self) -> UsagePage {
        UsagePage((self.0 >> 16) as u16)
    }

    #[inline]
    pub const fn usage(&self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    #[inline]
    pub const fn generic(usage: u16) -> Self {
        Self::new(UsagePage::GENERIC_DESKTOP, usage)
    }

    #[inline]
    pub const fn button(usage: u16) -> Self {
        Self::new(UsagePage::BUTTON, usage)
    }

    #[inline]
    pub const fn consumer(usage: u16) -> Self {
        Self::new(UsagePage::CONSUMER, usage)
    }

    #[inline]
    pub const fn digitizers(usage: u16) -> Self {
        Self::new(UsagePage::DIGITIZERS, usage)
    }

    #[inline]
    pub const fn led(usage: u16) -> Self {
        Self::new(UsagePage::LED, usage)
    }
}

impl core::fmt::Display for HidUsage {
    fn fmt(&self, f: &mut _core::fmt::Formatter<'_>) -> _core::fmt::Result {
        write!(f, "{:04x}_{:04x}", self.usage_page().0, self.usage())
    }
}

/// Usage ID of the keyboard as defined in the HID specification.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Usage(pub u8);

impl Usage {
    pub const NONE: Self = Self(0);
    pub const ERR_ROLL_OVER: Self = Self(1);
    pub const ERR_POST_FAIL: Self = Self(2);
    pub const ERR_UNDEFINED: Self = Self(3);

    pub const KEY_A: Self = Self(0x04);
    pub const KEY_B: Self = Self(0x05);
    pub const KEY_C: Self = Self(0x06);
    pub const KEY_D: Self = Self(0x07);
    pub const KEY_E: Self = Self(0x08);
    pub const KEY_F: Self = Self(0x09);
    pub const KEY_G: Self = Self(0x0A);
    pub const KEY_H: Self = Self(0x0B);
    pub const KEY_I: Self = Self(0x0C);
    pub const KEY_J: Self = Self(0x0D);
    pub const KEY_K: Self = Self(0x0E);
    pub const KEY_L: Self = Self(0x0F);
    pub const KEY_M: Self = Self(0x10);
    pub const KEY_N: Self = Self(0x11);
    pub const KEY_O: Self = Self(0x12);
    pub const KEY_P: Self = Self(0x13);
    pub const KEY_Q: Self = Self(0x14);
    pub const KEY_R: Self = Self(0x15);
    pub const KEY_S: Self = Self(0x16);
    pub const KEY_T: Self = Self(0x17);
    pub const KEY_U: Self = Self(0x18);
    pub const KEY_V: Self = Self(0x19);
    pub const KEY_W: Self = Self(0x1A);
    pub const KEY_X: Self = Self(0x1B);
    pub const KEY_Y: Self = Self(0x1C);
    pub const KEY_Z: Self = Self(0x1D);
    pub const KEY_1: Self = Self(0x1E);
    pub const KEY_2: Self = Self(0x1F);
    pub const KEY_3: Self = Self(0x20);
    pub const KEY_4: Self = Self(0x21);
    pub const KEY_5: Self = Self(0x22);
    pub const KEY_6: Self = Self(0x23);
    pub const KEY_7: Self = Self(0x24);
    pub const KEY_8: Self = Self(0x25);
    pub const KEY_9: Self = Self(0x26);
    pub const KEY_0: Self = Self(0x27);
    pub const KEY_ENTER: Self = Self(0x28);
    pub const KEY_ESCAPE: Self = Self(0x29);
    pub const KEY_BASKSPACE: Self = Self(0x2A);
    pub const KEY_TAB: Self = Self(0x2B);
    pub const KEY_SPACE: Self = Self(0x2C);

    pub const KEY_F1: Self = Self(0x3A);
    pub const KEY_F2: Self = Self(0x3B);
    pub const KEY_F3: Self = Self(0x3C);
    pub const KEY_F4: Self = Self(0x3D);
    pub const KEY_F5: Self = Self(0x3E);
    pub const KEY_F6: Self = Self(0x3F);
    pub const KEY_F7: Self = Self(0x40);
    pub const KEY_F8: Self = Self(0x41);
    pub const KEY_F9: Self = Self(0x42);
    pub const KEY_F10: Self = Self(0x43);
    pub const KEY_F11: Self = Self(0x44);
    pub const KEY_F12: Self = Self(0x45);
    pub const DELETE: Self = Self(0x4C);
    pub const KEY_RIGHT_ARROW: Self = Self(0x4F);
    pub const KEY_LEFT_ARROW: Self = Self(0x50);
    pub const KEY_DOWN_ARROW: Self = Self(0x51);
    pub const KEY_UP_ARROW: Self = Self(0x52);
    pub const KEY_NUM_LOCK: Self = Self(0x53);

    pub const NUMPAD_1: Self = Self(0x59);
    pub const NUMPAD_2: Self = Self(0x5A);
    pub const NUMPAD_3: Self = Self(0x5B);
    pub const NUMPAD_4: Self = Self(0x5C);
    pub const NUMPAD_5: Self = Self(0x5D);
    pub const NUMPAD_6: Self = Self(0x5E);
    pub const NUMPAD_7: Self = Self(0x5F);
    pub const NUMPAD_8: Self = Self(0x60);
    pub const NUMPAD_9: Self = Self(0x61);
    pub const NUMPAD_0: Self = Self(0x62);

    pub const INTERNATIONAL_1: Self = Self(0x87);
    pub const INTERNATIONAL_2: Self = Self(0x88);
    pub const INTERNATIONAL_3: Self = Self(0x89);
    pub const INTERNATIONAL_4: Self = Self(0x8A);
    pub const INTERNATIONAL_5: Self = Self(0x8B);
    pub const INTERNATIONAL_6: Self = Self(0x8C);
    pub const INTERNATIONAL_7: Self = Self(0x8D);
    pub const INTERNATIONAL_8: Self = Self(0x8E);
    pub const INTERNATIONAL_9: Self = Self(0x8F);

    pub const KEY_LEFT_CONTROL: Self = Self(0xE0);
    pub const KEY_LEFT_SHIFT: Self = Self(0xE1);
    pub const KEY_LEFT_ALT: Self = Self(0xE2);
    pub const KEY_LEFT_GUI: Self = Self(0xE3);
    pub const KEY_RIGHT_CONTROL: Self = Self(0xE4);
    pub const KEY_RIGHT_SHIFT: Self = Self(0xE5);
    pub const KEY_RIGHT_ALT: Self = Self(0xE6);
    pub const KEY_RIGHT_GUI: Self = Self(0xE7);

    pub const ALPHABET_MIN: Self = Self(0x04);
    pub const ALPHABET_MAX: Self = Self(0x1D);
    pub const NUMBER_MIN: Self = Self(0x1E);
    pub const NUMBER_MAX: Self = Self(0x27);
    pub const NON_ALPHABET_MIN: Self = Self(0x28);
    pub const NON_ALPHABET_MAX: Self = Self(0x38);
    pub const NUMPAD_MIN: Self = Self(0x54);
    pub const NUMPAD_MAX: Self = Self(0x63);
    pub const MOD_MIN: Self = Self(0xE0);
    pub const MOD_MAX: Self = Self(0xE7);

    #[inline]
    pub const fn full_usage(&self) -> HidUsage {
        HidUsage::new(UsagePage::KEYBOARD, self.0 as u16)
    }
}

bitflags! {
    /// Modifier keys as defined by the HID specification.
    pub struct Modifier: u8 {
        const LEFT_CTRL     = 0b0000_0001;
        const LEFT_SHIFT    = 0b0000_0010;
        const LEFT_ALT      = 0b0000_0100;
        const LEFT_GUI      = 0b0000_1000;
        const RIGHT_CTRL    = 0b0001_0000;
        const RIGHT_SHIFT   = 0b0010_0000;
        const RIGHT_ALT     = 0b0100_0000;
        const RIGHT_GUI     = 0b1000_0000;
    }
}

impl Modifier {
    #[inline]
    pub const fn has_shift(self) -> bool {
        self.contains(Self::LEFT_SHIFT) | self.contains(Self::RIGHT_SHIFT)
    }

    #[inline]
    pub const fn has_ctrl(self) -> bool {
        self.contains(Self::LEFT_CTRL) | self.contains(Self::RIGHT_CTRL)
    }

    #[inline]
    pub const fn has_alt(self) -> bool {
        self.contains(Self::LEFT_ALT) | self.contains(Self::RIGHT_ALT)
    }
}

impl From<Modifier> for usize {
    #[inline]
    fn from(v: Modifier) -> Self {
        v.bits() as Self
    }
}

impl From<usize> for Modifier {
    #[inline]
    fn from(v: usize) -> Self {
        Self::from_bits_truncate(v as u8)
    }
}

impl Default for Modifier {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MouseReport<T> {
    pub buttons: MouseButton,
    pub x: T,
    pub y: T,
    pub wheel: T,
}

impl<T: Default> Default for MouseReport<T> {
    fn default() -> Self {
        Self {
            buttons: Default::default(),
            x: Default::default(),
            y: Default::default(),
            wheel: Default::default(),
        }
    }
}

impl<T: Into<isize> + Copy> MouseReport<T> {
    /// Returns the mouse report in a canonical format.
    #[inline]
    pub fn normalize(self) -> MouseReport<isize> {
        MouseReport {
            buttons: self.buttons,
            x: self.x.into(),
            y: self.y.into(),
            wheel: self.wheel.into(),
        }
    }
}

bitflags! {
    /// Mouse buttons as defined by the HID specification.
    pub struct MouseButton: u8 {
        /// Primary/Trigger Button
        const PRIMARY   = 0b0000_0001;
        /// Secondary Button
        const SECONDARY = 0b0000_0010;
        /// Tertiary Button
        const TERTIARY  = 0b0000_0100;
        const BUTTON4   = 0b0000_1000;
        const BUTTON5   = 0b0001_0000;
        const BUTTON6   = 0b0010_0000;
        const BUTTON7   = 0b0100_0000;
        const BUTTON8   = 0b1000_0000;
    }
}

impl Default for MouseButton {
    fn default() -> Self {
        Self::empty()
    }
}

pub struct HidReporteReader<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> HidReporteReader<'a> {
    #[inline]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0 }
    }
}

impl HidReporteReader<'_> {
    #[inline]
    pub const fn position(&self) -> usize {
        self.index
    }

    pub fn next_u16(&mut self) -> Option<u16> {
        if self.index + 1 < self.data.len() {
            let result = unsafe {
                (*self.data.get_unchecked(self.index) as u16)
                    + (*self.data.get_unchecked(self.index + 1) as u16 * 256)
            };
            self.index += 2;
            Some(result)
        } else {
            None
        }
    }

    pub fn next_u32(&mut self) -> Option<u32> {
        if self.index + 3 < self.data.len() {
            let result = unsafe {
                (*self.data.get_unchecked(self.index) as u32)
                    + (*self.data.get_unchecked(self.index + 1) as u32 * 0x100)
                    + (*self.data.get_unchecked(self.index + 2) as u32 * 0x100_00)
                    + (*self.data.get_unchecked(self.index + 3) as u32 * 0x100_00_00)
            };
            self.index += 4;
            Some(result)
        } else {
            None
        }
    }

    pub fn read_param(
        &mut self,
        lead_byte: HidReportLeadByte,
    ) -> Option<HidReportAmbiguousSignedValue> {
        match lead_byte.data_size() {
            0 => Some(HidReportAmbiguousSignedValue::Zero),
            1 => self.next().map(|v| v.into()),
            2 => self.next_u16().map(|v| v.into()),
            _ => self.next_u32().map(|v| v.into()),
        }
    }
}

impl Iterator for HidReporteReader<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.len() {
            let result = self.data.get(self.index).map(|v| *v);
            self.index += 1;
            result
        } else {
            None
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidReportType {
    Input = 1,
    Output,
    Feature,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidReportLeadByte(pub u8);

impl HidReportLeadByte {
    #[inline]
    pub const fn data_size(&self) -> usize {
        match self.0 & 3 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        }
    }

    #[inline]
    pub const fn report_type(&self) -> HidReportItemType {
        HidReportItemType::from_u8((self.0 >> 2) & 3)
    }

    #[inline]
    pub fn item_tag(&self) -> Option<HidReportItemTag> {
        FromPrimitive::from_u8(self.0 & 0xFC)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidReportItemType {
    Main = 0,
    Global,
    Local,
    Reserved,
}

impl HidReportItemType {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Main,
            1 => Self::Global,
            2 => Self::Local,
            _ => Self::Reserved,
        }
    }
}

impl From<u8> for HidReportItemType {
    #[inline]
    fn from(v: u8) -> Self {
        Self::from_u8(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum HidReportItemTag {
    // Main
    Input = 0x80,
    Output = 0x90,
    Feature = 0xB0,
    Collection = 0xA0,
    EndCollection = 0xC0,
    // Global
    UsagePage = 0x04,
    LogicalMinimum = 0x14,
    LogicalMaximum = 0x24,
    PhysicalMinimum = 0x34,
    PhysicalMaximum = 0x44,
    UnitExponent = 0x54,
    Unit = 0x64,
    ReportSize = 0x74,
    ReportId = 0x84,
    ReportCount = 0x94,
    Push = 0xA4,
    Pop = 0xB4,
    // Local
    Usage = 0x08,
    UsageMinimum = 0x18,
    UsageMaximum = 0x28,
    DesignatorIndex = 0x38,
    DesignatorMinimum = 0x48,
    DesignatorMaximum = 0x58,
    StringIndex = 0x78,
    StringMinimum = 0x88,
    StringMaximum = 0x98,
    Delimiter = 0xA8,
}

bitflags! {
    pub struct HidReportMainFlag: usize {
        /// Data / Constant
        const CONSTANT          = 0x0001;
        /// Array / Variable
        const VARIABLE          = 0x0002;
        /// Absolute / Relative
        const RELATIVE          = 0x0004;
        /// No Wrap / Wrap
        const WRAP              = 0x0008;
        /// Linear / Non Linear
        const NON_LINEAR        = 0x0010;
        /// Preferred State / No Preferred
        const NO_PREFERRED      = 0x0020;
        /// No Null Position / Null State
        const NULL_STATE        = 0x0040;
        /// Non volatile / Volatile
        const VOLATILE          = 0x0080;
        /// Bit field / Buffered Bytes
        const BUFFERED_BYTES    = 0x0100;
    }
}

impl HidReportMainFlag {
    #[inline]
    pub fn is_const(&self) -> bool {
        self.contains(Self::CONSTANT)
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        !self.is_variable()
    }

    #[inline]
    pub fn is_variable(&self) -> bool {
        self.contains(Self::VARIABLE)
    }

    #[inline]
    pub fn is_relative(&self) -> bool {
        self.contains(Self::RELATIVE)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum HidReportCollectionType {
    Physical = 0,
    Application,
    Logical,
    Report,
    NamedArray,
    UsageSwitch,
    UsageModifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidReportAmbiguousSignedValue {
    Zero,
    U8(u8),
    U16(u16),
    U32(u32),
}

impl HidReportAmbiguousSignedValue {
    #[inline]
    pub const fn usize(&self) -> usize {
        match *self {
            Self::Zero => 0,
            Self::U8(v) => v as usize,
            Self::U16(v) => v as usize,
            Self::U32(v) => v as usize,
        }
    }

    #[inline]
    pub const fn isize(&self) -> isize {
        match *self {
            Self::Zero => 0,
            Self::U8(v) => v as i8 as isize,
            Self::U16(v) => v as i16 as isize,
            Self::U32(v) => v as i32 as isize,
        }
    }
}

impl From<u8> for HidReportAmbiguousSignedValue {
    #[inline]
    fn from(val: u8) -> Self {
        Self::U8(val)
    }
}

impl From<u16> for HidReportAmbiguousSignedValue {
    #[inline]
    fn from(val: u16) -> Self {
        Self::U16(val)
    }
}

impl From<u32> for HidReportAmbiguousSignedValue {
    #[inline]
    fn from(val: u32) -> Self {
        Self::U32(val)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidReportGlobalState {
    pub usage_page: UsagePage,
    pub logical_minimum: HidReportAmbiguousSignedValue,
    pub logical_maximum: HidReportAmbiguousSignedValue,
    pub physical_minimum: HidReportAmbiguousSignedValue,
    pub physical_maximum: HidReportAmbiguousSignedValue,
    pub unit_exponent: isize,
    pub unit: usize,
    pub report_size: usize,
    pub report_count: usize,
    pub report_id: Option<NonZeroU8>,
}

impl HidReportGlobalState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            usage_page: UsagePage(0),
            logical_minimum: HidReportAmbiguousSignedValue::Zero,
            logical_maximum: HidReportAmbiguousSignedValue::Zero,
            physical_minimum: HidReportAmbiguousSignedValue::Zero,
            physical_maximum: HidReportAmbiguousSignedValue::Zero,
            unit_exponent: 0,
            unit: 0,
            report_size: 0,
            report_count: 0,
            report_id: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidReportLocalState {
    pub usage: Vec<u32>,
    pub usage_minimum: u32,
    pub usage_maximum: u32,
    pub delimiter: usize,
}

impl HidReportLocalState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            usage: Vec::new(),
            usage_minimum: 0,
            usage_maximum: 0,
            delimiter: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.usage = Vec::new();
        self.usage_minimum = 0;
        self.usage_maximum = 0;
        self.delimiter = 0;
    }
}
