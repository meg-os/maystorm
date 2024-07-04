//! xHC Data Structures

use crate::drivers::usb::*;
use crate::*;
use core::mem::transmute;
use core::num::NonZeroU8;
use core::sync::atomic::*;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

/// xHCI Port Id
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortId(pub NonZeroU8);

impl From<UsbHubPortNumber> for PortId {
    #[inline]
    fn from(value: UsbHubPortNumber) -> Self {
        Self(value.0)
    }
}

/// xHCI Slot Id
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotId(pub NonZeroU8);

/// xHCI Device Context Index (Endpoint Id)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DCI(pub NonZeroU8);

impl DCI {
    pub const CONTROL: Self = Self(unsafe { NonZeroU8::new_unchecked(1) });

    #[inline]
    pub const fn ep_out(epno: u8) -> Option<Self> {
        if epno >= 1 && epno <= 15 {
            match NonZeroU8::new(epno * 2) {
                Some(v) => Some(Self(v)),
                None => None,
            }
        } else {
            None
        }
    }

    #[inline]
    pub const fn ep_in(epno: u8) -> Option<Self> {
        if epno >= 1 && epno <= 15 {
            match NonZeroU8::new(epno * 2 + 1) {
                Some(v) => Some(Self(v)),
                None => None,
            }
        } else {
            None
        }
    }

    #[inline]
    pub const fn is_control(&self) -> bool {
        self.0.get() == Self::CONTROL.0.get()
    }

    #[inline]
    pub const fn can_read(&self) -> bool {
        self.0.get() > 1 && (self.0.get() & 1) != 0
    }

    #[inline]
    pub const fn can_write(&self) -> bool {
        self.0.get() > 1 && (self.0.get() & 1) == 0
    }

    #[inline]
    pub const fn ep_no(&self) -> usize {
        (self.0.get() / 2) as usize
    }
}

impl From<UsbEndpointAddress> for DCI {
    #[inline]
    fn from(val: UsbEndpointAddress) -> Self {
        unsafe {
            Self(NonZeroU8::new_unchecked(
                (val.is_dir_in() as u8) | ((val.ep_no() as u8) << 1),
            ))
        }
    }
}

#[repr(transparent)]
pub struct CycleBit(AtomicBool);

impl CycleBit {
    #[inline]
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    #[inline]
    pub fn reset(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    #[inline]
    pub fn set(&self, val: bool) {
        self.0.store(val, Ordering::SeqCst);
    }

    #[inline]
    pub fn value(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn tr_value(&self) -> u64 {
        self.value() as u64
    }

    #[inline]
    pub fn trb_value(&self) -> u32 {
        self.value() as u32
    }

    #[inline]
    pub fn toggle(&self) {
        self.0.fetch_xor(true, Ordering::SeqCst);
    }
}

impl From<bool> for CycleBit {
    #[inline]
    fn from(val: bool) -> Self {
        Self(AtomicBool::new(val))
    }
}

impl PartialEq for CycleBit {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.load(Ordering::SeqCst) == other.0.load(Ordering::SeqCst)
    }
}

pub type TrbRawData = [AtomicU32; 4];

pub trait TrbBase {
    fn raw_data(&self) -> &TrbRawData;

    #[inline]
    fn has_known_type(&self) -> bool {
        self.trb_type()
            .map(|v| v != TrbType::RESERVED)
            .unwrap_or(false)
    }

    #[inline]
    fn cycle_bit(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x01) != 0
    }

    #[inline]
    fn trb_type(&self) -> Option<TrbType> {
        let val = (self.raw_data()[3].load(Ordering::SeqCst) >> 10) & 0x3F;
        FromPrimitive::from_u32(val)
    }

    fn copy<T>(&self, src: &T, cycle: &CycleBit)
    where
        T: TrbBase,
    {
        let dest = self.raw_data();
        let src = src.raw_data();
        for index in 0..3 {
            dest[index].store(src[index].load(Ordering::Acquire), Ordering::Release);
        }
        dest[3].store(
            src[3].load(Ordering::SeqCst) & 0xFFFF_FFFE | cycle.trb_value(),
            Ordering::SeqCst,
        );
    }

