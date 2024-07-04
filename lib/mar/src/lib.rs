//! MEG-OS Flattened Archive File
//!
//! # archive:
//! * header, tagged&lt;any&gt;, ..., tagged&lt;end&gt;
//!
//! # tagged &lt;tag&gt;:
//! * tag: u8
//! * payload: blob
//!
//! # blob:
//! * size: leb128
//! * payload: Array of u8
//!
//! # str: utf8 string
//! * size: leb128
//! * payload: Array of u8
//!
//! # xattr: extended file attributes (TBD)
//! * size: leb128
//! * payload: Array of TBD
//!
//! # end:
//! * tag: TAG_END(1)
//!
//! # namespace: sub directory
//! * tag: TAG_NAMESPACE(2)
//! * name: str
//! * xattr: xattr
//!
//! # file:
//! * tag: TAG_FILE(3)
//! * name: str
//! * xattr: xattr
//! * content: blob
//!
#![cfg_attr(not(test), no_std)]
extern crate alloc;
#[allow(unused_imports)]
use alloc::vec::Vec;
use core::mem::transmute;

pub const MAGIC: u32 = 0x0002beef;

pub const TAG_END: u8 = 0x01;
pub const TAG_NAMESPACE: u8 = 0x02;
pub const TAG_FILE: u8 = 0x03;

mod leb128;
pub use leb128::*;

#[repr(C)]
pub struct Header {
    magic: u32,
    _reserved: u32,
    offset: u32,
    size: u32,
}

impl Header {
    const SIZE_OF_HEADER: usize = 16;

    #[inline]
    pub const fn new() -> Self {
        Self {
            magic: MAGIC,
            _reserved: 0,
            offset: 0,
            size: 0,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }

    fn from_slice<'a>(slice: &'a [u8; Self::SIZE_OF_HEADER]) -> Result<&'a Self, ReadError> {
        let header: &Self = unsafe { transmute(slice) };
        header
            .is_valid()
            .then(|| header)
            .ok_or(ReadError::InvalidData)
    }

    #[inline]
    fn into_bytes(self) -> [u8; Self::SIZE_OF_HEADER] {
        unsafe { transmute(self) }
    }
}

pub struct ArchiveWriter {
    writer: Leb128Writer,
}

impl ArchiveWriter {
    #[inline]
    pub fn new() -> Self {
        Self {
            writer: Leb128Writer::new(),
        }
    }

    pub fn write(&mut self, value: Entry) -> Result<(), WriteError> {
        value._write_to(&mut self.writer)
    }

    pub fn finalize(mut self, additional: &[u8]) -> Result<Vec<u8>, WriteError> {
        self.write(Entry::End)?;

        let mut header = Header::new();
        header.offset = (Header::SIZE_OF_HEADER + additional.len())
            .try_into()
            .map_err(|_| WriteError::OutOfMemory)?;
        header.size = self
            .writer
            .len()
            .try_into()
            .map_err(|_| WriteError::OutOfMemory)?;

        let total_size = (header.offset as usize)
            .checked_add(header.size as usize)
            .ok_or(WriteError::OutOfMemory)?;
        let mut vec = Vec::new();
        vec.try_reserve(total_size)
            .map_err(|_| WriteError::OutOfMemory)?;

        vec.extend_from_slice(header.into_bytes().as_slice());
        vec.extend_from_slice(additional);
        vec.extend_from_slice(self.writer.as_slice());

        Ok(vec)
    }
}

#[non_exhaustive]
pub enum Entry<'a> {
    End,
    Namespace(&'a str, ExtendedAttributes<'a>),
    File(&'a str, ExtendedAttributes<'a>, &'a [u8]),
}

