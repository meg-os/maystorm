//! Human Interface Device Manager

use crate::prelude::*;
use core::num::NonZeroU8;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};
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
    pub const DISPLAY_BRIGHTNESS_INCREMENT: Self = Self::consumer(0x006F);
    pub const DISPLAY_BRIGHTNESS_DECREMENT: Self = Self::consumer(0x0070);
    pub const SELECTION: Self = Self::consumer(0x0080);
    pub const MEDIA_SELECTION: Self = Self::consumer(0x0087);
    pub const PLAY: Self = Self::consumer(0x00B0);
    pub const PAUSE: Self = Self::consumer(0x00B1);
    pub const RECORD: Self = Self::consumer(0x00B2);
    pub const FAST_FORWARD: Self = Self::consumer(0x00B3);
    pub const REWIND: Self = Self::consumer(0x00B4);
    pub const SCAN_NEXT_TRACK: Self = Self::consumer(0x00B5);
    pub const SCAN_PREVIOUS_TRACK: Self = Self::consumer(0x00B6);
    pub const STOP: Self = Self::consumer(0x00B7);
    pub const EJECT: Self = Self::consumer(0x00B8);
    pub const RANDOM_PLAY: Self = Self::consumer(0x00B9);
    pub const SELECT_DISC: Self = Self::consumer(0x00BA);
    pub const PLAY_PAUSE: Self = Self::consumer(0x00CD);
    pub const MUTE: Self = Self::consumer(0x00E2);
    pub const VOLUME_INCREMENT: Self = Self::consumer(0x00E9);
    pub const VOLUME_DECREMENT: Self = Self::consumer(0x00EA);
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
    pub const DEVICE_MODE: Self = Self::digitizers(0x0052);
    pub const CONTACT_COUNT: Self = Self::digitizers(0x0054);
    pub const CONTACT_COUNT_MAXIMUM: Self = Self::digitizers(0x0055);
    pub const SURFACE_SWITCH: Self = Self::digitizers(0x0057);
    pub const BUTTON_SWITCH: Self = Self::digitizers(0x0058);
    pub const PAD_TYPE: Self = Self::digitizers(0x0059);
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
    // pub const MUTE: Self = Self::led(0x0009);

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

impl core::fmt::Debug for HidUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:04x}_{:04x}", self.usage_page().0, self.usage())
    }
}

impl core::fmt::Display for HidUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

    pub const LANG_1: Self = Self(0x90);
    pub const LANG_2: Self = Self(0x91);
    pub const LANG_3: Self = Self(0x92);
    pub const LANG_4: Self = Self(0x93);
    pub const LANG_5: Self = Self(0x94);
    pub const LANG_6: Self = Self(0x95);
    pub const LANG_7: Self = Self(0x96);
    pub const LANG_8: Self = Self(0x97);
    pub const LANG_9: Self = Self(0x98);

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
    pub const fn full_qualified_usage(&self) -> HidUsage {
        HidUsage::new(UsagePage::KEYBOARD, self.0 as u16)
    }
}

impl From<Usage> for HidUsage {
    #[inline]
    fn from(v: Usage) -> Self {
        v.full_qualified_usage()
    }
}

/// Modifier keys as defined by the HID specification.
#[derive(Debug, Clone, Copy)]
pub struct Modifier(pub u8);

impl Modifier {
    pub const LEFT_CTRL: Self = Self(0b0000_0001);
    pub const LEFT_SHIFT: Self = Self(0b0000_0010);
    pub const LEFT_ALT: Self = Self(0b0000_0100);
    pub const LEFT_GUI: Self = Self(0b0000_1000);
    pub const RIGHT_CTRL: Self = Self(0b0001_0000);
    pub const RIGHT_SHIFT: Self = Self(0b0010_0000);
    pub const RIGHT_ALT: Self = Self(0b0100_0000);
    pub const RIGHT_GUI: Self = Self(0b1000_0000);

    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_bits_retain(val: u8) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
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
        Self(v as u8)
    }
}