    fn copy_without_cycle<T>(&self, src: &T)
    where
        T: TrbBase,
    {
        let dest = self.raw_data();
        let src = src.raw_data();
        for index in 0..3 {
            dest[index].store(src[index].load(Ordering::Acquire), Ordering::Release);
        }
        dest[3].store(
            (src[3].load(Ordering::SeqCst) & 0xFFFF_FFFE)
                | (dest[3].load(Ordering::SeqCst) & 0x0000_0001),
            Ordering::SeqCst,
        );
    }

    fn raw_copy<T>(&self, src: &T)
    where
        T: TrbBase,
    {
        let dest = self.raw_data();
        let src = src.raw_data();
        for index in 0..4 {
            dest[index].store(src[index].load(Ordering::SeqCst), Ordering::SeqCst);
        }
    }

    #[inline]
    fn clone(&self) -> Trb
    where
        Self: Sized,
    {
        let result = Trb::empty();
        result.raw_copy(self);
        result
    }

    #[inline]
    fn as_trb(&self) -> &Trb {
        unsafe { transmute(self.raw_data()) }
    }

    #[inline]
    unsafe fn transmute<T: TrbBase>(&self) -> &T {
        transmute(self.as_trb())
    }

    fn as_event(&self) -> Option<TrbEvent> {
        match self.trb_type() {
            Some(TrbType::COMMAND_COMPLETION_EVENT) => {
                Some(TrbEvent::CommandCompletion(unsafe { self.transmute() }))
            }
            Some(TrbType::PORT_STATUS_CHANGE_EVENT) => {
                Some(TrbEvent::PortStatusChange(unsafe { self.transmute() }))
            }
            Some(TrbType::TRANSFER_EVENT) => Some(TrbEvent::Transfer(unsafe { self.transmute() })),
            Some(TrbType::DEVICE_NOTIFICATION_EVENT) => {
                Some(TrbEvent::DeviceNotification(unsafe { self.transmute() }))
            }
            _ => None,
        }
    }
}

/// xHCI Common Transfer Request Block
#[repr(transparent)]
pub struct Trb(TrbRawData);

impl Trb {
    #[inline]
    pub const fn new(trb_type: TrbType) -> Self {
        let slice = [0, 0, 0, (trb_type as u32) << 10];
        unsafe { transmute(slice) }
    }

    #[inline]
    pub const fn empty() -> Self {
        Self::new(TrbType::RESERVED)
    }

    #[inline]
    pub fn set_trb_type(&mut self, trb_type: TrbType) {
        let val =
            (self.raw_data()[3].load(Ordering::SeqCst) & 0xFFFF_03FF) | ((trb_type as u32) << 10);
        self.raw_data()[3].store(val, Ordering::SeqCst)
    }
}

impl TrbBase for Trb {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

pub trait TrbPtr: TrbBase {
    #[inline]
    fn ptr(&self) -> PhysicalAddress {
        let low = self.raw_data()[0].load(Ordering::SeqCst) as u64;
        let high = self.raw_data()[1].load(Ordering::SeqCst) as u64;
        PhysicalAddress::new(low | (high << 32))
    }

    #[inline]
    fn set_ptr(&self, val: PhysicalAddress) {
        let val = val.as_u64();
        let low = val as u32;
        let high = (val >> 32) as u32;
        self.raw_data()[0].store(low, Ordering::SeqCst);
        self.raw_data()[1].store(high, Ordering::SeqCst);
    }
}

pub trait TrbCC: TrbBase {
    #[inline]
    fn completion_code(&self) -> Option<TrbCompletionCode> {
        let val = (self.raw_data()[2].load(Ordering::SeqCst) >> 24) & 0xFF;
        FromPrimitive::from_u32(val)
    }

    #[inline]
    fn to_usb_error(&self) -> UsbError {
        match self.completion_code() {
            Some(err) => err.into(),
            None => UsbError::General,
        }
    }
}

pub trait TrbPortId: TrbBase {
    #[inline]
    fn port_id(&self) -> Option<PortId> {
        NonZeroU8::new((self.raw_data()[0].load(Ordering::SeqCst) >> 24) as u8).map(|v| PortId(v))
    }
}

pub trait TrbSlotId: TrbBase {
    #[inline]
    fn slot_id(&self) -> Option<SlotId> {
        NonZeroU8::new((self.raw_data()[3].load(Ordering::SeqCst) >> 24) as u8).map(|v| SlotId(v))
    }

