//! Universal Serial Bus
//!
//! ```text
//!  ┏━○
//! ○┻┳━|＞
//!   ┗■
//! ```

use core::fmt;
use core::mem::{transmute, transmute_copy};
use core::num::NonZeroU8;
use core::time::Duration;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbVersion(pub u16);

impl UsbVersion {
    pub const USB1_0: Self = Self(0x0100);
    pub const USB1_1: Self = Self(0x0110);
    pub const USB2_0: Self = Self(0x0200);
    pub const USB3_0: Self = Self(0x0300);
    pub const USB3_1: Self = Self(0x0310);
    pub const USB3_2: Self = Self(0x0320);

    /// BOS descriptors are only supported for version number 0x0201 and above.
    pub const BOS_MIN: Self = Self(0x0201);
}

impl fmt::Display for UsbVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let part1 = (self.0 >> 12) & 0x0F;
        let part2 = (self.0 >> 8) & 0x0F;
        let part3 = (self.0 >> 4) & 0x0F;
        // let part4 = self.0 & 0x0F;
        write!(f, "{}.{}", part1 * 10 + part2, part3)
    }
}

/// Valid USB bus addresses are 1 to 127.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbAddress(NonZeroU8);

impl UsbAddress {
    #[inline]
    pub const unsafe fn from_nonzero_unchecked(v: NonZeroU8) -> Self {
        Self(v)
    }

    #[inline]
    pub fn from_nonzero(v: NonZeroU8) -> Option<Self> {
        Self::from_u8(v.get())
    }

    #[inline]
    pub fn from_u8(v: u8) -> Option<Self> {
        (v > 0 && v < 128).then(|| unsafe { Self(NonZeroU8::new_unchecked(v)) })
    }

    #[inline]
    pub const fn as_u8(&self) -> u8 {
        self.0.get()
    }
}

/// 16-bit word type used in the USB descriptor.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UsbWord([u8; 2]);

impl UsbWord {
    #[inline]
    pub const fn from_u16(val: u16) -> Self {
        Self(val.to_le_bytes())
    }

    #[inline]
    pub const fn as_u16(&self) -> u16 {
        u16::from_le_bytes(self.0)
    }

    #[inline]
    pub const fn as_length(&self) -> UsbLength {
        UsbLength(self.as_u16())
    }
}

impl From<UsbWord> for u16 {
    #[inline]
    fn from(v: UsbWord) -> Self {
        v.as_u16()
    }
}

impl fmt::Debug for UsbWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.as_u16())
    }
}

/// USB Vendor Id
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbVendorId(pub u16);

impl fmt::Display for UsbVendorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

/// USB Product Id
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbProductId(pub u16);

impl fmt::Display for UsbProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

/// USB Class code (BaseClass - SubClass - Protocol)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbClass(pub u32);

impl UsbClass {
    pub const COMPOSITE: Self = Self(0x00_00_00);
    pub const MIDI_STREAMING: Self = Self(0x01_03_00);
    pub const HID_GENERIC: Self = Self(0x03_00_00);
    pub const HID_BOOT_KEYBOARD: Self = Self(0x03_01_01);
    pub const HID_BOOT_MOUSE: Self = Self(0x03_01_02);
    pub const STILL_IMAGING: Self = Self(0x06_01_01);
    pub const MSD_BULK_ONLY: Self = Self(0x08_06_50);
    pub const FLOPPY: Self = Self(0x08_04_00);
    pub const HUB_FS: Self = Self(0x09_00_00);
    pub const HUB_HS_STT: Self = Self(0x09_00_01);
    pub const HUB_HS_MTT: Self = Self(0x09_00_02);
    pub const HUB_SS: Self = Self(0x09_00_03);
    pub const BLUETOOTH: Self = Self(0xE0_01_01);
    pub const IAD: Self = Self(0xEF_02_01);
    pub const XINPUT: Self = Self(0xFF_5D_01);
    pub const XINPUT_HEADSET: Self = Self(0xFF_5D_02);
    pub const XINPUT_IF2: Self = Self(0xFF_5D_03);
    pub const XINPUT_IF3: Self = Self(0xFF_5D_04);

