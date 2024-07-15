//! USB Hub Class Driver (09_xx_xx)

use super::super::*;
use crate::task::{scheduler::Timer, Task};
use crate::*;
use core::mem::transmute;
use core::pin::Pin;
use core::time::Duration;
use futures_util::Future;

pub struct UsbHubStarter;

impl UsbHubStarter {
    #[inline]
    pub fn new() -> Box<dyn UsbClassDriverStarter> {
        Box::new(Self {})
    }

    async fn _instantiate(device: Arc<UsbDeviceContext>) -> Result<Task, UsbError> {
        let class = device.device().class();

        let config = device.device().current_configuration();
        let mut current_interface = None;
        for interface in config.interfaces() {
            if interface.class() == class {
                current_interface = Some(interface);
                break;
            }
        }
        let interface = match current_interface.or(config.interfaces().first()) {
            Some(v) => v,
            None => return Err(UsbError::InvalidDescriptor),
        };
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > UsbLength(8) {
            return Err(UsbError::InvalidDescriptor);
        }
        device.configure_endpoint(endpoint.descriptor()).unwrap();

        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT => {
                Ok(Task::new(UsbHub2Driver::_start_hub(device.clone(), ep, ps)))
            }
            UsbClass::HUB_SS => Ok(Task::new(UsbHub3Driver::_start_hub(device.clone(), ep, ps))),
            _ => Err(UsbError::Unsupported),
        }
    }
}

impl UsbClassDriverStarter for UsbHubStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceContext>,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>> {
        let class = device.device().class();
        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT | UsbClass::HUB_SS => {
                Some(Box::pin(Self::_instantiate(device.clone())))
            }
            _ => None,
        }
    }
}

/// USB2 Hub (FS, HS, HS-MTT)
pub struct UsbHub2Driver {
    device: Arc<UsbDeviceContext>,
    hub_desc: Usb2HubDescriptor,
    ep: UsbEndpointAddress,
    ps: UsbLength,
}

