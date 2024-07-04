//! USB Device Driver Modules

use super::{UsbClassDriverStarter, UsbInterfaceDriverStarter};
use crate::*;

pub mod usb_audio;
pub mod usb_hid;
pub mod usb_hub;
pub mod usb_msd;
pub mod xinput;

pub(super) fn install_drivers(
    specific_drivers: &mut Vec<Box<dyn UsbClassDriverStarter>>,
    class_drivers: &mut Vec<Box<dyn UsbClassDriverStarter>>,
    interface_drivers: &mut Vec<Box<dyn UsbInterfaceDriverStarter>>,
) {
    // ## Device Specific Drivers
    let _ = specific_drivers;

    // ## Device Class Drivers

    // 09_xx_xx HUB
    class_drivers.push(usb_hub::UsbHubStarter::new());

    // ## Interface Class Drivers

    // 01_xx_xx Audio
    interface_drivers.push(usb_audio::UsbAudioStarter::new());

    // 03_xx_xx HID
    interface_drivers.push(usb_hid::UsbHidStarter::new());

    // 08_06_50 Mass Storage Device (Bulk Only Transfer)
    interface_drivers.push(usb_msd::UsbMsdStarter::new());

    // FF_5D_01 Xinput
    interface_drivers.push(xinput::XInputStarter::new());
}