    #[inline]
    pub const fn new(
        base_class: UsbBaseClass,
        sub_class: UsbSubClass,
        protocol: UsbProtocolCode,
    ) -> Self {
        Self(((base_class.0 as u32) << 16) | ((sub_class.0 as u32) << 8) | (protocol.0 as u32))
    }

    #[inline]
    pub const fn base_class(&self) -> UsbBaseClass {
        UsbBaseClass((self.0 >> 16) as u8)
    }

    #[inline]
    pub const fn sub_class(&self) -> UsbSubClass {
        UsbSubClass((self.0 >> 8) as u8)
    }

    #[inline]
    pub const fn protocol(&self) -> UsbProtocolCode {
        UsbProtocolCode(self.0 as u8)
    }

    #[inline]
    pub fn class_string(&self, is_interface: bool) -> Option<&'static str> {
        #[rustfmt::skip]
        let base_class_entries = [
            ( 0x01, UsbBaseClass::COMPOSITE, "Composite Device" ),
            ( 0x02, UsbBaseClass::AUDIO, "Audio Device" ),
            ( 0x03, UsbBaseClass::COMM, "Communication Device" ),
            ( 0x02, UsbBaseClass::HID, "Human Interface Device" ),
            ( 0x02, UsbBaseClass::PRINTER, "Printer" ),
            ( 0x02, UsbBaseClass::STORAGE, "Storage Device" ),
            ( 0x01, UsbBaseClass::HUB, "Hub" ),
            ( 0x02, UsbBaseClass::CDC_DATA, "CDC Data"),
            ( 0x02, UsbBaseClass::VIDEO, "Video Device" ),
            ( 0x02, UsbBaseClass::AUDIO_VIDEO, "Audio/Video Device" ),
            ( 0x01, UsbBaseClass::BILLBOARD, "Billboard Device" ),
            ( 0x02, UsbBaseClass::TYPE_C_BRIDGE, "Type-C Bridge" ),
            ( 0x03, UsbBaseClass::DIAGNOSTIC, "Diagnostic Device" ),
            ( 0x02, UsbBaseClass::WIRELESS, "Wireless Device" ),
            ( 0x02, UsbBaseClass::APPLICATION_SPECIFIC, "Application Specific" ),
            ( 0x03, UsbBaseClass::VENDOR_SPECIFIC, "Vendor Specific" ),
        ];

        #[rustfmt::skip]
        let full_class_entries = [
            (UsbClass::MIDI_STREAMING, "MIDI Streaming" ),
            (UsbClass::HID_BOOT_KEYBOARD, "HID Keyboard" ),
            (UsbClass::HID_BOOT_MOUSE, "HID Mouse" ),
            (UsbClass::MSD_BULK_ONLY, "Mass Storage Device" ),
            (UsbClass::FLOPPY, "Floppy Drive"),
            (UsbClass::STILL_IMAGING, "Still Imaging Device"),
            (UsbClass::HUB_FS, "USB1 Hub"),
            (UsbClass::HUB_HS_STT, "USB2 Hub"),
            (UsbClass::HUB_HS_MTT, "USB2 Hub with MTT"),
            (UsbClass::HUB_SS, "USB3 Hub"),
            (UsbClass::BLUETOOTH, "Bluetooth Interface"),
            (UsbClass::IAD, "Interface Association Descriptor"),
            (UsbClass::XINPUT, "XInput Device"),
        ];

        let bitmap = 1u8 << (is_interface as usize);
        full_class_entries
            .iter()
            .find(|v| v.0 == *self)
            .map(|v| v.1)
            .or_else(|| {
                base_class_entries
                    .iter()
                    .find(|v| (v.0 & bitmap) != 0 && v.1 == self.base_class())
                    .map(|v| v.2)
            })
    }
}

impl fmt::Display for UsbClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:06x}", self.0)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbBaseClass(pub u8);