impl UsbHub2Driver {
    async fn _start_hub(device: Arc<UsbDeviceContext>, ep: UsbEndpointAddress, ps: UsbLength) {
        let is_mtt = device.device().class() == UsbClass::HUB_HS_MTT;

        let hub_desc: Usb2HubDescriptor = match UsbHubCommon::get_hub_descriptor(&device, 0).await {
            Ok(v) => v,
            Err(_err) => {
                // TODO:
                log!("USB2 GET HUB DESCRIPTOR {:?}", _err);
                return;
            }
        };
        match device.configure_hub2(&hub_desc, is_mtt) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB2 COFNIGURE HUB2 {:?}", _err);
                return;
            }
        }

        let hub = Arc::new(UsbHub2Driver {
            device: device.clone(),
            hub_desc,
            ep,
            ps,
        });
        let _ = hub._main_task().await;
    }

    async fn _main_task(self: &Arc<Self>) -> Result<(), UsbError> {
        let focus = self.device.focus_device();
        for port in self.hub_desc.ports() {
            self.set_port_feature(UsbHub2PortFeatureSel::PORT_POWER, port)
                .await?;
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;
        for port in self.hub_desc.ports() {
            self.clear_port_feature(UsbHub2PortFeatureSel::C_PORT_CONNECTION, port)
                .await?;
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        // Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;

        for port in self.hub_desc.ports() {
            let status = self.get_port_status(port).await?;
            if status
                .status
                .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
            {
                let _ = self.attach_device(port).await;
            }
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        drop(focus);

        let mut port_event = [0u8; 8];
        loop {
            match self
                .device
                .read_slice(self.ep, &mut port_event, UsbLength(1), self.ps)
                .await
            {
                Ok(_) => {
                    let focus = self.device.focus_device();
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    for port in self.hub_desc.ports() {
                        if (port_change_bitmap & (1 << port.0.get())) != 0 {
                            let status = self.get_port_status(port).await.unwrap();
                            if status
                                .change
                                .contains(UsbHub2PortChangeBit::C_PORT_CONNECTION)
                            {
                                Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;
                                self.clear_port_feature(
                                    UsbHub2PortFeatureSel::C_PORT_CONNECTION,
                                    port,
                                )
                                .await
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
                                {
                                    let _ = self.attach_device(port).await;
                                } else {
                                    let _ = self.detatch_device(port).await;
                                }
                            } else {
                                self.clear_status_changes(
                                    status,
                                    &[
                                        UsbHub2PortFeatureSel::C_PORT_ENABLE,
                                        UsbHub2PortFeatureSel::C_PORT_SUSPEND,
                                        UsbHub2PortFeatureSel::C_PORT_OVER_CURRENT,
                                        UsbHub2PortFeatureSel::C_PORT_RESET,
                                    ],
                                    port,
                                )
                                .await
                                .unwrap();
                            }
                        }
                    }
                    drop(focus);
                }
                Err(UsbError::Aborted) => break,
                Err(err) => {
                    log!("USB2 HUB READ ERROR {:?}", err);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub async fn port_reset(self: &Arc<Self>, port: UsbHubPortNumber) -> Result<(), UsbError> {
        let status = self.get_port_status(port).await?;
        self.clear_status_changes(
            status,
            &[
                UsbHub2PortFeatureSel::C_PORT_CONNECTION,
                UsbHub2PortFeatureSel::C_PORT_ENABLE,
                UsbHub2PortFeatureSel::C_PORT_SUSPEND,
                UsbHub2PortFeatureSel::C_PORT_OVER_CURRENT,
            ],
            port,
        )
        .await?;
        self.set_port_feature(UsbHub2PortFeatureSel::PORT_RESET, port)
            .await?;
        Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;
        let status = self.get_port_status(port).await?;
        self.clear_status_changes(status, &[UsbHub2PortFeatureSel::C_PORT_RESET], port)
            .await?;

        Ok(())
    }

    pub async fn attach_device(
        self: &Arc<Self>,
        port: UsbHubPortNumber,
    ) -> Result<UsbAddress, UsbError> {
        self.port_reset(port).await?;

        for _ in 0..3 {
            let status = self.get_port_status(port).await?;
            if status
                .status
                .contains(UsbHub2PortStatusBit::PORT_CONNECTION | UsbHub2PortStatusBit::PORT_ENABLE)
            {
                self.clear_status_changes(status, &[UsbHub2PortFeatureSel::C_PORT_ENABLE], port)
                    .await?;

                let speed = status.status.speed();
                return self.device.attach_child_device(port, speed).await;
            }
            Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;
        }

        Err(UsbError::InvalidParameter)
    }

    pub async fn detatch_device(self: &Arc<Self>, port: UsbHubPortNumber) -> Result<(), UsbError> {
        self.device.detach_child_device(port).await?;

        self.port_reset(port).await?;

        Ok(())
    }

    pub async fn get_port_status(
        self: &Arc<Self>,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub2PortStatus, UsbError> {
        UsbHubCommon::get_port_status(&self.device, port).await
    }

    pub async fn set_port_feature(
        self: &Arc<Self>,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(&self.device, feature_sel, port).await
    }

    pub async fn clear_port_feature(
        self: &Arc<Self>,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(&self.device, feature_sel, port).await
    }

    pub async fn clear_status_changes(
        self: &Arc<Self>,
        status: UsbHub2PortStatus,
        features: &[UsbHub2PortFeatureSel],
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        for feature in features {
            let changes = feature.status_change_bit();
            assert!(!changes.is_empty());
            if status.change.contains(changes) {
                self.clear_port_feature(*feature, port).await?;
            }
        }
        Ok(())
    }
}

/// USB3 Hub (SS)
pub struct UsbHub3Driver {
    device: Arc<UsbDeviceContext>,
    hub_desc: Usb3HubDescriptor,
    ep: UsbEndpointAddress,
    ps: UsbLength,
}

impl UsbHub3Driver {
    async fn _start_hub(device: Arc<UsbDeviceContext>, ep: UsbEndpointAddress, ps: UsbLength) {
        let hub_desc: Usb3HubDescriptor = match UsbHubCommon::get_hub_descriptor(&device, 0).await {
            Ok(v) => v,
            Err(_err) => {
                // TODO:
                log!("USB3 GET HUB DESCRIPTOR {:?}", _err);
                return;
            }
        };
        Self::set_hub_depth(&device).await.unwrap();

        match device.configure_hub3(&hub_desc) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB3 COFNIGURE HUB3 {:?}", _err);
                return;
            }
        }

        let hub = Arc::new(UsbHub3Driver {
            device: device.clone(),
            hub_desc,
            ep,
            ps,
        });
        let _ = hub._main_task().await;
    }

    async fn _main_task(self: Arc<Self>) -> Result<(), UsbError> {
        let focus = self.device.focus_device();
        for port in self.hub_desc.ports() {
            let status = self.get_port_status(port).await?;
            self.set_port_feature(UsbHub3PortFeatureSel::PORT_POWER, port)
                .await
                .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
            // Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
            if status
                .status
                .contains(UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE)
            {
                let _ = self.clone().attach_device(port).await;
            }
        }
        drop(focus);

        let mut port_event = [0u8; 8];
        loop {
            match self
                .device
                .read_slice(self.ep, &mut port_event, UsbLength(1), self.ps)
                .await
            {
                Ok(_) => {
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    let focus = self.device.focus_device();
                    for port in self.hub_desc.ports() {
                        if (port_change_bitmap & (1 << port.0.get())) != 0 {
                            let status = self.get_port_status(port).await.unwrap();
                            if status
                                .change
                                .contains(UsbHub3PortChangeBit::C_PORT_CONNECTION)
                            {
                                Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;
                                self.clear_port_feature(
                                    UsbHub3PortFeatureSel::C_PORT_CONNECTION,
                                    port,
                                )
                                .await
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub3PortStatusBit::PORT_CONNECTION)
                                {
                                    let _ = self.attach_device(port).await;
                                } else {
                                    let _ = self.detatch_device(port).await;
                                }
                            } else {
                                self.clear_status_changes(
                                    status,
                                    &[
                                        UsbHub3PortFeatureSel::C_BH_PORT_RESET,
                                        UsbHub3PortFeatureSel::C_PORT_RESET,
                                        UsbHub3PortFeatureSel::C_PORT_OVER_CURRENT,
                                        UsbHub3PortFeatureSel::C_PORT_LINK_STATE,
                                        UsbHub3PortFeatureSel::C_PORT_CONFIG_ERROR,
                                    ],
                                    port,
                                )
                                .await
                                .unwrap();
                            }
                        }
                    }
                    drop(focus);
                }
                Err(UsbError::Aborted) => break,
                Err(err) => {
                    log!("USB3 HUB READ ERROR {:?}", err);
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    pub async fn attach_device(
        self: &Arc<Self>,
        port: UsbHubPortNumber,
    ) -> Result<UsbAddress, UsbError> {
        self.set_port_feature(UsbHub3PortFeatureSel::BH_PORT_RESET, port)
            .await?;

        let deadline = Timer::new(self.hub_desc.power_on_to_power_good() * 2);
        loop {
            let status = self.get_port_status(port).await?;
            if deadline.is_expired()
                || status.status.contains(
                    UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE,
                )
            {
                break;
            }
            Timer::sleep_async(Duration::from_millis(10)).await;
        }

        let status = self.get_port_status(port).await?;
        self.clear_status_changes(
            status,
            &[
                UsbHub3PortFeatureSel::C_BH_PORT_RESET,
                UsbHub3PortFeatureSel::C_PORT_RESET,
                UsbHub3PortFeatureSel::C_PORT_OVER_CURRENT,
                UsbHub3PortFeatureSel::C_PORT_LINK_STATE,
                UsbHub3PortFeatureSel::C_PORT_CONFIG_ERROR,
            ],
            port,
        )
        .await?;

        let status = self.get_port_status(port).await.unwrap();
        if status
            .status
            .contains(UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE)
        {
            return self.device.attach_child_device(port, PSIV::SS).await;
        }

        Err(UsbError::InvalidParameter)
    }

    pub async fn detatch_device(&self, port: UsbHubPortNumber) -> Result<(), UsbError> {
        self.device.detach_child_device(port).await?;
        Ok(())
    }

    pub async fn set_hub_depth(device: &Arc<UsbDeviceContext>) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::SET_HUB_DEPTH,
                )
                .value(device.device().route_string().depth() as u16),
            )
            .await
    }

    pub async fn get_port_status(
        self: &Arc<Self>,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub3PortStatus, UsbError> {
        UsbHubCommon::get_port_status(&self.device, port).await
    }

    pub async fn set_port_feature(
        self: &Arc<Self>,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(&self.device, feature_sel, port).await
    }

    pub async fn clear_port_feature(
        self: &Arc<Self>,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(&self.device, feature_sel, port).await
    }

    pub async fn clear_status_changes(
        self: &Arc<Self>,
        status: UsbHub3PortStatus,
        features: &[UsbHub3PortFeatureSel],
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        for feature in features {
            let changes = feature.status_change_bit();
            assert!(!changes.is_empty());
            if status.change.contains(changes) {
                self.clear_port_feature(*feature, port).await?;
            }
        }
        Ok(())
    }
}

pub struct UsbHubCommon;

impl UsbHubCommon {
    #[inline]
    pub async fn get_hub_descriptor<T: UsbDescriptor>(
        device: &UsbDeviceContext,
        index: u8,
    ) -> Result<T, UsbError> {
        device
            .get_descriptor(UsbControlRequestBitmap::GET_CLASS, index)
            .await
    }

    #[inline]
    pub async fn set_hub_feature(
        device: &UsbDeviceContext,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::SET_FEATURE,
                )
                .value(feature_sel as u16),
            )
            .await
    }

    #[inline]
    pub async fn clear_hub_feature(
        device: &UsbDeviceContext,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::CLEAR_FEATURE,
                )
                .value(feature_sel as u16),
            )
            .await
    }

    #[inline]
    pub async fn set_port_feature<T>(
        device: &UsbDeviceContext,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x23),
                    UsbControlRequest::SET_FEATURE,
                )
                .value(feature_sel.into())
                .index(port.0.get() as u16),
            )
            .await
    }

    #[inline]
    pub async fn clear_port_feature<T>(
        device: &UsbDeviceContext,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x23),
                    UsbControlRequest::CLEAR_FEATURE,
                )
                .value(feature_sel.into())
                .index(port.0.get() as u16),
            )
            .await
    }

    pub async fn get_port_status<T: Copy>(
        device: &UsbDeviceContext,
        port: UsbHubPortNumber,
    ) -> Result<T, UsbError> {
        let mut data = [0; 4];
        match device
            .control_slice(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA3),
                    UsbControlRequest::GET_STATUS,
                )
                .value(0)
                .index(port.0.get() as u16),
                &mut data,
            )
            .await
        {
            Ok(_) => {
                let result = unsafe {
                    let p = &data[0] as *const _ as *const T;
                    *p
                };
                Ok(result)
            }
            Err(err) => Err(err),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UsbHub2PortStatus {
    status: UsbHub2PortStatusBit,
    change: UsbHub2PortChangeBit,
}

impl UsbHub2PortStatus {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            status: UsbHub2PortStatusBit::empty(),
            change: UsbHub2PortChangeBit::empty(),
        }
    }

    #[inline]
    pub const fn changes(change: UsbHub2PortChangeBit) -> Self {
        Self {
            status: UsbHub2PortStatusBit::empty(),
            change,
        }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        unsafe { transmute(*self) }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UsbHub3PortStatus {
    status: UsbHub3PortStatusBit,
    change: UsbHub3PortChangeBit,
}

impl UsbHub3PortStatus {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            status: UsbHub3PortStatusBit::empty(),
            change: UsbHub3PortChangeBit::empty(),
        }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        unsafe { transmute(*self) }
    }
}

