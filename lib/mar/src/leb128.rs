//! Little Endian Base 128
#[allow(unused_imports)]
use alloc::vec::Vec;
use core::mem::size_of_val;
use core::str;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WriteError {
    OutOfMemory,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadError {
    InvalidData,
    UnexpectedEof,
    OutOfBounds,
}

pub struct Leb128Writer {
    inner: Vec<u8>,
}

impl Leb128Writer {
    #[inline]
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        self.inner
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        let additional: usize = bytes.len();
        if self.inner.capacity() - self.inner.len() < additional {
            self.inner
                .try_reserve(additional)
                .map_err(|_| WriteError::OutOfMemory)?;
        }

        self.inner.extend_from_slice(bytes);

        Ok(())
    }

    pub fn write_unsigned(&mut self, value: u64) -> Result<(), WriteError> {
        let bits = (size_of_val(&value) * 8) - value.leading_zeros() as usize;

        if bits <= 7 {
            let bytes = [value as u8];
            self.write_bytes(&bytes)
        } else if bits <= 14 {
            let bytes = [(value | 0x80) as u8, (value >> 7) as u8];
            self.write_bytes(&bytes)
        } else if bits <= 21 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                (value >> 14) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 28 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                (value >> 21) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 35 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                (value >> 28) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 42 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                (value >> 35) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 49 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                (value >> 42) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 56 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                (value >> 49) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits <= 63 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                ((value >> 49) | 0x80) as u8,
                (value >> 56) as u8,
            ];
            self.write_bytes(&bytes)
        } else {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                ((value >> 49) | 0x80) as u8,
                ((value >> 56) | 0x80) as u8,
                (value >> 63) as u8,
            ];
            self.write_bytes(&bytes)
        }
    }

    pub fn write_signed(&mut self, value: i64) -> Result<(), WriteError> {
        let bits = (size_of_val(&value) * 8)
            - if value < 0 {
                value.leading_ones() as usize
            } else {
                value.leading_zeros() as usize
            };

        if bits < 7 {
            let bytes = [(value & 0x7F) as u8];
            self.write_bytes(&bytes)
        } else if bits < 14 {
            let bytes = [(value | 0x80) as u8, ((value >> 7) & 0x7F) as u8];
            self.write_bytes(&bytes)
        } else if bits < 21 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 28 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 35 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 42 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 49 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 56 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                ((value >> 49) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else if bits < 63 {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                ((value >> 49) | 0x80) as u8,
                ((value >> 56) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        } else {
            let bytes = [
                (value | 0x80) as u8,
                ((value >> 7) | 0x80) as u8,
                ((value >> 14) | 0x80) as u8,
                ((value >> 21) | 0x80) as u8,
                ((value >> 28) | 0x80) as u8,
                ((value >> 35) | 0x80) as u8,
                ((value >> 42) | 0x80) as u8,
                ((value >> 49) | 0x80) as u8,
                ((value >> 56) | 0x80) as u8,
                ((value >> 63) & 0x7F) as u8,
            ];
            self.write_bytes(&bytes)
        }
    }

    #[inline]
    pub fn write_byte(&mut self, byte: u8) -> Result<(), WriteError> {
        self.write_bytes(&[byte])
    }

    /// blob:
    /// size: leb
    /// payload: array(u8)
    pub fn write_blob(&mut self, payload: &[u8]) -> Result<(), WriteError> {
        self.write(payload.len())?;
        self.write_bytes(payload)
    }

    /// tagged:
    /// tag: u8
    /// payload: blob
    pub fn write_tagged_payload(&mut self, tag: u8, payload: &[u8]) -> Result<(), WriteError> {
        self.write_byte(tag)?;
        self.write_blob(payload)
    }
}

pub struct Leb128Reader<'a> {
    slice: &'a [u8],
    position: usize,
}

impl<'a> Leb128Reader<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [u8]) -> Self {
        Self { slice, position: 0 }
    }

    #[inline]
    pub fn cloned(&self) -> Self {
        Self {
            slice: self.slice,
            position: self.position,
        }
    }

    pub fn read_bytes<'b>(&'b mut self, size: usize) -> Result<&'a [u8], ReadError> {
        self.slice
            .get(self.position..self.position + size)
            .map(|v| {
                self.position += size;
                v
            })
            .ok_or(ReadError::UnexpectedEof)
    }

    #[inline]
    pub fn read_blob<'b>(&'b mut self) -> Result<&'a [u8], ReadError> {
        self.read_unsigned()
            .and_then(move |size| self.read_bytes(size as usize))
    }
}