impl UsbBaseClass {
    pub const COMPOSITE: Self = Self(0x00);
    pub const AUDIO: Self = Self(0x01);
    pub const COMM: Self = Self(0x02);
    pub const HID: Self = Self(0x03);
    pub const PHYSICAL: Self = Self(0x05);
    pub const IMAGE: Self = Self(0x06);
    pub const PRINTER: Self = Self(0x07);
    pub const STORAGE: Self = Self(0x08);
    pub const HUB: Self = Self(0x09);
    pub const CDC_DATA: Self = Self(0x0A);
    pub const SMART_CARD: Self = Self(0x0B);
    pub const CONTENT_SECURITY: Self = Self(0x0C);
    pub const VIDEO: Self = Self(0x0E);
    pub const PERSONAL_HEALTHCARE: Self = Self(0x0F);
    pub const AUDIO_VIDEO: Self = Self(0x10);
    pub const BILLBOARD: Self = Self(0x11);
    pub const TYPE_C_BRIDGE: Self = Self(0x12);
    pub const DIAGNOSTIC: Self = Self(0xDC);
    pub const WIRELESS: Self = Self(0xE0);
    pub const MISCELLANEOUS: Self = Self(0xEF);
    pub const APPLICATION_SPECIFIC: Self = Self(0xFE);
    pub const VENDOR_SPECIFIC: Self = Self(0xFF);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbSubClass(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbProtocolCode(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbConfigurationValue(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbInterfaceNumber(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbAlternateSettingNumber(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbLength(pub u16);

impl UsbLength {
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }
}

/// USB Descriptor type
#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbDescriptorType {
    Device = 1,
    Configuration,
    String,
    Interface,
    Endpoint,
    DeviceQualifier,
    InterfaceAssociation = 11,
    BinaryObjectStore = 15,
    DeviceCapability,
    HidClass = 0x21,
    HidReport,
    HidPhysical,
    Hub = 0x29,
    Hub3 = 0x2A,
    SuperspeedUsbEndpointCompanion = 48,
    SuperspeedplusIsochronousEndpointCompanion = 49,
}

impl UsbDescriptorType {
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        unsafe { transmute(v) }
    }
}

/// USB Device Capability type
#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbDeviceCapabilityType {
    WirelessUsb = 1,
    Usb2_0Extention,
    SuperspeedUsb,
    ContainerId,
    Platform,
    PowerDelivery,
    BatteryInfo,
    PdConsumerPort,
    PdProviderPort,
    SuperspeedPlus,
    PrecisionTimeMeasurement,
    WirelessUsbExt,
    Billboard,
    Authentication,
    BillboardEx,
    ConfigurationSummary,
}

impl UsbDeviceCapabilityType {
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        unsafe { transmute(v) }
    }
}

/// A trait compatible with standard USB descriptors
pub unsafe trait UsbDescriptor: Sized {
    const DESCRIPTOR_TYPE: UsbDescriptorType;

    #[inline]
    fn len(&self) -> usize {
        let stub: &UsbStandardDescriptorStub = unsafe { transmute_copy(&self) };
        stub.length as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        let stub: &UsbStandardDescriptorStub = unsafe { transmute_copy(&self) };
        stub.descriptor_type
    }

    #[inline]
    fn from_slice(slice: &[u8]) -> Option<&Self> {
        // minimal valid usb descriptor is greater than 2
        if slice.len() < 2 {
            return None;
        }
        let temp = unsafe { &*(slice.as_ptr() as *const Self) };
        (temp.len() <= slice.len() && temp.descriptor_type() == Self::DESCRIPTOR_TYPE).then(|| temp)
    }
}

/// A type compatible with standard USB device capabilities
pub unsafe trait UsbDeviceCapabilityDescriptor: UsbDescriptor {
    const CAPABILITY_TYPE: UsbDeviceCapabilityType;

    #[inline]
    fn capability_type(&self) -> UsbDeviceCapabilityType {
        let stub: &UsbStandardDescriptorStub = unsafe { transmute_copy(&self) };
        stub.dev_capability_type
    }
}

unsafe impl<T: UsbDeviceCapabilityDescriptor> UsbDescriptor for T {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::DeviceCapability;
}

#[repr(C, packed)]
/// A type compatible with standard USB descriptors
struct UsbStandardDescriptorStub {
    length: u8,
    descriptor_type: UsbDescriptorType,
    dev_capability_type: UsbDeviceCapabilityType,
}

/// USB Device Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdUSB: UsbWord,
    pub bDeviceClass: UsbBaseClass,
    pub bDeviceSubClass: UsbSubClass,
    pub bDeviceProtocol: UsbProtocolCode,
    pub bMaxPacketSize0: u8,
    pub idVendor: UsbWord,
    pub idProduct: UsbWord,
    pub bcdDevice: UsbWord,
    pub iManufacturer: u8,
    pub iProduct: u8,
    pub iSerialNumber: u8,
    pub bNumConfigurations: u8,
}