/// USB Hub Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHubFeatureSel {
    C_HUB_LOCAL_POWER = 0,
    C_HUB_OVER_CURRENT = 1,
}

/// USB2 Hub Port Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHub2PortFeatureSel {
    PORT_CONNECTION = 0,
    PORT_ENABLE = 1,
    PORT_SUSPEND = 2,
    PORT_OVER_CURRENT = 3,
    PORT_RESET = 4,
    PORT_POWER = 8,
    PORT_LOW_SPEED = 9,
    C_PORT_CONNECTION = 16,
    C_PORT_ENABLE = 17,
    C_PORT_SUSPEND = 18,
    C_PORT_OVER_CURRENT = 19,
    C_PORT_RESET = 20,
    PORT_TEST = 21,
    PORT_INDICATOR = 22,
}

impl UsbHub2PortFeatureSel {
    #[inline]
    pub const fn status_change_bit(&self) -> UsbHub2PortChangeBit {
        match *self {
            UsbHub2PortFeatureSel::C_PORT_CONNECTION => UsbHub2PortChangeBit::C_PORT_CONNECTION,
            UsbHub2PortFeatureSel::C_PORT_ENABLE => UsbHub2PortChangeBit::C_PORT_ENABLE,
            UsbHub2PortFeatureSel::C_PORT_SUSPEND => UsbHub2PortChangeBit::C_PORT_SUSPEND,
            UsbHub2PortFeatureSel::C_PORT_OVER_CURRENT => UsbHub2PortChangeBit::C_PORT_OVER_CURRENT,
            UsbHub2PortFeatureSel::C_PORT_RESET => UsbHub2PortChangeBit::C_PORT_RESET,
            _ => UsbHub2PortChangeBit::empty(),
        }
    }
}

