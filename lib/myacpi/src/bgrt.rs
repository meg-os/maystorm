use super::*;

/// Boot Graphics Resource Table
#[repr(C, packed)]
#[allow(dead_code)]
pub struct Bgrt {
    hdr: AcpiHeader,
    version: u16,
    status: u8,
    image_type: ImageType,
    image_address: u64,
    offset_x: u32,
    offset_y: u32,
}

unsafe impl AcpiTable for Bgrt {
    const TABLE_ID: TableId = TableId::BGRT;
}

impl Bgrt {
    #[inline]
    pub fn image_type(&self) -> ImageType {
        self.image_type
    }

    #[inline]
    pub const fn orientation_offset(&self) -> usize {
        match (self.status & 0b110) >> 1 {
            0b00 => 0,
            0b01 => 90,
            0b10 => 180,
            0b11 => 270,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn bitmap(&self) -> *const u8 {
        self.image_address as usize as *const u8
    }

    #[inline]
    pub const fn offset(&self) -> (usize, usize) {
        (self.offset_x as usize, self.offset_y as usize)
    }
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageType {
    Bitmap = 0,
}