unsafe impl UsbDescriptor for UsbDeviceDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Device;
}

impl UsbDeviceDescriptor {
    #[inline]
    pub const fn usb_version(&self) -> UsbVersion {
        UsbVersion(self.bcdUSB.as_u16())
    }

    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        UsbVendorId(self.idVendor.as_u16())
    }

    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        UsbProductId(self.idProduct.as_u16())
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bDeviceClass,
            self.bDeviceSubClass,
            self.bDeviceProtocol,
        )
    }

    #[inline]
    pub const fn max_packet_size(&self) -> usize {
        self.bMaxPacketSize0 as usize
    }

    #[inline]
    pub const fn manufacturer_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iManufacturer)
    }

    #[inline]
    pub const fn product_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iProduct)
    }

    #[inline]
    pub const fn serial_number_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iSerialNumber)
    }
}

/// USB Configuration Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbConfigurationDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub wTotalLength: UsbWord,
    pub bNumInterface: u8,
    pub bConfigurationValue: UsbConfigurationValue,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub bMaxPower: u8,
}

unsafe impl UsbDescriptor for UsbConfigurationDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Configuration;
}

impl UsbConfigurationDescriptor {
    #[inline]
    pub const fn total_length(&self) -> UsbLength {
        self.wTotalLength.as_length()
    }

    #[inline]
    pub const fn num_interface(&self) -> u8 {
        self.bNumInterface
    }

    #[inline]
    pub const fn configuration_value(&self) -> UsbConfigurationValue {
        self.bConfigurationValue
    }

    #[inline]
    pub const fn configuration_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iConfiguration)
    }

    #[inline]
    pub const fn max_power(&self) -> u8 {
        self.bMaxPower
    }
}

#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbStringDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub wLangId: UsbWord,
}

unsafe impl UsbDescriptor for UsbStringDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::String;
}

impl UsbStringDescriptor {
    #[inline]
    pub const fn lang_id(&self) -> UsbLangId {
        UsbLangId(self.wLangId.as_u16())
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbLangId(pub u16);

/// USB Interface Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbInterfaceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bInterfaceNumber: UsbInterfaceNumber,
    pub bAlternateSetting: UsbAlternateSettingNumber,
    pub bNumEndpoints: u8,
    pub bInterfaceClass: UsbBaseClass,
    pub bInterfaceSubClass: UsbSubClass,
    pub bInterfaceProtocol: UsbProtocolCode,
    pub iInterface: u8,
}

unsafe impl UsbDescriptor for UsbInterfaceDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Interface;
}

impl UsbInterfaceDescriptor {
    #[inline]
    pub const fn if_no(&self) -> UsbInterfaceNumber {
        self.bInterfaceNumber
    }

    #[inline]
    pub const fn alternate_setting(&self) -> UsbAlternateSettingNumber {
        self.bAlternateSetting
    }

    #[inline]
    pub const fn num_endpoints(&self) -> usize {
        self.bNumEndpoints as usize
    }

    #[inline]
    pub const fn interface_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iInterface)
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bInterfaceClass,
            self.bInterfaceSubClass,
            self.bInterfaceProtocol,
        )
    }
}

/// USB Endpoint Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbEndpointDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bEndpointAddress: u8,
    pub bmAttributes: u8,
    pub wMaxPacketSize: UsbWord,
    pub bInterval: u8,
}

unsafe impl UsbDescriptor for UsbEndpointDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Endpoint;
}

impl UsbEndpointDescriptor {
    #[inline]
    pub fn endpoint_address(&self) -> Option<UsbEndpointAddress> {
        NonZeroU8::new(self.bEndpointAddress).map(|v| UsbEndpointAddress(v))
    }

    #[inline]
    pub fn ep_type(&self) -> UsbEndpointType {
        UsbEndpointType::from_u8(self.bmAttributes)
    }

