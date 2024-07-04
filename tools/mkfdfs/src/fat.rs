// File Allocation Table Filesystem

use bitflags::*;
use core::mem::transmute;

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct DosBpb {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors_count: u16,
    pub n_fats: u8,
    pub root_entries_count: u16,
    pub total_sectors: u16,
    pub media_descriptor: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub n_heads: u16,
}

#[repr(C, packed)]
pub struct DosExtendedBpb {
    pub bpb: DosBpb,
    pub hidden_sectors_count: u32,
    pub total_sectors32: u32,
    pub physical_drive_number: u8,
    pub flags: u8,
    pub extended_boot_sign: u8,
    pub volume_serial_number: u32,
    pub volume_label: [u8; 11],
    pub filesystem: [u8; 8],
}

impl DosBpb {
    #[inline]
    pub const fn new(
        bytes_per_sector: u16,
        sectors_per_cluster: u8,
        reserved_sectors_count: u16,
        n_fats: u8,
        root_entries_count: u16,
        total_sectors: u16,
        media_descriptor: u8,
        sectors_per_fat: u16,
        sectors_per_track: u16,
        n_heads: u16,
    ) -> Self {
        Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors_count,
            n_fats,
            root_entries_count,
            total_sectors,
            media_descriptor,
            sectors_per_fat,
            sectors_per_track,
            n_heads,
        }
    }
}

impl DosExtendedBpb {
    pub const EXTENDED_BOOT_SIGN: u8 = 0x29;

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.extended_boot_sign == Self::EXTENDED_BOOT_SIGN
    }
}

impl Default for DosExtendedBpb {
    #[inline]
    fn default() -> Self {
        Self {
            bpb: DosBpb::default(),
            hidden_sectors_count: 0,
            total_sectors32: 0,
            physical_drive_number: 0,
            flags: 0,
            extended_boot_sign: Self::EXTENDED_BOOT_SIGN,
            volume_serial_number: 0,
            volume_label: *b"NO NAME    ",
            filesystem: *b"FAT12   ",
        }
    }
}

#[repr(C, packed)]
pub struct BootSector {
    pub jumps: [u8; 3],
    pub oem_name: [u8; 8],
    pub ebpb: DosExtendedBpb,
    pub boot_code: [u8; 0x1C0],
    pub boot_signature: [u8; 2],
}

impl BootSector {
    pub const PREFERRED_SIZE: usize = 512;
    pub const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];

    #[inline]
    pub fn from_bytes(bytes: [u8; Self::PREFERRED_SIZE]) -> Self {
        unsafe { transmute(bytes) }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8; Self::PREFERRED_SIZE] {
        unsafe { transmute(self) }
    }
}

