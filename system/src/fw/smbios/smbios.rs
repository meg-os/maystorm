// System Management BIOS

use crate::*;
use core::ffi::c_void;
use core::mem::transmute;
use core::ptr::addr_of;
use core::slice;
use core::str;

/// System Management BIOS Entry Point
pub struct SmBios {
    base: usize,
    n_structures: usize,
}

impl SmBios {
    #[inline]
    pub unsafe fn init(entry: PhysicalAddress) -> Box<Self> {
        let ep: &SmBiosEntryV1 = transmute(entry.as_usize());
        let base = PhysicalAddress::new(ep.base as u64).direct_map::<c_void>() as usize;
        let n_structures = ep.n_structures as usize;
        Box::new(Self { base, n_structures })
    }

    /// Returns the system manufacturer name, if available
    #[inline]
    pub fn manufacturer_name(&self) -> Option<String> {
        self.find(HeaderType::SYSTEM_INFO)
            .and_then(|h| {
                let slice = h.as_slice();
                h.string(slice[4] as usize)
            })
            .map(|v| v.to_string())
    }

    /// Returns the system model name, if available
    #[inline]
    pub fn model_name(&self) -> Option<String> {
        self.find(HeaderType::SYSTEM_INFO)
            .and_then(|h| {
                let slice = h.as_slice();
                h.string(slice[5] as usize)
            })
            .map(|v| v.to_string())
    }

    /// Returns an iterator that iterates through the SMBIOS structure
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &'static SmBiosHeader> {
        SmBiosStructIterator {
            base: self.base,
            offset: 0,
            index: 0,
            limit: self.n_structures,
        }
    }

    /// Find the first structure matching the specified header type.
    #[inline]
    pub fn find(&self, header_type: HeaderType) -> Option<&'static SmBiosHeader> {
        self.iter().find(|v| v.header_type() == header_type)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeaderType(pub u8);

impl HeaderType {
    pub const BIOS_INFO: Self = Self(0);
    pub const SYSTEM_INFO: Self = Self(1);
    pub const BASEBOARD_INFO: Self = Self(2);
    pub const SYSTEM_ENCLOSURE: Self = Self(3);
    pub const PROCESSOR_INFO: Self = Self(4);
    pub const MEMORY_CONTROLLER_INFO: Self = Self(5);
    pub const MEMORY_MODULE_INFO: Self = Self(6);
    pub const CACHE_INFO: Self = Self(7);
    pub const PORT_CONNECTOR_INFO: Self = Self(8);
    pub const SYSTEM_SLOTS: Self = Self(9);
    pub const ONBOARD_DEVICE_INFO: Self = Self(10);
    pub const OEM_STRINGS: Self = Self(11);
    pub const SYSTEM_CONFIGURATION_OPTIONS: Self = Self(12);
    pub const BIOS_LANGUAGE_INFO: Self = Self(13);
    pub const GROUP_ASSOCIATIONS: Self = Self(14);
    pub const SYSTEM_EVENT_LOG: Self = Self(15);
    pub const PHYSICAL_MEMORY_ARRAY: Self = Self(16);
    pub const MEMORY_DEVICE: Self = Self(17);
    pub const _32BIT_MEMORY_ERROR_INFO: Self = Self(18);
    pub const MEMORY_ARRAY_MAPPED_ADDRESS: Self = Self(19);
    pub const MEMORY_DEVICE_MAPPED_ADDRESS: Self = Self(20);
    pub const BUILT_IN_POINTING_DEVICE: Self = Self(21);
    pub const PORTABLE_BATTERY: Self = Self(22);
    pub const SYSTEM_RESET: Self = Self(23);
    pub const HARDWARE_SECURITY: Self = Self(24);
    pub const SYSTEM_POWER_CONTROLS: Self = Self(25);
    pub const VOLTAGE_PROBE: Self = Self(26);
    pub const COOLING_DEVICE: Self = Self(27);
    pub const TEMPERATURE_PROBE: Self = Self(28);
    pub const ELECTRICAL_CURRENT_PROBE: Self = Self(29);
    pub const OUT_OF_BAND_REMOTE_ACCESS: Self = Self(30);
    pub const BOOT_INTEGRITY_SERVICE: Self = Self(31);
    pub const SYSTEM_BOOT_INFO: Self = Self(32);
    pub const _64BIT_MEMORY_ERROR_INFO: Self = Self(33);
    pub const MANAGEMENT_DEVICE: Self = Self(34);
    pub const MANAGEMENT_DEVICE_COMPONENT: Self = Self(35);
    pub const MANAGEMENT_DEVICE_THRESHOLD_DATA: Self = Self(36);
    pub const MEMORY_CHANNEL: Self = Self(37);
    pub const IPMI_DEVICE_INFO: Self = Self(38);
    pub const SYSTEM_POWER_SUPPLY: Self = Self(39);
    pub const ADDITIONAL_INFO: Self = Self(40);
    pub const ONBOARD_DEVICES_EXTENDED_INFO: Self = Self(41);
    pub const MANAGEMENT_CONTROLLER_HOST_INTERFACE: Self = Self(42);
    pub const TPM_DEVICE: Self = Self(43);
    pub const PROCESSOR_ADDITIONAL_INFO: Self = Self(44);
}