impl Leb128Reader<'_> {
    #[inline]
    pub fn reset(&mut self) {
        self.position = 0;
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.slice.len()
    }

    #[inline]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub fn set_position(&mut self, val: usize) {
        self.position = val;
    }

    #[inline]
    pub const fn is_eof(&self) -> bool {
        self.position >= self.slice.len()
    }

    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, ReadError> {
        self.slice
            .get(self.position)
            .map(|v| {
                self.position += 1;
                *v
            })
            .ok_or(ReadError::UnexpectedEof)
    }

    pub fn read_unsigned(&mut self) -> Result<u64, ReadError> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        loop {
            let d = match self.slice.get(cursor) {
                Some(v) => *v,
                None => return Err(ReadError::UnexpectedEof),
            };
            cursor += 1;

            value |= (d as u64 & 0x7F) << scale;
            scale += 7;
            if (d & 0x80) == 0 {
                break;
            }
        }
        self.position = cursor;
        Ok(value)
    }

    pub fn read_signed(&mut self) -> Result<i64, ReadError> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        let signed = loop {
            let d = match self.slice.get(cursor) {
                Some(v) => *v,
                None => return Err(ReadError::UnexpectedEof),
            };
            cursor += 1;

            value |= (d as u64 & 0x7F) << scale;
            let signed = (d & 0x40) != 0;
            if (d & 0x80) == 0 {
                break signed;
            }
            scale += 7;
        };
        self.position = cursor;
        if signed {
            Ok((value | 0xFFFF_FFFF_FFFF_FFC0 << scale) as i64)
        } else {
            Ok(value as i64)
        }
    }
}

pub trait ReadLeb128<'a, T> {
    fn read(&'a mut self) -> Result<T, ReadError>;
}

pub trait WriteLeb128<T> {
    fn write(&mut self, value: T) -> Result<(), WriteError>;
}

impl<'a, 'b> ReadLeb128<'a, &'b str> for Leb128Reader<'b> {
    #[inline]
    fn read(&'a mut self) -> Result<&'b str, ReadError> {
        self.read_blob()
            .and_then(|v| str::from_utf8(v).map_err(|_| ReadError::InvalidData))
    }
}

impl WriteLeb128<&str> for Leb128Writer {
    #[inline]
    fn write(&mut self, value: &str) -> Result<(), WriteError> {
        self.write(value.len())?;
        self.write_bytes(value.as_bytes())
    }
}

macro_rules! leb128_serialize_u {
    ($type:ident) => {
        impl<'a> ReadLeb128<'a, $type> for Leb128Reader<'_> {
            #[inline]
            fn read(&'a mut self) -> Result<$type, ReadError> {
                self.read_unsigned()
                    .and_then(|v| v.try_into().map_err(|_| ReadError::OutOfBounds))
            }
        }

        impl WriteLeb128<$type> for Leb128Writer {
            #[inline]
            fn write(&mut self, value: $type) -> Result<(), WriteError> {
                self.write_unsigned(value as u64)
            }
        }
    };
}

macro_rules! leb128_serialize_s {
    ($type:ident) => {
        impl<'a> ReadLeb128<'a, $type> for Leb128Reader<'_> {
            #[inline]
            fn read(&'a mut self) -> Result<$type, ReadError> {
                self.read_signed()
                    .and_then(|v| v.try_into().map_err(|_| ReadError::OutOfBounds))
            }
        }

        impl WriteLeb128<$type> for Leb128Writer {
            #[inline]
            fn write(&mut self, value: $type) -> Result<(), WriteError> {
                self.write_signed(value as i64)
            }
        }
    };
}

leb128_serialize_u!(u8);
leb128_serialize_u!(u16);
leb128_serialize_u!(u32);
leb128_serialize_u!(u64);
leb128_serialize_u!(usize);

