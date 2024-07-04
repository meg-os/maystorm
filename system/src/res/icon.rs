//! Icon Resource Manager

use crate::io::image::{DecodeError, ImageLoader};
use crate::*;
use megstd::drawing::*;

pub struct IconManager {}

impl IconManager {
    pub fn bitmap(icon: r::Icons) -> Result<OwnedBitmap32, DecodeError> {
        match icon {
            r::Icons::Pointer => {
                ImageLoader::load(include_bytes!("../../../assets/images/pointer.png"))
            }
            r::Icons::Apps => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_apps_black_24dp.png"
            )),
            r::Icons::Cancel => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_cancel_black_24dp.png"
            )),
            r::Icons::Check => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_check_black_24dp.png"
            )),
            r::Icons::ChevronLeft => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_chevron_left_black_24dp.png"
            )),
            r::Icons::ChevronRight => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_chevron_right_black_24dp.png"
            )),
            r::Icons::Close => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_close_black_24dp.png"
            )),
            r::Icons::Delete => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_delete_black_24dp.png"
            )),
            r::Icons::Error => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_error_black_24dp.png"
            )),
            r::Icons::Menu => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_menu_black_24dp.png"
            )),
            r::Icons::Info => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_info_outline_black_24dp.png"
            )),
            r::Icons::Usb => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_usb_black_24dp.png"
            )),
            r::Icons::Warning => ImageLoader::load(include_bytes!(
                "../../../assets/material-design-icons/ic_warning_black_24dp.png"
            )),
        }
    }

    pub fn mask(icon: r::Icons) -> Option<OperationalBitmap> {
        Self::bitmap(icon)
            .ok()
            .map(|v| v.as_ref().to_operational(|c| c.opacity().as_u8()))
    }
}