    #[inline]
    fn set_slot_id(&self, slot_id: SlotId) {
        self.raw_data()[3].store(
            self.raw_data()[3].load(Ordering::SeqCst) & 0x00FF_FFFF
                | ((slot_id.0.get() as u32) << 24),
            Ordering::SeqCst,
        );
    }
}

pub trait TrbDci: TrbBase {
    #[inline]
    fn dci(&self) -> Option<DCI> {
        NonZeroU8::new(0x1F & (self.raw_data()[3].load(Ordering::SeqCst) >> 16) as u8)
            .map(|v| DCI(v))
    }

    #[inline]
    fn set_dci(&self, dci: DCI) {
        self.raw_data()[3].store(
            self.raw_data()[3].load(Ordering::SeqCst) & 0xFFE0_FFFF | ((dci.0.get() as u32) << 16),
            Ordering::SeqCst,
        );
    }
}

pub trait TrbXferLen: TrbBase {
    #[inline]
    fn xfer_len(&self) -> usize {
        (self.raw_data()[2].load(Ordering::SeqCst) & 0x0001_FFFF) as usize
    }

    #[inline]
    fn set_xfer_len(&self, xfer_len: usize) {
        self.raw_data()[2].store(
            self.raw_data()[2].load(Ordering::SeqCst) & 0xFFFE_0000
                | ((xfer_len & 0x0001_FFFF) as u32),
            Ordering::SeqCst,
        );
    }
}

/// Interrupt on Short Packet
pub trait TrbIsp: TrbBase {
    #[inline]
    fn isp(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0004) != 0
    }