    #[inline]
    pub const fn max_packet_size(&self) -> UsbLength {
        UsbLength(self.wMaxPacketSize.as_u16())
    }

    #[inline]
    pub const fn interval(&self) -> u8 {
        self.bInterval
    }
}

/// USB Decive Qualifier Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceQualifierDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdUSB: UsbWord,
    pub bDeviceClass: UsbBaseClass,
    pub bDeviceSubClass: UsbSubClass,
    pub bDeviceProtocol: UsbProtocolCode,
    pub bMaxPacketSize0: u8,
    pub bNumConfigurations: u8,
    pub bReserved: u8,
}

unsafe impl UsbDescriptor for UsbDeviceQualifierDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::DeviceQualifier;
}

impl UsbDeviceQualifierDescriptor {
    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bDeviceClass,
            self.bDeviceSubClass,
            self.bDeviceProtocol,
        )
    }
}

/// Interface Association Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct InterfaceAssociationDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bFirstInterface: UsbInterfaceNumber,
    pub bInterfaceCount: u8,
    pub bFunctionClass: UsbBaseClass,
    pub bFunctionSubClass: UsbSubClass,
    pub bFunctionProtocol: UsbProtocolCode,
    pub iFunction: u8,
}

unsafe impl UsbDescriptor for InterfaceAssociationDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::InterfaceAssociation;
}

impl InterfaceAssociationDescriptor {
    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bFunctionClass,
            self.bFunctionSubClass,
            self.bFunctionProtocol,
        )
    }
}

/// BOS USB Binary Device Object Store Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbBinaryObjectStoreDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub wTotalLength: UsbWord,
    pub bNumDeviceCaps: u8,
}

unsafe impl UsbDescriptor for UsbBinaryObjectStoreDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::BinaryObjectStore;
}

impl UsbBinaryObjectStoreDescriptor {
    #[inline]
    pub const fn total_length(&self) -> UsbLength {
        self.wTotalLength.as_length()
    }

    #[inline]
    pub const fn num_children(&self) -> usize {
        self.bNumDeviceCaps as usize
    }
}

/// USB Superspeed USB Device Capability
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbSsDeviceCapability {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bDevCapabilityType: UsbDeviceCapabilityType,
    pub bmAttributes: u8,
    pub wSpeedSupported: UsbWord,
    pub bFunctionalitySupport: u8,
    pub bU1DevExitLat: u8,
    pub wU2DevExitLat: UsbWord,
}

unsafe impl UsbDeviceCapabilityDescriptor for UsbSsDeviceCapability {
    const CAPABILITY_TYPE: UsbDeviceCapabilityType = UsbDeviceCapabilityType::SuperspeedUsb;
}

impl UsbSsDeviceCapability {
    #[inline]
    pub const fn u1_dev_exit_lat(&self) -> usize {
        self.bU1DevExitLat as usize
    }

    #[inline]
    pub const fn u2_dev_exit_lat(&self) -> usize {
        self.wU2DevExitLat.as_u16() as usize
    }
}

/// USB Container Id capability
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbContainerIdCapability {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bDevCapabilityType: UsbDeviceCapabilityType,
    pub bReserved: u8,
    pub ContainerID: [u8; 16],
}

unsafe impl UsbDeviceCapabilityDescriptor for UsbContainerIdCapability {
    const CAPABILITY_TYPE: UsbDeviceCapabilityType = UsbDeviceCapabilityType::ContainerId;
}

impl UsbContainerIdCapability {
    #[inline]
    pub const fn uuid(&self) -> &[u8; 16] {
        &self.ContainerID
    }
}

/// USB HID Report Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
pub struct UsbHidReportDescriptor {
    pub bDescriptorType: UsbDescriptorType,
    pub wDescriptorLength: UsbWord,
}

/// USB HID Class Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct UsbHidClassDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdHID: UsbWord,
    pub bCountryCode: u8,
    pub bNumDescriptors: u8,
    pub bDescriptorType_: UsbDescriptorType,
    pub wDescriptorLength_: UsbWord,
}

unsafe impl UsbDescriptor for UsbHidClassDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::HidClass;
}

impl UsbHidClassDescriptor {
    #[inline]
    pub const fn num_descriptors(&self) -> usize {
        self.bNumDescriptors as usize
    }