impl From<UsbHub2PortFeatureSel> for u16 {
    #[inline]
    fn from(val: UsbHub2PortFeatureSel) -> Self {
        val as u16
    }
}

/// USB3 Hub Port Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHub3PortFeatureSel {
    PORT_CONNECTION = 0,
    PORT_OVER_CURRENT = 3,
    PORT_RESET = 4,
    PORT_LINK_STATE = 5,
    PORT_POWER = 8,
    C_PORT_CONNECTION = 16,
    C_PORT_OVER_CURRENT = 19,
    C_PORT_RESET = 20,
    PORT_U1_TIMEOUT = 23,
    PORT_U2_TIMEOUT = 24,
    C_PORT_LINK_STATE = 25,
    C_PORT_CONFIG_ERROR = 26,
    PORT_REMOTE_WAKE_MASK = 27,
    BH_PORT_RESET = 28,
    C_BH_PORT_RESET = 29,
    FORCE_LINKPM_ACCEPT = 30,
}

impl UsbHub3PortFeatureSel {
    #[inline]
    pub const fn status_change_bit(&self) -> UsbHub3PortChangeBit {
        match *self {
            UsbHub3PortFeatureSel::C_PORT_CONNECTION => UsbHub3PortChangeBit::C_PORT_CONNECTION,
            UsbHub3PortFeatureSel::C_PORT_OVER_CURRENT => UsbHub3PortChangeBit::C_PORT_OVER_CURRENT,
            UsbHub3PortFeatureSel::C_PORT_RESET => UsbHub3PortChangeBit::C_PORT_RESET,
            UsbHub3PortFeatureSel::C_PORT_LINK_STATE => UsbHub3PortChangeBit::C_PORT_LINK_STATE,
            UsbHub3PortFeatureSel::C_PORT_CONFIG_ERROR => UsbHub3PortChangeBit::C_PORT_CONFIG_ERROR,
            UsbHub3PortFeatureSel::C_BH_PORT_RESET => UsbHub3PortChangeBit::C_BH_PORT_RESET,
            _ => UsbHub3PortChangeBit::empty(),
        }
    }
}