impl Default for Modifier {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

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

/// Mouse buttons as defined by the HID specification.
#[derive(Debug, Clone, Copy)]
pub struct MouseButton(pub u8);

impl MouseButton {
    /// Primary Button
    pub const PRIMARY: Self = Self(0b0000_0001);
    /// Secondary Button
    pub const SECONDARY: Self = Self(0b0000_0010);
    /// Tertiary Button
    pub const TERTIARY: Self = Self(0b0000_0100);
    pub const BUTTON4: Self = Self(0b0000_1000);
    pub const BUTTON5: Self = Self(0b0001_0000);
    pub const BUTTON6: Self = Self(0b0010_0000);
    pub const BUTTON7: Self = Self(0b0100_0000);
    pub const BUTTON8: Self = Self(0b1000_0000);

    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_bits_retain(val: u8) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Default for MouseButton {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl BitOrAssign<Self> for MouseButton {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 = self.0 | rhs.0;
    }
}

impl BitOr<Self> for MouseButton {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitAndAssign<Self> for MouseButton {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 = self.0 & rhs.0;
    }
}

impl BitAnd<Self> for MouseButton {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitXorAssign<Self> for MouseButton {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 = self.0 ^ rhs.0;
    }
}

impl BitXor<Self> for MouseButton {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl From<u8> for MouseButton {
    #[inline]
    fn from(value: u8) -> Self {
        MouseButton(value)
    }
}

impl From<MouseButton> for u8 {
    #[inline]
    fn from(value: MouseButton) -> Self {
        value.0
    }
}

impl From<usize> for MouseButton {
    #[inline]
    fn from(value: usize) -> Self {
        MouseButton(value as u8)
    }
}

impl From<MouseButton> for usize {
    #[inline]
    fn from(value: MouseButton) -> Self {
        value.0 as usize
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidReportId(NonZeroU8);

impl HidReportId {
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        match NonZeroU8::new(v) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn as_u8(&self) -> u8 {
        self.0.get()
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

    fn next_u16(&mut self) -> Option<u16> {
        let b1 = self.next()?;
        let b2 = self.next()?;
        Some((b1 as u16) + (b2 as u16 * 0x100))
    }

    fn next_u32(&mut self) -> Option<u32> {
        let b1 = self.next()?;
        let b2 = self.next()?;
        let b3 = self.next()?;
        let b4 = self.next()?;
        Some((b1 as u32) + (b2 as u32 * 0x100) + (b3 as u32 * 0x100_00) + (b4 as u32 * 0x100_00_00))
    }

    pub fn read_param(&mut self, lead_byte: HidReportLeadByte) -> Option<HidReportValue> {
        match lead_byte.trail_bytes() {
            HidTrailBytes::Zero => Some(HidReportValue::Zero),
            HidTrailBytes::Byte => self.next().map(|v| v.into()),
            HidTrailBytes::Word => self.next_u16().map(|v| v.into()),
            HidTrailBytes::DWord => self.next_u32().map(|v| v.into()),
        }
    }
}

impl Iterator for HidReporteReader<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.len() {
            let result = unsafe { *self.data.get_unchecked(self.index) };
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}

#[repr(u8)]
#[non_exhaustive]
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
    pub const fn is_long_item(&self) -> bool {
        self.0 == 0xFE
    }

    #[inline]
    pub const fn trail_bytes(&self) -> HidTrailBytes {
        HidTrailBytes::from_u8(self.0)
    }

    #[inline]
    pub const fn report_type(&self) -> HidReportItemType {
        HidReportItemType::from_u8(self.0)
    }

    #[inline]
    pub fn item_tag(&self) -> Option<HidReportItemTag> {
        FromPrimitive::from_u8(self.0 & 0xFC)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidTrailBytes {
    Zero,
    Byte,
    Word,
    DWord,
}

impl HidTrailBytes {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val & 3 {
            0 => Self::Zero,
            1 => Self::Byte,
            2 => Self::Word,
            3 => Self::DWord,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub const fn trail_bytes(&self) -> usize {
        match *self {
            Self::Zero => 0,
            Self::Byte => 1,
            Self::Word => 2,
            Self::DWord => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidReportItemType {
    Main,
    Global,
    Local,
    Reserved,
}

impl HidReportItemType {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match (val >> 2) & 3 {
            0 => Self::Main,
            1 => Self::Global,
            2 => Self::Local,
            3 => Self::Reserved,
            _ => unreachable!(),
        }
    }
}

#[non_exhaustive]
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

#[derive(Debug, Clone, Copy)]
pub struct HidReportMainFlag(u32);

impl HidReportMainFlag {
    /// Data / Constant
    pub const CONSTANT: Self = Self(0x0001);
    /// Array / Variable
    pub const VARIABLE: Self = Self(0x0002);
    /// Absolute / Relative
    pub const RELATIVE: Self = Self(0x0004);
    /// No Wrap / Wrap
    pub const WRAP: Self = Self(0x0008);
    /// Linear / Non Linear
    pub const NON_LINEAR: Self = Self(0x0010);
    /// Preferred State / No Preferred
    pub const NO_PREFERRED: Self = Self(0x0020);
    /// No Null Position / Null State
    pub const NULL_STATE: Self = Self(0x0040);
    /// Non volatile / Volatile
    pub const VOLATILE: Self = Self(0x0080);
    /// Bit field / Buffered Bytes
    pub const BUFFERED_BYTES: Self = Self(0x0100);

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_bits_retain(val: u32) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

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

#[non_exhaustive]
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

#[derive(Clone, Copy)]
pub enum HidReportValue {
    Zero,
    X8(u8),
    X16(u16),
    X32(u32),
}

impl HidReportValue {
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.as_usize() as u32
    }

    #[inline]
    pub const fn as_i32(&self) -> i32 {
        self.as_isize() as i32
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        match *self {
            Self::Zero => 0,
            Self::X8(v) => v as usize,
            Self::X16(v) => v as usize,
            Self::X32(v) => v as usize,
        }
    }

    #[inline]
    pub const fn as_isize(&self) -> isize {
        match *self {
            Self::Zero => 0,
            Self::X8(v) => v as i8 as isize,
            Self::X16(v) => v as i16 as isize,
            Self::X32(v) => v as i32 as isize,
        }
    }
}

impl From<u8> for HidReportValue {
    #[inline]
    fn from(val: u8) -> Self {
        Self::X8(val)
    }
}

impl From<u16> for HidReportValue {
    #[inline]
    fn from(val: u16) -> Self {
        Self::X16(val)
    }
}

impl From<u32> for HidReportValue {
    #[inline]
    fn from(val: u32) -> Self {
        Self::X32(val)
    }
}

impl From<HidReportValue> for usize {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_usize()
    }
}

impl From<HidReportValue> for isize {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_isize()
    }
}

impl From<HidReportValue> for u8 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_usize() as u8
    }
}

impl From<HidReportValue> for u16 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_usize() as u16
    }
}

impl From<HidReportValue> for u32 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_u32()
    }
}

impl From<HidReportValue> for i8 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_isize() as i8
    }
}