    #[inline]
    pub fn first_child(&self) -> (UsbDescriptorType, u16) {
        (self.bDescriptorType_, self.wDescriptorLength_.as_u16())
    }

    #[inline]
    pub fn children<'a>(&'a self) -> impl Iterator<Item = (UsbDescriptorType, UsbLength)> + 'a {
        UsbHidClassDescriptorIter {
            base: self,
            index: 0,
        }
    }
}

struct UsbHidClassDescriptorIter<'a> {
    base: &'a UsbHidClassDescriptor,
    index: usize,
}

impl Iterator for UsbHidClassDescriptorIter<'_> {
    type Item = (UsbDescriptorType, UsbLength);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.index < self.base.num_descriptors() {
                type Struct = (UsbDescriptorType, UsbWord);
                let offset = self.index + 2;
                let p = (self.base as *const _ as *const Struct).add(offset);
                let (ty, len) = p.read();
                self.index += 1;
                Some((ty, len.as_length()))
            } else {
                None
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHubPortNumber(pub NonZeroU8);

/// USB2 Hub Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct Usb2HubDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: UsbWord,
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub DeviceRemovable: [u8; 1],
}

unsafe impl UsbDescriptor for Usb2HubDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Hub;
}

impl Usb2HubDescriptor {
    #[inline]
    pub const fn num_ports(&self) -> usize {
        self.bNbrPorts as usize
    }

    #[inline]
    pub fn ports(&self) -> impl Iterator<Item = UsbHubPortNumber> {
        (1..=self.num_ports())
            .map(|i| UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) }))
    }

    #[inline]
    pub const fn characteristics(&self) -> Usb2HubCharacterisrics {
        Usb2HubCharacterisrics(self.wHubCharacteristics.as_u16())
    }

    /// Time (in 2 ms intervals) from the time the power-on sequence begins on a port until power is good on that port. The USB System Software uses this value to determine how long to wait before accessing a powered-on port.
    #[inline]
    pub const fn power_on_to_power_good(&self) -> Duration {
        Duration::from_millis(self.bPwrOn2PwrGood as u64 * 2)
    }

    #[inline]
    pub const fn device_removable(&self) -> u8 {
        self.DeviceRemovable[0]
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Usb2HubCharacterisrics(pub u16);

impl Usb2HubCharacterisrics {
    #[inline]
    pub const fn is_compound_device(&self) -> bool {
        (self.0 & 0x0004) != 0
    }

    #[inline]
    pub const fn ttt(&self) -> usize {
        ((self.0 >> 5) & 3) as usize
    }

    #[inline]
    pub const fn supports_port_indicators(&self) -> bool {
        (self.0 & 0x0080) != 0
    }
}

/// USB3 Hub Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct Usb3HubDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: UsbWord,
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub bHubHdrDecLat: u8,
    pub wHubDelay: UsbWord,
    pub DeviceRemovable: UsbWord,
}

unsafe impl UsbDescriptor for Usb3HubDescriptor {
    const DESCRIPTOR_TYPE: UsbDescriptorType = UsbDescriptorType::Hub3;
}

impl Usb3HubDescriptor {
    #[inline]
    pub const fn num_ports(&self) -> usize {
        self.bNbrPorts as usize
    }

