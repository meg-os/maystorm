use crate::sync::atomic::AtomicWrapper;
use core::cell::UnsafeCell;
use megstd::drawing::{rotation::Rotation, *};

pub trait Screen<T>: Image
where
    T: Image<ColorType = Self::ColorType>,
{
    fn blt(&self, src: &T, origin: Point, rect: Rect);

    fn fill_rect(&self, rect: Rect, color: Self::ColorType);

    fn draw_glyph(&self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType);

    /// Returns the native screen size
    fn native_size(&self) -> Size;

    /// Returns the physical size of the screen, if available.
    fn physical_size(&self) -> Option<Size> {
        None
    }

    /// Returns the number of pixels per inch.
    fn pixels_per_inch(&self) -> usize {
        96
    }

    /// Returns the screen rotation status.
    fn rotation(&self) -> Rotation {
        Default::default()
    }

    /// Changes the rotation state of the screen.
    fn set_rotation(&self, value: Rotation) -> Result<Rotation, Rotation> {
        let _ = value;
        Err(self.rotation())
    }

    /// Rotate the screen one level, if possible.
    fn rotate(&self) -> Result<Rotation, Rotation> {
        self.set_rotation(self.rotation().succ())
    }

    /// Returns the screen orientation status.
    fn orientation(&self) -> ScreenOrientation {
        let dims = self.size();
        if dims.width() < dims.height() {
            ScreenOrientation::Portrait
        } else {
            ScreenOrientation::Landscape
        }
    }

    /// Changes the screen orientation state.
    fn set_orientation(
        &self,
        value: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        let _ = value;
        Err(self.orientation())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenOrientation {
    Portrait,
    Landscape,
    // LandscapeLeft,
    // LandscapeRight,
    // PortraitUpsideDown,
}

impl ScreenOrientation {
    #[inline]
    pub const fn is_portrait(&self) -> bool {
        matches!(self, Self::Portrait)
    }

    #[inline]
    pub const fn is_landscape(&self) -> bool {
        matches!(self, Self::Landscape)
    }
}

pub struct BitmapScreen<'a> {
    fb: UnsafeCell<BitmapRefMut32<'a>>,
    native_size: Size,
    rotation: AtomicWrapper<Rotation>,
}

impl<'a> BitmapScreen<'a> {
    #[inline]
    pub fn new(bitmap: BitmapRefMut32<'a>) -> Self {
        Self {
            native_size: bitmap.size(),
            fb: UnsafeCell::new(bitmap),
            rotation: AtomicWrapper::default(),
        }
    }

    #[inline]
    fn bitmap(&self) -> &'a mut BitmapRefMut32<'a> {
        unsafe { &mut *self.fb.get() }
    }

    #[inline]
    fn is_natural_orientation(&self) -> bool {
        matches!(self.rotation(), Rotation::Default | Rotation::UpsideDown)
    }

    #[inline]
    fn is_portrait_native(&self) -> bool {
        self.native_size.width < self.native_size.height
    }
}

impl Image for BitmapScreen<'_> {
    type ColorType = TrueColor;

    fn size(&self) -> Size {
        if self.is_natural_orientation() {
            self.native_size
        } else {
            Size::new(self.native_size.height, self.native_size.width)
        }
    }
}

impl Screen<BitmapRef32<'_>> for BitmapScreen<'_> {
    fn native_size(&self) -> Size {
        self.native_size
    }

    fn blt(&self, src: &BitmapRef32, origin: Point, rect: Rect) {
        match self.rotation() {
            Rotation::Default => self.bitmap().blt(src, origin, rect),
            Rotation::ClockWise => self.bitmap().blt_cw(src, origin, rect),
            Rotation::UpsideDown | Rotation::CounterClockWise => unreachable!(),
        }
    }

    fn fill_rect(&self, rect: Rect, color: Self::ColorType) {
        if self.is_natural_orientation() {
            self.bitmap().fill_rect(rect, color.into());
        } else {
            let rect = Rect::new(
                self.native_size.width() as i32 - rect.min_y() - rect.height() as i32,
                rect.min_x(),
                rect.height(),
                rect.width(),
            );
            self.bitmap().fill_rect(rect, color.into());
        }
    }

    fn draw_glyph(&self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        if self.is_natural_orientation() {
            self.bitmap().draw_glyph(glyph, size, origin, color);
        } else {
            self.bitmap().draw_glyph_cw(glyph, size, origin, color);
        }
    }

    fn rotation(&self) -> Rotation {
        self.rotation.value()
    }

    fn set_rotation(&self, value: Rotation) -> Result<Rotation, Rotation> {
        if match value {
            Rotation::Default | Rotation::ClockWise => {
                self.rotation.store(value);
                true
            }
            Rotation::UpsideDown | Rotation::CounterClockWise => false,
        } {
            Ok(self.rotation())
        } else {
            Err(self.rotation())
        }
    }

    fn rotate(&self) -> Result<Rotation, Rotation> {
        let new_val = match self.rotation.value() {
            Rotation::Default | Rotation::UpsideDown => Rotation::ClockWise,
            Rotation::ClockWise | Rotation::CounterClockWise => Rotation::Default,
        };
        self.rotation.store(new_val);
        Ok(self.rotation())
    }

    fn set_orientation(
        &self,
        value: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        if self.is_portrait_native() {
            self.rotation.store(match value {
                ScreenOrientation::Portrait => Rotation::Default,
                ScreenOrientation::Landscape => Rotation::ClockWise,
            });
        } else {
            self.rotation.store(match value {
                ScreenOrientation::Portrait => Rotation::ClockWise,
                ScreenOrientation::Landscape => Rotation::Default,
            });
        }
        Ok(self.orientation())
    }
}