impl From<UsbHub3PortFeatureSel> for u16 {
    #[inline]
    fn from(val: UsbHub3PortFeatureSel) -> Self {
        val as u16
    }
}

my_bitflags! {
    /// USB2 Hub Port Status Bits
    pub struct UsbHub2PortStatusBit: u16 {
        const PORT_CONNECTION   = 0b0000_0000_0000_0001;
        const PORT_ENABLE       = 0b0000_0000_0000_0010;
        const PORT_SUSPEND      = 0b0000_0000_0000_0100;
        const PORT_OVER_CURRENT = 0b0000_0000_0000_1000;
        const PORT_RESET        = 0b0000_0000_0001_0000;

        const PORT_POWER        = 0b0000_0001_0000_0000;
        const PORT_LOW_SPEED    = 0b0000_0010_0000_0000;
        const PORT_HIGH_SPEED   = 0b0000_0100_0000_0000;
        const PORT_TEST         = 0b0000_1000_0000_0000;
        const PORT_INDICATOR    = 0b0001_0000_0000_0000;
    }
}

impl UsbHub2PortStatusBit {
    #[inline]
    pub fn speed(&self) -> PSIV {
        if self.contains(Self::PORT_LOW_SPEED) {
            PSIV::LS
        } else if self.contains(Self::PORT_HIGH_SPEED) {
            PSIV::HS
        } else {
            PSIV::FS
        }
    }
}