    #[inline]
    fn set_isp(&self, value: bool) {
        let bit = 0x0000_0004;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Interrupt on Completion
pub trait TrbIoC: TrbBase {
    #[inline]
    fn ioc(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0020) != 0
    }

    #[inline]
    fn set_ioc(&self, value: bool) {
        let bit = 0x0000_0020;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Immediate Data
pub trait TrbIDt: TrbBase {
    #[inline]
    fn idt(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0040) != 0
    }

    #[inline]
    fn set_idt(&self, value: bool) {
        let bit = 0x0000_0040;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Direction is device to host
pub trait TrbDir: TrbBase {
    #[inline]
    fn dir(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0001_0000) != 0
    }

    #[inline]
    fn set_dir(&self, value: bool) {
        let bit = 0x0001_0000;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// TRB Types
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum TrbType {
    RESERVED = 0,
    NORMAL,
    SETUP,
    DATA,
    STATUS,
    ISOCH,
    LINK,
    EVENT_DATA,
    NOP,

    ENABLE_SLOT_COMMAND = 9,
    DISABLE_SLOT_COMMAND,
    ADDRESS_DEVICE_COMMAND,
    CONFIGURE_ENDPOINT_COMMAND,
    EVALUATE_CONTEXT_COMMAND,
    RESET_ENDPOINT_COMMAND,

    NOP_COMMAND = 23,

    TRANSFER_EVENT = 32,
    COMMAND_COMPLETION_EVENT = 33,
    PORT_STATUS_CHANGE_EVENT = 34,
    BANDWIDTH_REQUEST_EVENT = 35,
    DOORBELL_EVENT = 36,
    HOST_CONTROLLER_EVENT = 37,
    DEVICE_NOTIFICATION_EVENT = 38,
    MFINDEX_WRAP_EVENT = 39,
}

impl TrbType {
    #[inline]
    pub fn is_event(&self) -> bool {
        *self >= Self::TRANSFER_EVENT && *self <= Self::MFINDEX_WRAP_EVENT
    }

    #[inline]
    pub fn is_command(&self) -> bool {
        *self == Self::LINK || (*self >= Self::ENABLE_SLOT_COMMAND && *self < Self::TRANSFER_EVENT)
    }

    #[inline]
    pub fn is_transfer(&self) -> bool {
        *self >= Self::NORMAL && *self <= Self::NOP
    }
}

/// TRB Completion Codes
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum TrbCompletionCode {
    INVALID = 0,
    SUCCESS,
    DATA_BUFFER_ERROR,
    BABBLE_DETECTED,
    USB_TRANSACTION_ERROR,
    TRB_ERROR,
    STALL,
    RESOURCE_ERROR,
    BANDWIDTH_ERROR,
    NO_SLOTS_AVAILABLE,
    INVALID_STREAM_TYPE,
    SLOT_NOT_ENABLED,
    ENDPOINT_NOT_ENABLED,
    SHORT_PACKET,
    RING_UNDERRUN,
    RING_OVERRUN,
    VF_EVENT_RING_FULL,
    PARAMETER_ERROR,
    BANDWIDTH_OVERRUN,
    CONTEXT_STATE_ERROR,
    NO_PING_RESPONSE,
    EVENT_RING_FULL,
    IMCOMPATIBLE_DEVICE,
    MISSED_SERVICE,
    COMMAND_RING_STOPPED,
    COMMAND_ABORTED,
    STOPPED,
    STOPPED_LENGTH_INVALID,
    STOPPED_SHORT_PACKET,
    MAX_EXIT_LATENCY_TOO_LARGE,
    // reserved = 30,
    ISOCH_BUFFER_OVERRUN = 31,
    EVENT_LOST,
    UNDEFINED_ERROR,
    INVALID_STREAM_ID,
    SECONDARY_BANDWIDTH,
    SPLIT_TRANSACTION,
}

impl From<TrbCompletionCode> for UsbError {
    #[inline]
    fn from(value: TrbCompletionCode) -> Self {
        match value {
            TrbCompletionCode::INVALID => UsbError::InvalidParameter,
            TrbCompletionCode::USB_TRANSACTION_ERROR => UsbError::UsbTransactionError,
            TrbCompletionCode::SHORT_PACKET => UsbError::ShortPacket,
            _ => UsbError::ControllerError(value as usize),
        }
    }
}

/// TRB for LINK
pub struct TrbLink(TrbRawData);

impl TrbLink {
    #[inline]
    pub fn new(ptr: PhysicalAddress, toggle_cycle: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::LINK)) };
        result.set_ptr(ptr);
        if toggle_cycle {
            result.raw_data()[3].fetch_or(0x02, Ordering::Release);
        }
        result
    }
}

impl TrbBase for TrbLink {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbLink {}

/// TRB for NOP
pub struct TrbNop(TrbRawData);

impl TrbNop {
    #[inline]
    pub fn new() -> Self {
        unsafe { transmute(Trb::new(TrbType::NOP)) }
    }
}

impl TrbBase for TrbNop {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

/// TRB for NORMAL
pub struct TrbNormal(TrbRawData);

impl TrbNormal {
    pub fn new(ptr: PhysicalAddress, xfer_len: usize, ioc: bool, isp: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::NORMAL)) };
        result.set_ptr(ptr);
        result.set_xfer_len(xfer_len);
        result.set_ioc(ioc);
        result.set_isp(isp);
        result
    }
}

impl TrbBase for TrbNormal {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbNormal {}

impl TrbXferLen for TrbNormal {}

impl TrbIsp for TrbNormal {}

impl TrbIoC for TrbNormal {}

impl TrbIDt for TrbNormal {}

/// TRB for SETUP
pub struct TrbSetupStage(TrbRawData);

impl TrbSetupStage {
    pub fn new(trt: TrbTranfserType, setup: UsbControlSetupData) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::SETUP)) };
        unsafe {
            (&result as *const Self as *mut u32).copy_from(&setup as *const _ as *const u32, 2);
        }
        result.set_xfer_len(8);
        result.set_idt(true);
        result.raw_data()[3].fetch_or((trt as u32) << 16, Ordering::SeqCst);
        result
    }

    #[inline]
    pub fn setup_data(&self) -> &UsbControlSetupData {
        unsafe { &*(self as *const _ as *const UsbControlSetupData) }
    }
}

impl TrbBase for TrbSetupStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbXferLen for TrbSetupStage {}

impl TrbIDt for TrbSetupStage {}

/// TRB for DATA
pub struct TrbDataStage(TrbRawData);

impl TrbDataStage {
    pub fn new(ptr: PhysicalAddress, xfer_len: usize, dir: bool, isp: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::DATA)) };
        result.set_ptr(ptr);
        result.set_xfer_len(xfer_len);
        result.set_dir(dir);
        result.set_isp(isp);
        result
    }
}

impl TrbBase for TrbDataStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbDataStage {}

impl TrbXferLen for TrbDataStage {}

impl TrbIsp for TrbDataStage {}

impl TrbIoC for TrbDataStage {}

impl TrbDir for TrbDataStage {}

/// TRB for STATUS
pub struct TrbStatusStage(TrbRawData);