#[repr(C)]
#[allow(dead_code)]
pub struct SmBiosEntryV1 {
    /// b"_SM_"
    anchor: [u8; 4],
    checksum: u8,
    len: u8,
    ver_major: u8,
    ver_minor: u8,
    max_struct: u16,
    revision: u8,
    formatted: [u8; 5],
    /// b"_DMI_"
    anchor2: [u8; 5],
    checksum2: u8,
    len2: u8,
    base: u32,
    n_structures: u16,
    rev: u8,
}

// impl SmBiosEntry {
//     fn is_valid(&self) -> bool {
//         (self.anchor == *b"_SM_") && (self.anchor2 == *b"_DMI_")
//     }
// }

struct SmBiosStructIterator {
    base: usize,
    offset: usize,
    index: usize,
    limit: usize,
}

impl Iterator for SmBiosStructIterator {
    type Item = &'static SmBiosHeader;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.limit {
            return None;
        }
        unsafe {
            let p = (self.base + self.offset) as *const SmBiosHeader;
            let r = &*p;
            self.offset += r.struct_size();
            self.index += 1;
            Some(r)
        }
    }
}

/// Common definition of SmBios's structures
#[repr(C)]
pub struct SmBiosHeader {
    header_type: HeaderType,
    size: u8,
    handle: u16,
}

impl SmBiosHeader {
    /// Some products return meaningless strings.
    pub const DEFAULT_STRING: &'static str = "Default string";
    /// Some products return meaningless strings.
    pub const TO_BE_FILLED_BY_OEM: &'static str = "To be filled by O.E.M.";

    #[inline]
    pub const fn header_type(&self) -> HeaderType {
        self.header_type
    }

    #[inline]
    pub const fn header_size(&self) -> usize {
        self.size as usize
    }

    #[inline]
    pub fn handle(&self) -> u16 {
        unsafe { addr_of!(self.handle).read_unaligned() }
    }

    #[inline]
    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        let data = self as *const _ as *const u8;
        let len = self.header_size();
        unsafe { slice::from_raw_parts(data, len) }
    }

    #[inline]
    fn strings(&self) -> SmBiosStrings {
        let base = self as *const _ as usize + self.header_size();
        SmBiosStrings { base, offset: 0 }
    }

    #[inline]
    pub fn string<'a>(&'a self, index: usize) -> Option<&'a str> {
        if index > 0 {
            self.strings().nth(index - 1).and_then(|v| match v {
                Self::DEFAULT_STRING | Self::TO_BE_FILLED_BY_OEM => None,
                _ => Some(v),
            })
        } else {
            None
        }
    }

    #[inline]
    pub fn struct_size(&self) -> usize {
        let mut iter = self.strings();
        while iter.next().is_some() {}
        if iter.offset > 0 {
            // There is a NULL after some strings
            self.header_size() + iter.offset + 1
        } else {
            // There is no strings and a double NULL
            self.header_size() + 2
        }
    }
}

struct SmBiosStrings {
    base: usize,
    offset: usize,
}

impl Iterator for SmBiosStrings {
    type Item = &'static str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ptr = (self.base + self.offset) as *const u8;
            let len = strlen(ptr);
            if len > 0 {
                self.offset += len + 1;
                Some(str::from_utf8(slice::from_raw_parts(ptr, len)).unwrap_or("?"))
            } else {
                None
            }
        }
    }
}

#[inline]
unsafe fn strlen(p: *const u8) -> usize {
    let mut count = 0;
    loop {
        if p.add(count).read_volatile() == 0 {
            break count;
        } else {
            count += 1;
        }
    }
}
