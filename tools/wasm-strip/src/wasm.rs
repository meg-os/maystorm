//! WemAssembly mini library (expr)

use core::mem::transmute;
use core::str;

pub struct WasmMiniLoader {
    _phantom: (),
}

impl WasmMiniLoader {
    /// Minimal valid module size, Magic(4) + Version(4) + Empty sections(0) = 8
    const MINIMAL_MOD_SIZE: usize = 8;
    /// Magic number of WebAssembly Binary Format
    pub const MAGIC: [u8; 4] = *b"\0asm";
    /// Current Version
    pub const VER_CURRENT: [u8; 4] = *b"\x01\0\0\0";

    #[inline]
    #[cfg(target_endian = "little")]
    pub const fn file_header() -> [u8; Self::MINIMAL_MOD_SIZE] {
        unsafe { transmute([Self::MAGIC, Self::VER_CURRENT]) }
    }

    /// Identify the file format
    #[inline]
    pub fn identify(bytes: &[u8]) -> bool {
        bytes.len() >= Self::MINIMAL_MOD_SIZE
            && &bytes[0..4] == Self::MAGIC
            && &bytes[4..8] == Self::VER_CURRENT
    }

    pub fn load_sections(blob: &[u8]) -> Result<Vec<WasmSection>, WasmDecodeErrorType> {
        let magic = Self::file_header().len();
        let mut positions = Vec::new();
        let mut leb = Leb128Stream::from_slice(&blob[magic..]);
        while let Some(position) = leb.next_section()? {
            positions.push(position);
        }
        drop(leb);

        Ok(positions
            .iter()
            .map(|(section_type, start, length)| WasmSection {
                section_type: *section_type,
                file_position: *start,
                stream: Leb128Stream::from_slice(&blob[magic + start..magic + start + length]),
            })
            .collect())
    }
}

/// Stream encoded with LEB128
pub struct Leb128Stream<'a> {
    blob: &'a [u8],
    position: usize,
}

impl<'a> Leb128Stream<'a> {
    /// Instantiates from a slice
    #[inline]
    pub const fn from_slice(slice: &'a [u8]) -> Self {
        Self {
            blob: slice,
            position: 0,
        }
    }

    #[inline]
    pub fn cloned(&self) -> Self {
        Self {
            blob: self.blob,
            position: self.position,
        }
    }
}