impl TrbStatusStage {
    pub fn new(dir: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::STATUS)) };
        result.set_dir(dir);
        result.set_ioc(true);
        result
    }
}

impl TrbBase for TrbStatusStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbIoC for TrbStatusStage {}

impl TrbDir for TrbStatusStage {}

pub enum TrbEvent<'a> {
    CommandCompletion(&'a TrbCommandCompletionEvent),
    PortStatusChange(&'a TrbPortStatusChangeEvent),
    Transfer(&'a TrbTransferEvent),
    DeviceNotification(&'a TrbDeviceNotificationEvent),
}

impl TrbEvent<'_> {
    #[inline]
    pub fn completion_code(&self) -> Option<TrbCompletionCode> {
        match self {
            TrbEvent::CommandCompletion(ref v) => v.completion_code(),
            TrbEvent::PortStatusChange(ref v) => v.completion_code(),
            TrbEvent::Transfer(ref v) => v.completion_code(),
            TrbEvent::DeviceNotification(ref v) => v.completion_code(),
        }
    }
}

/// TRB for COMMAND_COMPLETION_EVENT
pub struct TrbCommandCompletionEvent(TrbRawData);

impl TrbCommandCompletionEvent {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy(self);
        result
    }
}

impl TrbBase for TrbCommandCompletionEvent {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbCommandCompletionEvent {}

impl TrbSlotId for TrbCommandCompletionEvent {}

impl TrbPtr for TrbCommandCompletionEvent {}

/// TRB for PORT_STATUS_CHANGE_EVENT
pub struct TrbPortStatusChangeEvent(TrbRawData);

impl TrbBase for TrbPortStatusChangeEvent {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbPortStatusChangeEvent {}

impl TrbPortId for TrbPortStatusChangeEvent {}

/// TRB for TRANSFER_EVENT
pub struct TrbTransferEvent(TrbRawData);

impl TrbTransferEvent {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy(self);
        result
    }

    #[inline]
    pub fn transfer_length(&self) -> usize {
        (self.raw_data()[2].load(Ordering::SeqCst) & 0x00FF_FFFF) as usize
    }

    #[inline]
    pub fn is_event_data(&self) -> bool {
        (self.raw_data()[3].load(Ordering::Relaxed) & 4) != 0
    }
}

impl TrbBase for TrbTransferEvent {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbTransferEvent {}

impl TrbSlotId for TrbTransferEvent {}

impl TrbPtr for TrbTransferEvent {}

impl TrbDci for TrbTransferEvent {}

/// TRB for DEVICE_NOTIFICATION_EVENT
pub struct TrbDeviceNotificationEvent(TrbRawData);

impl TrbDeviceNotificationEvent {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy(self);
        result
    }
}

impl TrbBase for TrbDeviceNotificationEvent {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbDeviceNotificationEvent {}

impl TrbSlotId for TrbDeviceNotificationEvent {}

impl TrbPtr for TrbDeviceNotificationEvent {}

/// TRB for ADDRESS_DEVICE_COMMAND
pub struct TrbAddressDeviceCommand(TrbRawData);

impl TrbAddressDeviceCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: PhysicalAddress) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::ADDRESS_DEVICE_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbBase for TrbAddressDeviceCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbAddressDeviceCommand {}

impl TrbPtr for TrbAddressDeviceCommand {}

/// TRB for CONFIGURE_ENDPOINT_COMMAND
pub struct TrbConfigureEndpointCommand(TrbRawData);

impl TrbConfigureEndpointCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: PhysicalAddress) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::CONFIGURE_ENDPOINT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbBase for TrbConfigureEndpointCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbConfigureEndpointCommand {}

impl TrbPtr for TrbConfigureEndpointCommand {}

/// TRB for EVALUATE_CONTEXT_COMMAND
pub struct TrbEvaluateContextCommand(TrbRawData);

impl TrbEvaluateContextCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: PhysicalAddress) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::EVALUATE_CONTEXT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbBase for TrbEvaluateContextCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbEvaluateContextCommand {}

impl TrbPtr for TrbEvaluateContextCommand {}

/// TRB for RESET_ENDPOINT_COMMAND
pub struct TrbResetEndpointCommand(TrbRawData);

impl TrbResetEndpointCommand {
    #[inline]
    pub fn new(slot_id: SlotId, dci: DCI) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::RESET_ENDPOINT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_dci(dci);
        result
    }
}

