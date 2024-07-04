//! XInput Class Driver (FF_5D_01)

use super::super::*;
use crate::io::hid_mgr::{GameInput, GameInputManager};
use crate::sync::RwLock;
use crate::task::Task;
use crate::*;
use core::pin::Pin;
use futures_util::Future;

pub struct XInputStarter;

impl XInputStarter {
    #[inline]
    pub fn new() -> Box<dyn UsbInterfaceDriverStarter> {
        Box::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for XInputStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>> {
        if class == UsbClass::XINPUT {
            Some(Box::pin(XInputDriver::_instantiate(
                device.clone(),
                if_no,
                class,
            )))
        } else {
            None
        }
    }
}

pub struct XInputDriver;

impl XInputDriver {
    async fn _instantiate(
        device: Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        _class: UsbClass,
    ) -> Result<Task, UsbError> {
        let interface = match device
            .device()
            .current_configuration()
            .find_interface(if_no, None)
        {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => return Err(UsbError::InvalidDescriptor),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();

        device.configure_endpoint(endpoint.descriptor()).unwrap();

        Ok(Task::new(Self::_xinput_task(device.clone(), if_no, ep, ps)))
    }

    async fn _xinput_task(
        device: Arc<UsbDeviceContext>,
        _if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        ps: UsbLength,
    ) {
        let addr = device.device().addr();
        let input = Arc::new(RwLock::new(GameInput::empty()));
        let _handle = GameInputManager::connect_new_input(input.clone());
        let mut buffer = [0u8; 512];
        loop {
            match device
                .read_slice(ep, &mut buffer, XInputMsg14::MIN_LEN, ps)
                .await
            {
                Ok(_) => {
                    if buffer[0] == XInputMsg14::VALID_TYPE && buffer[1] == XInputMsg14::VALID_LEN {
                        let data = unsafe { *(&buffer[2] as *const _ as *const GameInput) };
                        input.write().unwrap().copy_from(&data);
                    }
                }
                Err(UsbError::Aborted) => break,
                Err(err) => {
                    log!("XINPUT READ ERROR {:?} {:?}", addr.as_u8(), err);
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct XInputMsg14 {
    _type: u8,
    len: u8,
    button1: u8,
    button2: u8,
    lt: u8,
    rt: u8,
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
}

impl XInputMsg14 {
    pub const VALID_TYPE: u8 = 0;
    pub const VALID_LEN: u8 = 0x14;
    pub const MIN_LEN: UsbLength = UsbLength(14);
}
