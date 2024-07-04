use megstd::drawing::*;
use zune_jpeg::JpegDecoder;

pub struct ImageLoader;

impl ImageLoader {
    pub fn load(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let drivers = [
            Self::_from_png,
            Self::_from_jpeg,
            Self::_from_qoi,
            Self::_from_mpic,
        ];
        for driver in drivers {
            match driver(blob) {
                Err(DecodeError::NotSupported) => continue,
                Ok(v) => return Ok(v),
                Err(err) => return Err(err),
            }
        }
        Err(DecodeError::NotSupported)
    }

    #[inline]
    fn _from_png(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        png_decoder::decode(blob)
            .map_err(|err| match err {
                png_decoder::DecodeError::InvalidMagicBytes => DecodeError::NotSupported,
                _ => DecodeError::InvalidData,
            })
            .map(|(header, pixels)| {
                let size = Size::new(header.width, header.height);
                OwnedBitmap32::from_vec_rgba(pixels, size)
            })
    }

    #[inline]
    fn _from_jpeg(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let mut decoder = JpegDecoder::new(blob);
        decoder
            .decode_headers()
            .map_err(|_| DecodeError::NotSupported)?;
        let info = decoder.info().ok_or(DecodeError::InvalidData)?;
        let pixels = decoder.decode().map_err(|_| DecodeError::InvalidData)?;
        OwnedBitmap32::from_bytes_rgb(&pixels, Size::new(info.width as u32, info.height as u32))
            .ok_or(DecodeError::InvalidData)
    }

    #[inline]
    fn _from_qoi(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        rapid_qoi::Qoi::decode_alloc(blob)
            .map_err(|err| match err {
                rapid_qoi::DecodeError::NotEnoughData => DecodeError::OutOfMemory,
                rapid_qoi::DecodeError::InvalidMagic => DecodeError::NotSupported,
                _ => DecodeError::InvalidData,
            })
            .and_then(|(qoi, pixels)| {
                let size = Size::new(qoi.width, qoi.height);
                if qoi.colors.has_alpha() {
                    Some(OwnedBitmap32::from_vec_rgba(pixels, size))
                } else {
                    OwnedBitmap32::from_bytes_rgb(&pixels, size)
                }
                .ok_or(DecodeError::InvalidData)
            })
    }

    #[inline]
    fn _from_mpic(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let decoder = mpic::Decoder::<()>::new(blob).ok_or(DecodeError::NotSupported)?;
        let info = decoder.info();
        let pixels = decoder
            .decode_rgba()
            .map_err(|_| DecodeError::InvalidData)?;
        Ok(OwnedBitmap32::from_vec_rgba(
            pixels,
            Size::new(info.width(), info.height()),
        ))
    }
}

#[derive(Debug)]
pub enum DecodeError {
    General,
    OutOfMemory,
    NotSupported,
    InvalidParameter,
    InvalidData,
}