    #[inline]
    pub fn ports(&self) -> impl Iterator<Item = UsbHubPortNumber> {
        (1..=self.num_ports())
            .map(|i| UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) }))
    }

    #[inline]
    pub const fn characteristics(&self) -> Usb3HubCharacterisrics {
        Usb3HubCharacterisrics(self.wHubCharacteristics.as_u16())
    }

    /// Time (in 2 ms intervals) from the time the power-on sequence begins on a port until power is good on that port. The USB System Software uses this value to determine how long to wait before accessing a powered-on port.
    #[inline]
    pub const fn power_on_to_power_good(&self) -> Duration {
        Duration::from_millis(self.bPwrOn2PwrGood as u64 * 2)
    }

    #[inline]
    pub const fn device_removable(&self) -> u16 {
        self.DeviceRemovable.as_u16()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Usb3HubCharacterisrics(pub u16);

impl Usb3HubCharacterisrics {
    #[inline]
    pub const fn is_compound_device(&self) -> bool {
        (self.0 & 0x0004) != 0
    }
}

/// USB3 Route String
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct UsbRouteString(u32);

impl UsbRouteString {
    /// Empty route (root)
    pub const EMPTY: Self = Self(0);
    /// Since a valid Route String is 20 bits, a valid mask is 0xFFFFF.
    pub const VALID_MASK: u32 = 0xF_FFFF;
    /// Max depth is 5
    pub const MAX_DEPTH: usize = 5;

    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw & Self::VALID_MASK)
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn depth(&self) -> usize {
        match self.0 {
            0 => 0,
            0x0_0001..=0x0_000F => 1,
            0x0_0010..=0x0_00FF => 2,
            0x0_0100..=0x0_0FFF => 3,
            0x0_1000..=0x0_FFFF => 4,
            _ => 5,
        }
    }

    #[inline]
    pub const fn appending(&self, port: UsbHubPortNumber) -> Result<Self, Self> {
        let lpc = port.0.get() as u32;
        let lpc = if lpc < 15 { lpc } else { 15 };
        let raw = self.0;
        let depth = self.depth();
        if depth < Self::MAX_DEPTH {
            Ok(Self(raw | (lpc << (depth * 4))))
        } else {
            Err(*self)
        }
    }

    #[inline]
    pub const fn append(&mut self, port: UsbHubPortNumber) -> Result<(), ()> {
        match self.appending(port) {
            Ok(v) => {
                *self = v;
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbControlSetupData {
    pub bmRequestType: UsbControlRequestBitmap,
    pub bRequest: UsbControlRequest,
    pub wValue: u16,
    pub wIndex: u16,
    pub wLength: UsbLength,
}

impl UsbControlSetupData {
    #[inline]
    pub const fn request(
        request_type: UsbControlRequestBitmap,
        request: UsbControlRequest,
    ) -> Self {
        Self {
            bmRequestType: request_type,
            bRequest: request,
            wValue: 0,
            wIndex: 0,
            wLength: UsbLength(0),
        }
    }

    #[inline]
    pub const fn value(mut self, value: u16) -> Self {
        self.wValue = value;
        self
    }

    #[inline]
    pub const fn index(mut self, index: u16) -> Self {
        self.wIndex = index;
        self
    }

    #[inline]
    pub const fn index_if(self, if_no: UsbInterfaceNumber) -> Self {
        self.index(if_no.0 as u16)
    }

    #[inline]
    pub const fn length(mut self, length: UsbLength) -> Self {
        self.wLength = length;
        self
    }

    #[inline]
    pub const fn get_descriptor(
        request_type: UsbControlRequestBitmap,
        desc_type: UsbDescriptorType,
        index: u8,
        size: UsbLength,
    ) -> Self {
        Self::request(request_type, UsbControlRequest::GET_DESCRIPTOR)
            .value((desc_type as u16) << 8 | index as u16)
            .length(size)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlRequestBitmap(pub u8);

impl UsbControlRequestBitmap {
    /// Device to host standard request
    pub const GET_DEVICE: Self = Self(0x80);
    /// Host to device standard request
    pub const SET_DEVICE: Self = Self(0x00);

    /// Device to host interface specific request
    pub const GET_INTERFACE: Self = Self(0x81);
    /// Host to device interface specific request
    pub const SET_INTERFACE: Self = Self(0x01);

    /// Device to host class specific request
    pub const GET_CLASS: Self = Self(0xA0);
    /// Host to device class specific request
    pub const SET_CLASS: Self = Self(0x20);

    #[inline]
    pub const fn new(
        device_to_host: bool,
        request_type: UsbControlRequestType,
        target: UsbControlRequestTarget,
    ) -> Self {
        Self(((device_to_host as u8) << 7) | ((request_type as u8) << 5) | (target as u8))
    }

    #[inline]
    pub const fn is_device_to_host(&self) -> bool {
        (self.0 & 0x80) != 0
    }
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbControlRequestType {
    Standard = 0,
    Class,
    Vendor,
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbControlRequestTarget {
    Device = 0,
    Interface,
    Endpoint,
    Other,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlRequest(pub u8);

impl UsbControlRequest {
    pub const GET_STATUS: Self = Self(0);
    pub const CLEAR_FEATURE: Self = Self(1);
    pub const SET_FEATURE: Self = Self(3);
    // pub const SET_ADDRESS:Self = Self(5);
    pub const GET_DESCRIPTOR: Self = Self(6);
    pub const SET_DESCRIPTOR: Self = Self(7);
    pub const GET_CONFIGURATION: Self = Self(8);
    pub const SET_CONFIGURATION: Self = Self(9);
    pub const GET_INTERFACE: Self = Self(0x0A);
    pub const SET_INTERFACE: Self = Self(0x0B);
    // pub const SYNC_FRAME:Self = Self(0x0C);
    pub const SET_SEL: Self = Self(0x30);
    pub const SET_ISOCH_DELAY: Self = Self(0x31);
    pub const HID_GET_REPORT: Self = Self(1);
    pub const HID_SET_REPORT: Self = Self(9);
    pub const HID_SET_PROTOCOL: Self = Self(11);
    pub const SET_HUB_DEPTH: Self = Self(12);
}

#[repr(u16)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbDeviceFeatureSel {
    DEVICE_REMOTE_WAKEUP = 1,
}

/// Protocol Speed Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PSIV {
    /// USB1 FullSpeed 12Mbps
    FS = 1,
    /// USB1 LowSpeed 1.5Mbps
    LS = 2,
    /// USB2 HighSpeed 480Mbps
    HS = 3,
    /// USB3 SuperSpeed 5Gbps
    SS = 4,
    PSIV5,
    PSIV6,
    PSIV7,
    PSIV8,
    PSIV9,
    PSIV10,
    PSIV11,
    PSIV12,
    PSIV13,
    PSIV14,
    PSIV15,
}

impl PSIV {
    #[inline]
    pub const fn max_packet_size(&self) -> UsbLength {
        match self {
            PSIV::FS | PSIV::LS => UsbLength(8),
            PSIV::HS => UsbLength(64),
            _ => UsbLength(512),
        }
    }

    #[inline]
    pub const fn protocol_speed(&self) -> usize {
        match self {
            PSIV::FS => 12_000_000,
            PSIV::LS => 1_500_000,
            PSIV::HS => 480_000_000,
            PSIV::SS => 5_000_000_000,
            _ => 0,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Usb3LinkState {
    U0 = 0,
    U1,
    U2,
    U3,
    Disabled,
    RxDetect,
    Inactive,
    Polling,
    Recovery,
    HotReset,
    ComplianceMode,
    TestMode,
    Resume = 15,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Usb3ExitLatencyValues {
    pub u1sel: u8,
    pub u1pel: u8,
    pub u2sel: UsbWord,
    pub u2pel: UsbWord,
}

impl Usb3ExitLatencyValues {
    #[inline]
    pub const fn from_ss_dev_cap(ss_dev_cap: &UsbSsDeviceCapability) -> Self {
        Self {
            u1sel: ss_dev_cap.bU1DevExitLat,
            u1pel: ss_dev_cap.bU1DevExitLat,
            u2sel: ss_dev_cap.wU2DevExitLat,
            u2pel: ss_dev_cap.wU2DevExitLat,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbEndpointAddress(pub NonZeroU8);

impl UsbEndpointAddress {
    #[inline]
    pub const fn new(ep_no: u8, is_dir_in: bool) -> Option<Self> {
        match NonZeroU8::new((ep_no & 0x0F) | ((is_dir_in as u8) << 7)) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn is_dir_in(&self) -> bool {
        (self.0.get() & 0x80) != 0
    }

    #[inline]
    pub const fn ep_no(&self) -> usize {
        (self.0.get() & 0x0F) as usize
    }

    #[inline]
    pub const fn compact(&self) -> u8 {
        let c = self.0.get();
        (c & 0x0F) | ((c & 0x80) >> 3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbEndpointType {
    Control = 0,
    Isochronous,
    Bulk,
    Interrupt,
}

impl UsbEndpointType {
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v & 3 {
            0b00 => Self::Control,
            0b01 => Self::Isochronous,
            0b10 => Self::Bulk,
            0b11 => Self::Interrupt,
            _ => unreachable!(),
        }
    }
}