leb128_serialize_s!(i8);
leb128_serialize_s!(i16);
leb128_serialize_s!(i32);
leb128_serialize_s!(i64);
leb128_serialize_s!(isize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leb128_reader() {
        let data = [
            0x7F, 0xFF, 0x00, 0xEF, 0xFD, 0xB6, 0xF5, 0x0D, 0xEF, 0xFD, 0xB6, 0xF5, 0x7D,
        ];
        let mut stream = Leb128Reader::from_slice(&data);

        stream.reset();
        assert_eq!(stream.position(), 0);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 0xdeadbeef);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 0x7deadbeef);

        stream.reset();
        assert_eq!(stream.position(), 0);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, -1);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, 0xdeadbeef);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, -559038737);
    }

    #[test]
    fn leb128_writer() {
        let mut writer = Leb128Writer::new();

        writer.clear();
        assert_eq!(writer.len(), 0);
        writer.write(0u32).unwrap();
        assert_eq!(writer.as_slice(), &[0]);

        writer.clear();
        assert_eq!(writer.len(), 0);
        writer.write(0i32).unwrap();
        assert_eq!(writer.as_slice(), &[0]);

        for i in 0..64 {
            let value1 = 1u64 << i;
            let mut writer = Leb128Writer::new();
            writer.write(value1).unwrap();

            let byte_cnt = (i + 7) / 7;
            assert_eq!(writer.as_slice().len(), byte_cnt);

            assert_ne!(*writer.as_slice().last().unwrap(), 0);

            let mut reader = Leb128Reader::from_slice(writer.as_slice());
            let test1 = reader.read().unwrap();
            assert_eq!(value1, test1);
        }

        writer.clear();
        writer.write(127u32).unwrap();
        assert_eq!(writer.as_slice(), &[0x7F]);

        writer.clear();
        writer.write(128u32).unwrap();
        assert_eq!(writer.as_slice(), &[0x80, 0x01]);

        writer.clear();
        writer.write(0xdeadbeefu32).unwrap();
        assert_eq!(writer.as_slice(), &[0xEF, 0xFD, 0xB6, 0xF5, 0x0D]);

        writer.clear();
        writer.write(0x7deadbeefu64).unwrap();
        assert_eq!(writer.as_slice(), &[0xEF, 0xFD, 0xB6, 0xF5, 0x7D]);

        writer.clear();
        writer.write(127i32).unwrap();
        assert_eq!(writer.as_slice(), &[0xFF, 0x00]);

        writer.clear();
        writer.write(63i32).unwrap();
        assert_eq!(writer.as_slice(), &[0x3F]);

        writer.clear();
        writer.write(64i32).unwrap();
        assert_eq!(writer.as_slice(), &[0xC0, 0x00]);

        writer.clear();
        writer.write(-1i32).unwrap();
        assert_eq!(writer.as_slice(), &[0x7F]);

        writer.clear();
        writer.write(-64i32).unwrap();
        assert_eq!(writer.as_slice(), &[0x40]);

        writer.clear();
        writer.write(0xdeadbeefi64).unwrap();
        assert_eq!(writer.as_slice(), &[0xEF, 0xFD, 0xB6, 0xF5, 0x0D]);

        writer.clear();
        writer.write(-559038737i64).unwrap();
        assert_eq!(writer.as_slice(), &[0xEF, 0xFD, 0xB6, 0xF5, 0x7D]);
    }

    #[test]
    fn leb128_read_write() {
        for i in 0..64 {
            let value1u = 1u64 << i;
            let value2u = value1u - 1;
            let value3u = !value2u;

            let value1i = value1u as i64;
            let value2i = value2u as i64;
            let value3i = value3u as i64;

            let value5 = value2u & 0x5555_5555_5555_5555;
            let value6 = value2u & 0x1234_5678_9ABC_DEF0;
            let value7 = value2u & 0xDEAD_BEEF_F00D_BAAD;

            let mut writer = Leb128Writer::new();
            writer.write(value1i).unwrap();
            writer.write(value1u).unwrap();
            writer.write(value2i).unwrap();
            writer.write(value2u).unwrap();
            writer.write(value3i).unwrap();
            writer.write(value3u).unwrap();
            writer.write(value5).unwrap();
            writer.write(value6).unwrap();
            writer.write(value7).unwrap();
            let mut reader = Leb128Reader::from_slice(writer.as_slice());

            let test1i = reader.read().unwrap();
            assert_eq!(value1i, test1i);
            let test1u = reader.read().unwrap();
            assert_eq!(value1u, test1u);
            let test2i = reader.read().unwrap();
            assert_eq!(value2i, test2i);
            let test2u = reader.read().unwrap();
            assert_eq!(value2u, test2u);
            let test3i = reader.read().unwrap();
            assert_eq!(value3i, test3i);
            let test3u = reader.read().unwrap();
            assert_eq!(value3u, test3u);

            let test5 = reader.read().unwrap();
            assert_eq!(value5, test5);
            let test6 = reader.read().unwrap();
            assert_eq!(value6, test6);
            let test7 = reader.read().unwrap();
            assert_eq!(value7, test7);

            assert!(reader.is_eof());

            assert_eq!(reader.read_byte().unwrap_err(), ReadError::UnexpectedEof);
        }
    }
}