my_bitflags! {
    /// USB2 Hub Port Status Change Bits
    pub struct UsbHub2PortChangeBit: u16 {
        const C_PORT_CONNECTION     = 0b0000_0000_0000_0001;
        const C_PORT_ENABLE         = 0b0000_0000_0000_0010;
        const C_PORT_SUSPEND        = 0b0000_0000_0000_0100;
        const C_PORT_OVER_CURRENT   = 0b0000_0000_0000_1000;
        const C_PORT_RESET          = 0b0000_0000_0001_0000;
    }
}

my_bitflags! {
    /// USB3 Hub Port Status Bits
    pub struct UsbHub3PortStatusBit: u16 {
        const PORT_CONNECTION   = 0b0000_0000_0000_0001;
        const PORT_ENABLE       = 0b0000_0000_0000_0010;
        const PORT_OVER_CURRENT = 0b0000_0000_0000_1000;
        const PORT_RESET        = 0b0000_0000_0001_0000;
        const PORT_LINK_STATE   = 0b0000_0001_1110_0000;
        const PORT_POWER        = 0b0000_0010_0000_0000;
        const PORT_SPEED        = 0b0001_1100_0000_0000;
    }
}

impl UsbHub3PortStatusBit {
    #[inline]
    pub const fn link_state_raw(&self) -> usize {
        ((self.bits() & Self::PORT_LINK_STATE.bits()) as usize) >> 5
    }

    #[inline]
    pub fn link_state(&self) -> Option<Usb3LinkState> {
        unsafe { transmute(self.link_state_raw() as u8) }
    }
}

my_bitflags! {
    /// USB3 Hub Port Status Change Bits
    pub struct UsbHub3PortChangeBit: u16 {
        const C_PORT_CONNECTION     = 0b0000_0000_0000_0001;
        const C_PORT_OVER_CURRENT   = 0b0000_0000_0000_1000;
        const C_PORT_RESET          = 0b0000_0000_0001_0000;
        const C_BH_PORT_RESET       = 0b0000_0000_0010_0000;
        const C_PORT_LINK_STATE     = 0b0000_0000_0100_0000;
        const C_PORT_CONFIG_ERROR   = 0b0000_0000_1000_0000;
    }
}