impl From<HidReportValue> for i16 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_isize() as i16
    }
}

impl From<HidReportValue> for i32 {
    #[inline]
    fn from(val: HidReportValue) -> Self {
        val.as_i32()
    }
}

impl core::fmt::Debug for HidReportValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Zero => write!(f, "Zero"),
            Self::X8(arg0) => write!(f, "{:02x}", arg0),
            Self::X16(arg0) => write!(f, "{:04x}", arg0),
            Self::X32(arg0) => write!(f, "{:08x}", arg0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HidReportGlobalState {
    pub usage_page: UsagePage,
    pub logical_minimum: HidReportValue,
    pub logical_maximum: HidReportValue,
    pub physical_minimum: HidReportValue,
    pub physical_maximum: HidReportValue,
    pub unit_exponent: isize,
    pub unit: usize,
    pub report_size: usize,
    pub report_count: usize,
    pub report_id: Option<HidReportId>,
}

impl HidReportGlobalState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            usage_page: UsagePage(0),
            logical_minimum: HidReportValue::Zero,
            logical_maximum: HidReportValue::Zero,
            physical_minimum: HidReportValue::Zero,
            physical_maximum: HidReportValue::Zero,
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
        *self = Self::new();
    }
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceMode {
    Mouse = 0,
    SingleInputDevice,
    MultiInputDevice,
}