impl Entry<'_> {
    fn _write_to(&self, writer: &mut Leb128Writer) -> Result<(), WriteError> {
        match self {
            Entry::End => writer.write_tagged_payload(TAG_END, &[]),
            Entry::Namespace(name, xattr) => {
                let payload = {
                    let mut writer = Leb128Writer::new();
                    writer.write(*name)?;
                    writer.write(xattr)?;
                    writer.into_vec()
                };
                writer.write_tagged_payload(TAG_NAMESPACE, &payload)
            }
            Entry::File(name, xattr, payload) => {
                let leading = {
                    let mut writer = Leb128Writer::new();
                    writer.write(*name)?;
                    writer.write(xattr)?;
                    writer.write(payload.len())?;
                    writer.into_vec()
                };
                let total_size = leading.len() + payload.len();

                writer.write_byte(TAG_FILE)?;
                writer.write(total_size)?;
                writer.write_bytes(&leading)?;
                writer.write_bytes(&payload)
            }
        }
    }
}

pub struct ExtendedAttributes<'a>(&'a [u8]);

impl<'a> ExtendedAttributes<'a> {
    #[inline]
    pub fn empty() -> Self {
        Self(&[])
    }
}

impl WriteLeb128<&ExtendedAttributes<'_>> for Leb128Writer {
    #[inline]
    fn write(&mut self, value: &ExtendedAttributes) -> Result<(), WriteError> {
        self.write_blob(value.0)
    }
}

impl<'a, 'b> ReadLeb128<'a, ExtendedAttributes<'b>> for Leb128Reader<'b> {
    #[inline]
    fn read(&'a mut self) -> Result<ExtendedAttributes<'b>, ReadError> {
        self.read_blob().map(|v| ExtendedAttributes(v))
    }
}

pub struct ArchiveReader<'a> {
    reader: Leb128Reader<'a>,
}

impl<'a> ArchiveReader<'a> {
    pub fn from_slice(slice: &'a [u8]) -> Result<Self, ReadError> {
        let mut reader = Leb128Reader::from_slice(slice);
        let header = reader
            .read_bytes(Header::SIZE_OF_HEADER)
            .and_then(|v| v.try_into().map_err(|_| ReadError::UnexpectedEof))
            .and_then(|v| Header::from_slice(v))?;
        let offset: usize = header
            .offset
            .try_into()
            .map_err(|_| ReadError::OutOfBounds)?;
        let size: usize = header.size.try_into().map_err(|_| ReadError::OutOfBounds)?;
        let last = offset.checked_add(size).ok_or(ReadError::OutOfBounds)?;

        let slice = slice.get(offset..last).ok_or(ReadError::InvalidData)?;

        Ok(Self {
            reader: Leb128Reader::from_slice(slice),
        })
    }
}

impl ArchiveReader<'static> {
    #[inline]
    pub unsafe fn from_static(
        base: *const u8,
        len: usize,
    ) -> Result<ArchiveReader<'static>, ReadError> {
        let slice = unsafe { core::slice::from_raw_parts(base, len) };
        Self::from_slice(slice)
    }
}

impl<'a> ArchiveReader<'a> {
    pub fn reader_test(&mut self) -> &mut Leb128Reader<'a> {
        &mut self.reader
    }
}

impl<'a> Iterator for ArchiveReader<'a> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tag = self.reader.read_byte().unwrap();
        match tag {
            TAG_NAMESPACE => {
                let blob = self.reader.read_blob().ok()?;
                let mut reader = Leb128Reader::from_slice(blob);
                let name: &str = reader.read().ok()?;
                let xattr: ExtendedAttributes = reader.read().ok()?;
                Some(Entry::Namespace(name, xattr))
            }
            TAG_FILE => {
                let blob = self.reader.read_blob().ok()?;
                let mut reader = Leb128Reader::from_slice(blob);
                let name: &str = reader.read().ok()?;
                let xattr: ExtendedAttributes = reader.read().ok()?;
                let content = reader.read_blob().ok()?;
                Some(Entry::File(name, xattr, content))
            }
            TAG_END => {
                self.reader.read_blob().ok()?;
                Some(Entry::End)
            }
            // _ => panic!("UNKNOWN_TAG {tag:08x}"),
            _ => None,
        }
    }
}
