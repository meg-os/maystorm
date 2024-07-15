//! USB HID Class Driver (03_xx_xx)

use super::super::*;
use crate::io::hid_mgr::*;
use crate::task::{scheduler::Timer, Task};
use crate::*;
use core::pin::Pin;
use core::time::Duration;
use futures_util::Future;
use megstd::io::hid::*;

pub struct UsbHidStarter;

impl UsbHidStarter {
    #[inline]
    pub fn new() -> Box<dyn UsbInterfaceDriverStarter> {
        Box::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbHidStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>> {
        if class.base_class() == UsbBaseClass::HID {
            Some(Box::pin(UsbHidDriver::_instantiate(
                device.clone(),
                if_no,
                class,
            )))
        } else {
            None
        }
    }
}

pub struct UsbHidDriver;

impl UsbHidDriver {
    const BUFFER_LEN: UsbLength = UsbLength(64);

    async fn _instantiate(
        device: Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Result<Task, UsbError> {
        let Some(interface) = device
            .device()
            .current_configuration()
            .find_interface(if_no, None)
        else {
            return Err(UsbError::InvalidParameter);
        };
        let Some(endpoint) = interface.endpoints().first() else {
            return Err(UsbError::InvalidDescriptor);
        };
        if !endpoint.is_dir_in() {
            return Err(UsbError::InvalidDescriptor);
        }
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > Self::BUFFER_LEN {
            return Err(UsbError::InvalidDescriptor);
        }

        let report_desc = interface
            .hid_reports_by(UsbDescriptorType::HidReport)
            .unwrap_or(&[]);
        let report_desc = match HidParsedReport::parse(report_desc) {
            Ok(v) => v,
            Err(err) => {
                log!("HID PARSE ERROR {}", err);
                return Err(UsbError::InvalidDescriptor);
            }
        };

        device.configure_endpoint(endpoint.descriptor()).unwrap();

        // disable boot protocol
        if class.sub_class() == UsbSubClass(1) {
            let _result = Self::set_boot_protocol(&device, if_no, false).await.is_ok();
        }

        Ok(Task::new(Self::_usb_hid_task(
            device.clone(),
            if_no,
            ep,
            ps,
            report_desc,
        )))
    }

    async fn _usb_hid_task(
        device: Arc<UsbDeviceContext>,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        ps: UsbLength,
        report_desc: HidParsedReport,
    ) {
        let addr = device.device().addr();

        // log!(
        //     "HID {:03}:{} VEN {} DEV {} UP {:08x} {:?} {}",
        //     addr.as_u8(),
        //     if_no.0,
        //     device.device().vid(),
        //     device.device().pid(),
        //     report_desc.primary_app().unwrap().usage().0,
        //     report_desc.report_ids.iter().map(|v| v.as_u8()),
        //     device.device().preferred_device_name().unwrap_or_default(),
        // );
        // for app in report_desc.applications() {
        //     println!(
        //         " {:02x} {} I {} O {} F {}",
        //         app.report_id().map(|v| v.as_u8()).unwrap_or_default(),
        //         app.usage(),
        //         app.bit_count_for_input(),
        //         app.bit_count_for_output(),
        //         app.bit_count_for_feature(),
        //     );
        //     println!(" {:?}", app.entries().collect::<Vec<_>>());
        // }

        for app in report_desc
            .primary_app()
            .into_iter()
            .chain(report_desc.applications())
        {
            let mut data = Vec::new();
            data.resize(
                (app.bit_count_for_feature().max(app.bit_count_for_output()) + 7) / 8,
                0,
            );
            let mut writer = HidBitStreamWriter::new(data.as_mut_slice());
            match app.usage() {
                HidUsage::KEYBOARD => {
                    // Flashing LED on the keyboard
                    let len = UsbLength(((app.bit_count_for_output() + 7) / 8) as u16);
                    if !len.is_empty() {
                        for item in app.output_items() {
                            if item.report_size() == 1
                                && item.usage_min().usage_page() == UsagePage::LED
                            {
                                for _ in item.usage_range() {
                                    let _ = writer.write_item(item, 1);
                                }
                            } else {
                                writer.advance_by(item);
                            }
                        }

                        match Self::set_report(
                            &device,
                            if_no,
                            HidReportType::Output,
                            app.report_id(),
                            len,
                            writer.data(),
                        )
                        .await
                        {
                            Ok(_) => (),
                            Err(_) => break,
                        }
                        Timer::sleep_async(Duration::from_millis(100)).await;

                        writer.clear();

                        let _ = Self::set_report(
                            &device,
                            if_no,
                            HidReportType::Output,
                            app.report_id(),
                            len,
                            writer.data(),
                        )
                        .await
                        .unwrap();
                        Timer::sleep_async(Duration::from_millis(50)).await;
                    }
                }

                // HidUsage::DEVICE_CONFIGURATION => {
                //     let len = (app.bit_count_for_feature() + 7) / 8;
                //     if len > 0 {
                //         for item in app.feature_items() {
                //             match item.usage_min() {
                //                 HidUsage::DEVICE_MODE => {
                //                     let _ = writer
                //                         .write_item(item, DeviceMode::MultiInputDevice as u32);
                //                 }
                //                 // HidUsage::SURFACE_SWITCH | HidUsage::BUTTON_SWITCH => {
                //                 //     let _ = writer.write_item(item, 1);
                //                 // }
                //                 _ => {
                //                     writer.advance_by(item);
                //                 }
                //             }
                //         }

                //         let _ = Self::set_report(
                //             &device,
                //             if_no,
                //             HidReportType::Feature,
                //             app.report_id(),
                //             len,
                //             data.as_slice(),
                //         )
                //         .await
                //         .unwrap();
                //     }
                // }
                _ => (),
            }
        }

        let mut key_state = KeyboardState::new();
        let mut mouse_state = MouseState::empty();
        let mut buffer = Vec::new();
        loop {
            match device
                .read_to_vec(ep, &mut buffer, UsbLength::ZERO, ps)
                .await
            {
                Ok(UsbLength::ZERO) => {
                    // Read size is zero, so nothing to do.
                }
                Ok(_) => {
                    // if report_desc.has_report_id() && buffer.iter().fold(0, |a, b| a | *b) != 0 {
                    //     println!("HID {:?}", HexDump(&buffer));
                    // }

                    let (app, _report_id) = if report_desc.has_report_id() {
                        let report_id = HidReportId::new(buffer[0]);
                        let app =
                            report_id.and_then(|report_id| report_desc.app_by_report_id(report_id));
                        (app, report_id)
                    } else {
                        (report_desc.primary_app(), None)
                    };

                    let Some(app) = app else { continue };
                    if buffer.len() * 8
                        < report_desc.initial_bit_position() + app.bit_count_for_input()
                    {
                        // Some devices send smaller garbage data
                        continue;
                    }

                    let mut reader = HidBitStreamReader::new(
                        buffer.as_slice(),
                        report_desc.initial_bit_position(),
                    );
                    match app.usage() {
                        HidUsage::KEYBOARD => {
                            let mut report = KeyReportRaw::default();
                            for item in app.input_items() {
                                if item.usage_min() == HidUsage::from(Usage::MOD_MIN)
                                    && item.usage_max() == HidUsage::from(Usage::MOD_MAX)
                                {
                                    // Modifier bit array
                                    report.modifier = reader
                                        .read_bit_array(item)
                                        .map(|v| Modifier::from_bits_retain(v as u8))
                                        .unwrap_or_default();
                                } else if item.report_size() == 8
                                    && item.is_array()
                                    && item.usage_min().usage_page() == UsagePage::KEYBOARD
                                {
                                    // Keyboard usage array
                                    let read_data = (0..item.report_count())
                                        .flat_map(|_| {
                                            reader.read_value(item).map(|v| Usage(v as u8)).ok()
                                        })
                                        .collect::<Vec<_>>();
                                    for (data, usage) in
                                        report.keydata.iter_mut().zip(read_data.into_iter())
                                    {
                                        *data = usage;
                                    }
                                } else {
                                    reader.advance_by(item);
                                }
                            }
                            key_state.process_report(report);
                        }
                        HidUsage::MOUSE => {
                            let mut is_absolute = false;
                            let mut report = MouseReport::default();
                            for item in app.input_items() {
                                match item.usage_min() {
                                    HidUsage::BUTTON_1 => {
                                        if item.is_variable() && item.report_size() == 1 {
                                            report.buttons = reader
                                                .read_bit_array(item)
                                                .map(|v| MouseButton::from_bits_retain(v as u8))
                                                .unwrap();
                                        }
                                    }
                                    HidUsage::X => {
                                        if item.is_relative() {
                                            report.x =
                                                reader.read_value_signed(item).unwrap() as isize;
                                        } else {
                                            is_absolute = true;
                                            mouse_state.max_x = item.logical_max() as i32;
                                            report.x = reader.read_value(item).unwrap() as isize;
                                        }
                                    }
                                    HidUsage::Y => {
                                        if item.is_relative() {
                                            report.y =
                                                reader.read_value_signed(item).unwrap() as isize;
                                        } else {
                                            is_absolute = true;
                                            mouse_state.max_y = item.logical_max() as i32;
                                            report.y = reader.read_value(item).unwrap() as isize;
                                        }
                                    }
                                    HidUsage::WHEEL => {
                                        report.wheel =
                                            reader.read_value_signed(item).unwrap_or_default()
                                                as isize;
                                    }
                                    _ => {
                                        reader.advance_by(item);
                                    }
                                }
                            }
                            if is_absolute {
                                mouse_state.process_absolute_report(report);
                            } else {
                                mouse_state.process_relative_report(report);
                            }
                        }
                        HidUsage::CONSUMER_CONTROL => {
                            let mut bitmap = Vec::new();
                            for item in app.input_items() {
                                if item.is_const() {
                                    reader.advance_by(item);
                                    continue;
                                };
                                if item.is_variable() && item.report_size() == 1 {
                                    if let Ok(data) = reader.read_value(item) {
                                        if data != 0 {
                                            bitmap.push(item.usage_min());
                                        }
                                    }
                                } else {
                                    reader.advance_by(item);
                                }
                            }
                            if bitmap.len() > 0 {
                                log!("CONSUME {:?}", bitmap);
                            }
                        }
                        _ => {
                            // TODO: Other app
                        }
                    }
                }
                Err(UsbError::Aborted) => {
                    println!("USB HID Aborted {}:{}", addr.as_u8(), if_no.0);
                    break;
                }
                // Err(UsbError::InvalidParameter) => {
                //     log!(
                //         "USB HID error {}:{} {:?}",
                //         addr.as_u8(),
                //         if_no.0,
                //         UsbError::InvalidParameter
                //     );
                //     break;
                // }
                Err(err) => {
                    // TODO: error
                    log!("USB HID error {}:{} {:?}", addr.as_u8(), if_no.0, err);
                }
            }
        }
    }

    #[inline]
    pub async fn set_boot_protocol(
        device: &UsbDeviceContext,
        if_no: UsbInterfaceNumber,
        is_boot: bool,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x21),
                    UsbControlRequest::HID_SET_PROTOCOL,
                )
                .value((!is_boot) as u16)
                .index_if(if_no),
            )
            .await
    }

    #[inline]
    pub async fn get_report(
        device: &UsbDeviceContext,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: Option<HidReportId>,
        len: UsbLength,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        device
            .control_vec(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA1),
                    UsbControlRequest::HID_GET_REPORT,
                )
                .value(
                    (report_id.map(|v| v.as_u8()).unwrap_or_default() as u16)
                        + (report_type as u16) * 256,
                )
                .index_if(if_no),
                vec,
                len,
                len,
            )
            .await
    }

    #[inline]
    pub async fn set_report(
        device: &UsbDeviceContext,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: Option<HidReportId>,
        max_len: UsbLength,
        data: &[u8],
    ) -> Result<(), UsbError> {
        device
            .control_send(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x21),
                    UsbControlRequest::HID_SET_REPORT,
                )
                .value(
                    (report_id.map(|v| v.as_u8()).unwrap_or_default() as u16)
                        + (report_type as u16) * 256,
                )
                .index_if(if_no),
                max_len,
                data,
            )
            .await
    }
}