impl TrbBase for TrbResetEndpointCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbResetEndpointCommand {}

impl TrbDci for TrbResetEndpointCommand {}

/// xHC Event Ring Segment Table Entry
#[allow(dead_code)]
#[repr(C)]
pub struct EventRingSegmentTableEntry {
    base: u64,
    size: u16,
    _rsrv: [u8; 6],
}

impl EventRingSegmentTableEntry {
    #[inline]
    pub const fn new(base: PhysicalAddress, size: u16) -> Self {
        Self {
            base: base.as_u64(),
            size,
            _rsrv: [0; 6],
        }
    }

    #[inline]
    pub const fn base(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.base)
    }

    #[inline]
    pub const fn size(&self) -> u16 {
        self.size
    }
}

#[repr(C)]
pub struct InputControlContext {
    drop: u32,
    add: u32,
    _reserved: [u32; 5],
    _word7: u32,
}

impl InputControlContext {
    #[inline]
    pub fn clear(&mut self) {
        unsafe {
            (self as *mut Self).write_bytes(0, 1);
        }
    }

    #[inline]
    pub fn set_add(&mut self, val: u32) {
        self.add = val;
    }

    #[inline]
    pub fn set_drop(&mut self, val: u32) {
        self.drop = val;
    }
}

#[repr(C)]
pub struct SlotContext {
    data: [u32; 8],
}

impl SlotContext {
    #[inline]
    pub const fn route_string(&self) -> UsbRouteString {
        UsbRouteString::from_raw(self.data[0])
    }

    #[inline]
    pub fn set_route_string(&mut self, route: UsbRouteString) {
        self.data[0] = self.data[0] & 0xFFF0_0000 | route.as_u32();
    }

    #[inline]
    pub fn set_speed(&mut self, speed: PSIV) {
        self.data[0] = self.data[0] & 0xFF0F_FFFF | ((speed as u32) << 20)
    }

    #[inline]
    pub fn speed(&self) -> PSIV {
        unsafe { transmute((self.data[0] >> 20) as u8 & 15) }
    }

    #[inline]
    pub const fn set_is_mtt(&mut self, value: bool) {
        let bit = 1 << 25;
        if value {
            self.data[0] |= bit;
        } else {
            self.data[0] &= !bit;
        }
    }

    #[inline]
    pub const fn set_is_hub(&mut self, value: bool) {
        let bit = 1 << 26;
        if value {
            self.data[0] |= bit;
        } else {
            self.data[0] &= !bit;
        }
    }

    #[inline]
    pub const fn context_entries(&self) -> usize {
        (self.data[0] as usize) >> 27
    }

    #[inline]
    pub fn set_context_entries(&mut self, val: usize) {
        self.data[0] = self.data[0] & 0x07FF_FFFF | ((val as u32) << 27);
    }

    #[inline]
    pub fn max_exit_latency(&mut self) -> usize {
        (self.data[1] & 0xFFFF) as usize
    }

    #[inline]
    pub fn set_max_exit_latency(&mut self, val: usize) {
        self.data[1] = self.data[1] & 0xFFFF_0000 | ((val & 0xFFFF) as u32);
    }

    #[inline]
    pub fn root_hub_port(&self) -> Option<PortId> {
        NonZeroU8::new((self.data[1] >> 16) as u8).map(|v| PortId(v))
    }

    #[inline]
    pub fn set_root_hub_port(&mut self, port_id: PortId) {
        self.data[1] = self.data[1] & 0xFF00_FFFF | ((port_id.0.get() as u32) << 16)
    }

    #[inline]
    pub fn num_ports(&mut self) -> usize {
        (self.data[1] >> 24) as usize
    }

    #[inline]
    pub fn set_num_ports(&mut self, num: usize) {
        self.data[1] = self.data[1] & 0x00FF_FFFF | ((num as u32) << 24);
    }

    #[inline]
    pub fn set_parent_hub_slot_id(&mut self, slot_id: SlotId) {
        self.data[2] = self.data[2] & 0xFFFF_FF00 | (slot_id.0.get() as u32);
    }

    #[inline]
    pub fn set_parent_port_id(&mut self, port_id: UsbHubPortNumber) {
        self.data[2] = self.data[2] & 0xFFFF_00FF | ((port_id.0.get() as u32) << 8);
    }

    #[inline]
    pub fn set_ttt(&mut self, ttt: usize) {
        self.data[2] = self.data[2] & 0xFFFC_FFFF | ((ttt as u32) << 16);
    }
}