impl Default for BootSector {
    #[inline]
    fn default() -> Self {
        Self {
            jumps: [0xEB, 0xFE, 0x90],
            oem_name: [0; 8],
            ebpb: DosExtendedBpb::default(),
            boot_code: [0; 0x1C0],
            boot_signature: Self::BOOT_SIGNATURE,
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct DosDirEnt {
    pub name: [u8; 11],
    pub attr: DosAttributes,
    pub nt_reserved: u8,
    pub ctime_ms: u8,
    pub ctime: DosFileTimeStamp,
    pub atime: DosFileDate,
    pub cluster_hi: u16,
    pub mtime: DosFileTimeStamp,
    pub first_cluster: u16,
    pub file_size: u32,
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct DosAttributes: u8 {
        const READONLY  = 0b0000_0001;
        const HIDDEN    = 0b0000_0010;
        const SYSTEM    = 0b0000_0100;
        const LABEL     = 0b0000_1000;
        const SUBDIR    = 0b0001_0000;
        const ARCHIVE   = 0b0010_0000;

        const LFN_ENTRY = Self::READONLY.bits() | Self::HIDDEN.bits() | Self::SYSTEM.bits() | Self::LABEL.bits();
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DosFileTime(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DosFileDate(pub u16);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DosFileTimeStamp {
    pub time: DosFileTime,
    pub date: DosFileDate,
}

impl DosFileTime {
    pub const EMPTY: Self = Self(0);
}

impl DosFileDate {
    pub const EMPTY: Self = Self(0);
}

impl DosFileTimeStamp {
    pub const EMPTY: Self = Self {
        time: DosFileTime::EMPTY,
        date: DosFileDate::EMPTY,
    };
}

impl DosDirEnt {
    #[inline]
    pub const fn new() -> Self {
        Self {
            name: [0x20; 11],
            attr: DosAttributes::empty(),
            nt_reserved: 0,
            ctime_ms: 0,
            ctime: DosFileTimeStamp::EMPTY,
            atime: DosFileDate::EMPTY,
            mtime: DosFileTimeStamp::EMPTY,
            first_cluster: 0,
            cluster_hi: 0,
            file_size: 0,
        }
    }

    pub fn volume_label(label: &str) -> Result<Self, ConvertError> {
        let mut result = Self::new();
        result.attr = DosAttributes::LABEL;

        let mut label = label.chars();
        for i in 0..11 {
            let c = match label.next() {
                Some(c) => c,
                None => break,
            };
            let c = match Self::validate_volname_char(c) {
                Some(c) => c,
                None => return Err(ConvertError::InvalidChar),
            };
            result.name[i] = c;
        }
        Ok(result)
    }

    pub fn file_entry(name: &str) -> Result<Self, ConvertError> {
        let mut result = Self::new();
        result.attr = DosAttributes::ARCHIVE;

        let mut has_ext = true;
        let mut has_to_truncate = true;
        let mut name_has_upper = false;
        let mut name_has_lower = false;
        let mut ext_has_upper = false;
        let mut ext_has_lower = false;
        let mut chars = name.chars();

        for i in 0..8 {
            let c = match chars.next() {
                Some('.') => {
                    has_to_truncate = false;
                    break;
                }
                Some(c) => c,
                None => {
                    has_ext = false;
                    break;
                }
            };
            name_has_upper |= c.is_uppercase();
            name_has_lower |= c.is_lowercase();

            if let Some(c) = Self::validate_shortname_char(c) {
                result.name[i] = c;
            } else {
                return Err(ConvertError::InvalidChar);
            }
        }

        if has_to_truncate {
            loop {
                match chars.next() {
                    None => {
                        has_ext = false;
                        break;
                    }
                    Some('.') => break,
                    _ => (),
                }
            }
        }

        if has_ext {
            for i in 8..11 {
                let c = match chars.next() {
                    Some(c) => c,
                    None => break,
                };

                ext_has_upper |= c.is_uppercase();
                ext_has_lower |= c.is_lowercase();

                if let Some(c) = Self::validate_shortname_char(c) {
                    result.name[i] = c;
                } else {
                    return Err(ConvertError::InvalidChar);
                }
            }
        }

        result.nt_reserved = if name_has_lower & !name_has_upper {
            0x08
        } else {
            0
        } | if ext_has_lower & !ext_has_upper {
            0x10
        } else {
            0
        };

        if result.name[0] != 0x20 {
            Ok(result)
        } else {
            Err(ConvertError::Empty)
        }
    }

    fn validate_volname_char(c: char) -> Option<u8> {
        let c = c as u8;
        match c {
            0x20
            | 0x21
            | 0x23..=0x29
            | 0x2D
            | 0x30..=0x39
            | 0x41..=0x5A
            | 0x5E
            | 0x5F
            | 0x7B
            | 0x7D
            | 0x7E => Some(c),
            0x61..=0x7A => Some(c - 0x20),
            _ => None,
        }
    }

    fn validate_shortname_char(c: char) -> Option<u8> {
        let c = c as u8;
        match c {
            0x21
            | 0x23..=0x29
            | 0x2D
            | 0x30..=0x39
            | 0x41..=0x5A
            | 0x5E
            | 0x5F
            | 0x7B
            | 0x7D
            | 0x7E => Some(c),
            0x61..=0x7A => Some(c - 0x20),
            _ => None,
        }
    }
}

impl Default for DosDirEnt {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    Empty,
    InvalidChar,
}
