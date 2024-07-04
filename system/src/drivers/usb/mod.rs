//! Universal Serial Bus

mod usb;
mod usb_mgr;
pub use usb::*;
pub use usb_mgr::*;
pub mod drivers;
pub mod xhci;

#[derive(Debug, Clone, Copy)]
pub enum UsbError {
    General,
    Unsupported,
    ControllerError(usize),
    InvalidParameter,
    InvalidDescriptor,
    UnexpectedToken,
    Aborted,
    Stall,
    ShortPacket,
    UsbTransactionError,
    OutOfMemory,
}