pub struct EndpointContext {
    data: [u32; 8],
}

impl EndpointContext {
    #[inline]
    pub fn set_interval(&mut self, value: u8) {
        self.data[0] = self.data[0] & 0xFF00_FFFF | ((value as u32) << 16);
    }

    #[inline]
    pub fn set_ep_type(&mut self, ep_type: EpType) {
        self.data[1] = self.data[1] & 0xFFFF_FFC7 | ((ep_type as u32) << 3)
    }

    #[inline]
    pub fn set_max_packet_size(&mut self, max_packet_size: UsbLength) {
        self.data[1] = self.data[1] & 0x0000_FFFF | ((max_packet_size.0 as u32) << 16);
    }

    #[inline]
    pub fn set_max_burst_size(&mut self, max_burst_size: usize) {
        self.data[1] = self.data[1] & 0xFFFF_00FF | ((max_burst_size as u32) << 8);
    }

    #[inline]
    pub fn set_average_trb_len(&mut self, average_trb_len: usize) {
        self.data[4] = self.data[4] & 0xFFFF_0000 | (average_trb_len as u32);
    }

    #[inline]
    pub fn set_error_count(&mut self, cerr: usize) {
        self.data[1] = self.data[1] & 0xFFFF_FFF9 | (cerr as u32 * 2);
    }

    #[inline]
    pub fn set_trdp(&mut self, dp: PhysicalAddress) {
        let dp = dp.as_u64();
        self.data[2] = dp as u32;
        self.data[3] = (dp >> 32) as u32;
    }

    #[inline]
    pub const fn trdp(&self) -> u64 {
        (self.data[2] as u64) + (self.data[3] as u64) * 0x10000
    }
}

/// Endpoint Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EpType {
    Invalid = 0,
    IsochOut,
    BulkOut,
    InterruptOut,
    Control,
    IsochIn,
    BulkIn,
    InterruptIn,
}

impl EpType {
    #[inline]
    pub fn from_usb_ep_type(usb_ep_type: UsbEndpointType, dir: bool) -> Self {
        match (dir, usb_ep_type) {
            (false, UsbEndpointType::Control) => Self::Invalid,
            (false, UsbEndpointType::Isochronous) => Self::IsochOut,
            (false, UsbEndpointType::Bulk) => Self::BulkOut,
            (false, UsbEndpointType::Interrupt) => Self::InterruptOut,
            (true, UsbEndpointType::Control) => Self::Control,
            (true, UsbEndpointType::Isochronous) => Self::IsochIn,
            (true, UsbEndpointType::Bulk) => Self::BulkIn,
            (true, UsbEndpointType::Interrupt) => Self::InterruptIn,
        }
    }

    #[inline]
    pub const fn is_control(&self) -> bool {
        match self {
            EpType::Control => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_isochronous(&self) -> bool {
        match self {
            EpType::IsochOut | EpType::IsochIn => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_bulk(&self) -> bool {
        match self {
            EpType::BulkOut | EpType::BulkIn => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_interrupt(&self) -> bool {
        match self {
            EpType::InterruptOut | EpType::InterruptIn => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrbTranfserType {
    NoData = 0,
    ControlOut = 2,
    ControlIn = 3,
}

#[repr(C)]
pub struct XhciSupportedProtocolCapability(pub *const u32);

impl XhciSupportedProtocolCapability {
    pub const NAME_USB: [u8; 4] = *b"USB ";

    #[inline]
    pub fn rev_minor(&self) -> u8 {
        let data = unsafe { self.0.read_volatile() };
        (data >> 16) as u8
    }

    #[inline]
    pub fn rev_major(&self) -> u8 {
        let data = unsafe { self.0.read_volatile() };
        (data >> 24) as u8
    }

    #[inline]
    pub fn name(&self) -> [u8; 4] {
        unsafe { transmute(self.0.add(1).read_volatile()) }
    }

    #[inline]
    pub fn compatible_port_offset(&self) -> u8 {
        let data = unsafe { self.0.add(2).read_volatile() };
        (data & 0xFF) as u8
    }

    #[inline]
    pub fn compatible_port_count(&self) -> u8 {
        let data = unsafe { self.0.add(2).read_volatile() };
        ((data >> 8) & 0xFF) as u8
    }
}