#[allow(dead_code)]
impl Leb128Stream<'_> {
    /// Returns to the origin of the stream
    #[inline]
    pub fn reset(&mut self) {
        self.position = 0;
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.blob.len()
    }

    /// Gets current position of stream
    #[inline]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub fn set_position(&mut self, val: usize) {
        self.position = val;
    }

    /// Returns whether the end of the stream has been reached
    #[inline]
    pub const fn is_eof(&self) -> bool {
        self.position >= self.blob.len()
    }

    /// Reads one byte from a stream
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, WasmDecodeErrorType> {
        if self.is_eof() {
            return Err(WasmDecodeErrorType::UnexpectedEof);
        }
        let d = self.blob[self.position];
        self.position += 1;
        Ok(d)
    }

    /// Returns a slice of the specified number of bytes from the stream
    pub fn get_bytes(&mut self, size: usize) -> Result<&[u8], WasmDecodeErrorType> {
        let limit = self.blob.len();
        if self.position <= limit && size <= limit && self.position + size <= limit {
            let offset = self.position;
            self.position += size;
            Ok(&self.blob[offset..offset + size])
        } else {
            Err(WasmDecodeErrorType::UnexpectedEof)
        }
    }

    /// Reads multiple bytes from the stream
    #[inline]
    pub fn read_bytes(&mut self) -> Result<&[u8], WasmDecodeErrorType> {
        self.read_unsigned()
            .and_then(move |size| self.get_bytes(size as usize))
    }

    /// Reads an unsigned integer from a stream
    pub fn read_unsigned(&mut self) -> Result<u64, WasmDecodeErrorType> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        loop {
            if self.is_eof() {
                return Err(WasmDecodeErrorType::UnexpectedEof);
            }
            let d = self.blob[cursor];
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

    /// Reads a signed integer from a stream
    pub fn read_signed(&mut self) -> Result<i64, WasmDecodeErrorType> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        let signed = loop {
            if self.is_eof() {
                return Err(WasmDecodeErrorType::UnexpectedEof);
            }
            let d = self.blob[cursor];
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

    /// Reads the UTF-8 encoded string from the stream
    #[inline]
    pub fn get_string(&mut self) -> Result<&str, WasmDecodeErrorType> {
        self.read_bytes()
            .and_then(|v| str::from_utf8(v).map_err(|_| WasmDecodeErrorType::UnexpectedToken))
    }

    fn next_section(
        &mut self,
    ) -> Result<Option<(WasmSectionType, usize, usize)>, WasmDecodeErrorType> {
        let section_type = match self.read_byte().ok() {
            Some(v) => v,
            None => return Ok(None),
        };

        let length = self.read_unsigned()? as usize;
        let start = self.position();
        self.set_position(self.position + length);

        Ok(Some((section_type.into(), start, length)))
    }

    pub fn write_unsigned(vec: &mut Vec<u8>, value: u64) {
        let mut value = value;
        loop {
            let byte = value & 0x7F;
            value >>= 7;
            if value == 0 {
                vec.push(byte as u8);
                break;
            } else {
                vec.push(0x80 | byte as u8);
            }
        }
    }
}

/// WebAssembly section stream
pub struct WasmSection<'a> {
    section_type: WasmSectionType,
    file_position: usize,
    stream: Leb128Stream<'a>,
}

impl WasmSection<'_> {
    #[inline]
    pub const fn section_type(&self) -> WasmSectionType {
        self.section_type
    }

    #[inline]
    pub const fn file_position(&self) -> usize {
        self.file_position
    }

    #[inline]
    pub const fn stream_size(&self) -> usize {
        self.stream.len()
    }

    #[inline]
    pub fn custom_section_name(&self) -> Option<String> {
        if self.section_type != WasmSectionType::Custom {
            return None;
        }
        let mut blob = self.stream.cloned();
        blob.reset();
        blob.get_string().map(|v| v.to_string()).ok()
    }

    pub fn write_to_vec(&self, vec: &mut Vec<u8>) {
        vec.push(self.section_type() as u8);
        Leb128Stream::write_unsigned(vec, self.stream_size() as u64);
        vec.extend_from_slice(self.stream.blob);
    }
}

/// WebAssembly section types
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq)]
pub enum WasmSectionType {
    Custom = 0,
    Type,
    Import,
    Function,
    Table,
    Memory,
    Global,
    Export,
    Start,
    Element,
    Code,
    Data,
}

impl From<u8> for WasmSectionType {
    #[inline]
    fn from(v: u8) -> Self {
        match v {
            1 => WasmSectionType::Type,
            2 => WasmSectionType::Import,
            3 => WasmSectionType::Function,
            4 => WasmSectionType::Table,
            5 => WasmSectionType::Memory,
            6 => WasmSectionType::Global,
            7 => WasmSectionType::Export,
            8 => WasmSectionType::Start,
            9 => WasmSectionType::Element,
            10 => WasmSectionType::Code,
            11 => WasmSectionType::Data,
            _ => WasmSectionType::Custom,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WasmDecodeErrorType {
    UnexpectedEof,
    UnexpectedToken,
    InvalidParameter,
    InvalidBytecode,
    InvalidStackLevel,
    InvalidType,
    InvalidGlobal,
    InvalidLocal,
    OutOfStack,
    OutOfBranch,
    OutOfMemory,
    TypeMismatch,
    BlockMismatch,
    ElseWithoutIf,
    UnreachableTrap,
    NoMethod,
    NoModule,
    NotSupprted,
    BadExecutable,
    ExceededBytecode,
}
