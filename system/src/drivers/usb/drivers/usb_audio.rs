//! USB Audio Class Driver (01_xx_xx)

use super::super::*;
use crate::task::Task;
use crate::*;
use core::pin::Pin;
use futures_util::Future;

pub struct UsbAudioStarter;

impl UsbAudioStarter {
    #[inline]
    pub fn new() -> Box<dyn UsbInterfaceDriverStarter> {
        Box::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbAudioStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>> {
        if class.base_class() == UsbBaseClass::AUDIO {
            Some(Box::pin(UsbAudioDriver::_instantiate(
                device.clone(),
                if_no,
                class,
            )))
        } else {
            None
        }
    }
}

pub struct UsbAudioDriver;

impl UsbAudioDriver {
    async fn _instantiate(
        device: Arc<UsbDeviceContext>,
        _if_no: UsbInterfaceNumber,
        _class: UsbClass,
    ) -> Result<Task, UsbError> {
        Ok(Task::new(Self::_usb_audio_task(device.clone())))
    }

    async fn _usb_audio_task(_device: Arc<UsbDeviceContext>) {
        // TODO: everything
    }
}
