//! Universally Unique Identifier (RFC 4122)
#![cfg_attr(not(test), no_std)]

use core::{fmt::*, mem::transmute};
pub use uuid_identify::*;

/// Universally Unique Identifier (RFC 4122)
#[repr(transparent)]
#[derive(Copy, Clone, Eq)]
pub struct Uuid([u8; 16]);

impl Uuid {
    pub const NULL: Self = Self::null();

    #[inline]
    pub const fn from_parts(a: u32, b: u16, c: u16, d: u16, e: [u8; 6]) -> Self {
        Self([
            ((a >> 24) & 0xFF) as u8,
            ((a >> 16) & 0xFF) as u8,
            ((a >> 8) & 0xFF) as u8,
            (a & 0xFF) as u8,
            ((b >> 8) & 0xFF) as u8,
            (b & 0xFF) as u8,
            ((c >> 8) & 0xFF) as u8,
            (c & 0xFF) as u8,
            ((d >> 8) & 0xFF) as u8,
            (d & 0xFF) as u8,
            e[0],
            e[1],
            e[2],
            e[3],
            e[4],
            e[5],
        ])
    }

    #[inline]
    pub const fn from_raw(data: [u8; 16]) -> Self {
        Self(data)
    }

    #[inline]
    pub const fn from_slice(slice: &[u8; 16]) -> Self {
        Self(*slice)
    }

    #[inline]
    pub const fn a(&self) -> u32 {
        ((self.0[0] as u32) << 24)
            + ((self.0[1] as u32) << 16)
            + ((self.0[2] as u32) << 8)
            + (self.0[3] as u32)
    }

    #[inline]
    pub const fn b(&self) -> u16 {
        ((self.0[4] as u16) << 8) + (self.0[5] as u16)
    }

    #[inline]
    pub const fn c(&self) -> u16 {
        ((self.0[6] as u16) << 8) + (self.0[7] as u16)
    }

    #[inline]
    pub const fn d(&self) -> u16 {
        ((self.0[8] as u16) << 8) + (self.0[9] as u16)
    }

    #[inline]
    pub fn e(&self) -> &[u8] {
        &self.0[10..]
    }

    #[inline]
    pub fn e_u48(&self) -> u64 {
        self.e().iter().fold(0, |acc, v| (acc << 8) + (*v as u64))
    }

    #[inline]
    pub const fn null() -> Self {
        Self([0; 16])
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.eq(&Self::NULL)
    }

    #[inline]
    pub const fn into_raw(self) -> [u8; 16] {
        self.0
    }

    #[inline]
    pub const fn as_slice(&self) -> &[u8; 16] {
        &self.0
    }

    #[inline]
    pub const unsafe fn as_u128(&self) -> &u128 {
        transmute(self)
    }

    #[inline]
    pub fn version(&self) -> Option<UuidVersion> {
        unsafe { transmute(self.0[6] >> 4) }
    }
}

impl PartialEq for Uuid {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        unsafe { *self.as_u128() == *other.as_u128() }
    }
}

impl PartialOrd for Uuid {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for Uuid {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl Debug for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.a(),
            self.b(),
            self.c(),
            self.d(),
            self.e_u48(),
        )
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UuidVersion {
    V1 = 1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
    _V9,
    _V10,
    _V11,
    _V12,
    _V13,
    _V14,
    _V15,
}

pub unsafe trait Identify {
    const UUID: Uuid;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid1() {
        let uuid1_raw = Uuid::from_raw([
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ]);
        let uuid1 = Uuid::from_parts(
            0x1234_5678,
            0x9ABC,
            0xDEF0,
            0xFEDC,
            [0xBA, 0x98, 0x76, 0x54, 0x32, 0x10],
        );
        let uuid2_raw = Uuid::from_raw([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]);
        let uuid2 = Uuid::from_parts(
            0x0011_2233,
            0x4455,
            0x6677,
            0x8899,
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
        );

        assert_eq!(uuid1, uuid1_raw);
        assert_eq!(uuid2, uuid2_raw);
        assert_ne!(uuid1, uuid2);

        assert_eq!(uuid1.a(), 0x1234_5678);
        assert_eq!(uuid1.b(), 0x9ABC);
        assert_eq!(uuid1.c(), 0xDEF0);
        assert_eq!(uuid1.d(), 0xFEDC);
        assert_eq!(uuid1.e_u48(), 0xBA98_7654_3210);

        assert_eq!(uuid2.a(), 0x0011_2233);
        assert_eq!(uuid2.b(), 0x4455);
        assert_eq!(uuid2.c(), 0x6677);
        assert_eq!(uuid2.d(), 0x8899);
        assert_eq!(uuid2.e_u48(), 0xAABB_CCDD_EEFF);
    }

    #[test]
    fn identify() {
        #[identify("12345678-9abc-def0-fedc-ba9876543210")]
        struct Foo;

        let uuid1_foo = Uuid::from_raw([
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ]);

        assert_eq!(Foo::UUID, uuid1_foo);
    }
}
