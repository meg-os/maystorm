use super::*;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use core::borrow::{Borrow, BorrowMut};
use core::cell::UnsafeCell;
use core::intrinsics::copy_nonoverlapping;
use core::mem::{swap, transmute, ManuallyDrop};
use core::num::NonZeroUsize;
use core::ptr::slice_from_raw_parts_mut;
use core::slice;
use libm::{ceil, floor};
use paste::paste;
use vec::Vec2;

pub trait Image
where
    Self::ColorType: PixelColor,
{
    type ColorType;

    fn size(&self) -> Size;

    #[inline]
    fn width(&self) -> GlUInt {
        self.size().width()
    }

    #[inline]
    fn height(&self) -> GlUInt {
        self.size().height()
    }

    #[inline]
    fn bounds(&self) -> Rect {
        Rect::from(self.size())
    }
}

pub trait GetPixel: Image {
    /// Faster but unsafe version of get_pixel
    ///
    /// # Safety
    ///
    /// The point must be within the size range.
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType;

    #[inline]
    fn get_pixel(&self, point: Point) -> Option<Self::ColorType> {
        if self.bounds().contains(point) {
            Some(unsafe { self.get_pixel_unchecked(point) })
        } else {
            None
        }
    }

    #[inline]
    fn to_operational<F>(&self, kernel: F) -> OperationalBitmap
    where
        F: FnMut(<Self as Image>::ColorType) -> u8,
    {
        OperationalBitmap::from_pixels(self, kernel)
    }

    /// Returns an iterator that enumerates all pixels of the bitmap in order from left to right and top to bottom.
    #[inline]
    fn all_pixels<'a>(&'a self) -> AllPixels<'a, Self> {
        AllPixels::new(self)
    }
}

pub struct AllPixels<'a, T>
where
    T: GetPixel + ?Sized,
{
    inner: &'a T,
    x: GlUInt,
    y: GlUInt,
}

impl<'a, T> AllPixels<'a, T>
where
    T: GetPixel + ?Sized,
{
    #[inline]
    pub const fn new(inner: &'a T) -> Self {
        Self { inner, x: 0, y: 0 }
    }
}

impl<T> Iterator for AllPixels<'_, T>
where
    T: GetPixel + ?Sized,
{
    type Item = T::ColorType;

    fn next(&mut self) -> Option<Self::Item> {
        if self.y < self.inner.height() {
            if self.x >= self.inner.width() {
                self.x = 0;
                self.y += 1;
            }
            let result = unsafe {
                self.inner
                    .get_pixel_unchecked(Point::new(self.x as GlSInt, self.y as GlSInt))
            };
            self.x += 1;
            Some(result)
        } else {
            None
        }
    }
}

pub trait GetPixelMut: Image {
    /// Faster but unsafe version of get_pixel_mut
    ///
    /// # Safety
    ///
    /// The point must be within the size range.
    unsafe fn get_pixel_unchecked_mut(&mut self, point: Point) -> &mut Self::ColorType;

    #[inline]
    fn get_pixel_mut(&mut self, point: Point) -> Option<&mut Self::ColorType> {
        (self.bounds().contains(point)).then(|| unsafe { self.get_pixel_unchecked_mut(point) })
    }
}

pub trait SetPixel: Image {
    /// Faster but unsafe version of set_pixel
    ///
    /// # Safety
    ///
    /// The point must be within the size range.
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType);

    #[inline]
    fn set_pixel(&mut self, point: Point, pixel: Self::ColorType) {
        if self.bounds().contains(point) {
            unsafe {
                self.set_pixel_unchecked(point, pixel);
            }
        }
    }
}

impl<T> SetPixel for T
where
    T: GetPixelMut,
{
    #[inline]
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        *self.get_pixel_unchecked_mut(point) = pixel;
    }

    #[inline]
    fn set_pixel(&mut self, point: Point, pixel: Self::ColorType) {
        self.get_pixel_mut(point).map(|v| *v = pixel);
    }
}

pub trait RasterImage: Image {
    fn slice(&self) -> &[Self::ColorType];

    fn stride(&self) -> usize {
        self.width() as usize
    }
}

impl<T> GetPixel for T
where
    Self: RasterImage,
{
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        *self
            .slice()
            .get_unchecked(point.x as usize + point.y as usize * self.stride())
    }
}

pub trait MutableRasterImage: RasterImage {
    fn slice_mut(&mut self) -> &mut [Self::ColorType];

    #[inline]
    unsafe fn process_pixel_unchecked<F>(&mut self, point: Point, kernel: F)
    where
        F: FnOnce(Self::ColorType) -> Self::ColorType,
    {
        let stride = self.stride();
        let pixel = self
            .slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride);
        *pixel = kernel(*pixel);
    }
}

impl<T> GetPixelMut for T
where
    Self: MutableRasterImage,
{
    unsafe fn get_pixel_unchecked_mut(&mut self, point: Point) -> &mut Self::ColorType {
        let stride = self.stride();
        self.slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride)
    }
}

pub trait Blt<T: Image>: Image {
    fn blt(&mut self, src: &T, origin: Point, rect: Rect);
}

pub trait DrawRect: SetPixel {
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType);

    fn draw_hline(&mut self, origin: Point, width: GlUInt, color: Self::ColorType);

    fn draw_vline(&mut self, origin: Point, height: GlUInt, color: Self::ColorType);

    fn clear(&mut self) {
        self.fill_rect(self.bounds(), Default::default());
    }

    fn draw_rect(&mut self, rect: Rect, color: Self::ColorType) {
        let Ok(coords) = Coordinates::from_rect(rect) else {
            return;
        };
        let width = rect.width();
        let height = rect.height();
        self.draw_hline(coords.left_top(), width, color);
        if height > 1 {
            self.draw_hline(coords.left_bottom() - Point::new(0, 1), width, color);
            if height > 2 {
                self.draw_vline(coords.left_top() + Point::new(0, 1), height - 2, color);
                self.draw_vline(coords.right_top() + Point::new(-1, 1), height - 2, color);
            }
        }
    }

    fn draw_circle(&mut self, origin: Point, radius: GlUInt, color: Self::ColorType) {
        let radius = safe_clip(radius, GlSInt::MAX / 2);
        let rect = Rect::from((
            origin - (radius as GlSInt),
            Size::new(radius * 2, radius * 2),
        ));
        self.draw_round_rect(rect, radius, color);
    }

    fn fill_circle(&mut self, origin: Point, radius: GlUInt, color: Self::ColorType) {
        let radius = safe_clip(radius, GlSInt::MAX / 2);
        let rect = Rect::from((
            origin - (radius as GlSInt),
            Size::new(radius * 2, radius * 2),
        ));
        self.fill_round_rect(rect, radius, color);
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: GlUInt, color: Self::ColorType) {
        let width = safe_clip(rect.width(), GlSInt::MAX);
        let height = safe_clip(rect.height(), GlSInt::MAX);
        let dx = rect.min_x();
        let dy = rect.min_y();

        let mut radius = safe_clip(radius, GlSInt::MAX / 2);
        if radius * 2 > width {
            radius = width / 2;
        }
        if radius * 2 > height {
            radius = height / 2;
        }

        let lh = height - radius * 2;
        if lh > 0 {
            let rect_line = Rect::new(dx, dy + radius as GlSInt, width, lh);
            self.fill_rect(rect_line, color);
        }

        let radius = radius as GlSInt;
        let mut cx = radius;
        let mut cy = 0;
        let mut f = -2 * radius + 3;
        let qh = height as GlSInt - 1;

        while cx >= cy {
            {
                let bx = radius - cy;
                let by = radius - cx;
                let dw = width - bx as GlUInt * 2;
                self.draw_hline(Point::new(dx + bx, dy + by), dw, color);
                self.draw_hline(Point::new(dx + bx, dy + qh - by), dw, color);
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width - bx as GlUInt * 2;
                self.draw_hline(Point::new(dx + bx, dy + by), dw, color);
                self.draw_hline(Point::new(dx + bx, dy + qh - by), dw, color);
            }

            if f >= 0 {
                cx -= 1;
                f -= 4 * cx;
            }
            cy += 1;
            f += 4 * cy + 2;
        }
    }

    fn draw_round_rect(&mut self, rect: Rect, radius: GlUInt, color: Self::ColorType) {
        let width = safe_clip(rect.width(), GlSInt::MAX);
        let height = safe_clip(rect.height(), GlSInt::MAX);
        let dx = rect.min_x();
        let dy = rect.min_y();

        let mut radius = safe_clip(radius, GlSInt::MAX / 2);
        if radius * 2 > width {
            radius = width / 2;
        }
        if radius * 2 > height {
            radius = height / 2;
        }

        let lh = height - radius * 2;
        if lh > 0 {
            self.draw_vline(Point::new(dx, dy + radius as GlSInt), lh, color);
            self.draw_vline(
                Point::new(dx + width as GlSInt - 1, dy + radius as GlSInt),
                lh,
                color,
            );
        }
        let lw = width - radius * 2;
        if lw > 0 {
            self.draw_hline(Point::new(dx + radius as GlSInt, dy), lw, color);
            self.draw_hline(
                Point::new(dx + radius as GlSInt, dy + height as GlSInt - 1),
                lw,
                color,
            );
        }

        let radius = radius as GlSInt;
        let mut cx = radius;
        let mut cy = 0;
        let mut f = -2 * radius + 3;
        let qh = height as GlSInt - 1;

        while cx >= cy {
            {
                let bx = radius - cy;
                let by = radius - cx;
                let dw = width as GlSInt - bx * 2 - 1;
                self.set_pixel(Point::new(dx + bx, dy + by), color);
                self.set_pixel(Point::new(dx + bx, dy + qh - by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + qh - by), color);
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width as GlSInt - bx * 2 - 1;
                self.set_pixel(Point::new(dx + bx, dy + by), color);
                self.set_pixel(Point::new(dx + bx, dy + qh - by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + qh - by), color);
            }

            if f >= 0 {
                cx -= 1;
                f -= 4 * cx;
            }
            cy += 1;
            f += 4 * cy + 2;
        }
    }

    fn draw_line(&mut self, c1: Point, c2: Point, color: Self::ColorType) {
        if c1.x() == c2.x() {
            if c1.y() < c2.y() {
                let height = 1 + c2.y() - c1.y();
                self.draw_vline(c1, height as GlUInt, color);
            } else {
                let height = 1 + c1.y() - c2.y();
                self.draw_vline(c2, height as GlUInt, color);
            }
        } else if c1.y() == c2.y() {
            if c1.x() < c2.x() {
                let width = 1 + c2.x() - c1.x();
                self.draw_hline(c1, width as GlUInt, color);
            } else {
                let width = 1 + c1.x() - c2.x();
                self.draw_hline(c2, width as GlUInt, color);
            }
        } else {
            c1.line_to(c2, |point| {
                self.set_pixel(point, color);
            });
        }
    }
}

pub trait DrawGlyph: SetPixel {
    fn draw_glyph(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        let stride = (size.width as usize + 7) / 8;

        let Ok(mut coords) = Coordinates::from_rect(Rect::from((origin, size))) else {
            return;
        };

        let width = safe_to_int(self.width());
        let height = safe_to_int(self.height());
        if coords.right > width {
            coords.right = width;
        }
        if coords.bottom > height {
            coords.bottom = height;
        }
        if coords.left < 0 || coords.left >= width || coords.top < 0 || coords.top >= height {
            return;
        }

        let new_rect = Rect::from(coords);
        let width = new_rect.width() as usize;
        let height = new_rect.height();
        let w8 = width / 8;
        let w7 = width & 7;
        let mut cursor = 0;
        for y in 0..height {
            for i in 0..w8 {
                let data = unsafe { glyph.get_unchecked(cursor + i) };
                for j in 0..8 {
                    let position = 0x80u8 >> j;
                    if (data & position) != 0 {
                        let x = (i * 8 + j) as GlSInt;
                        let y = y;
                        let point = Point::new(origin.x + x, origin.y + y as GlSInt);
                        self.set_pixel(point, color);
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { glyph.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for i in 0..w7 {
                    let position = 0x80u8 >> i;
                    if (data & position) != 0 {
                        let x = (i + base_x) as GlSInt;
                        let y = y;
                        let point = Point::new(origin.x + x, origin.y + y as GlSInt);
                        self.set_pixel(point, color);
                    }
                }
            }
            cursor += stride;
        }
    }

    fn draw_glyph_cw(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        let stride = (size.width as usize + 7) / 8;
        let width = self.width() as GlSInt;
        let height = self.height() as GlSInt;

        let Ok(mut coords) = Coordinates::from_rect(Rect::new(
            width - origin.y - size.height as GlSInt,
            origin.x,
            size.height,
            size.width,
        )) else {
            return;
        };

        if coords.right > width {
            coords.right = width;
        }
        if coords.bottom > height {
            coords.bottom = height;
        }
        if coords.left < 0 || coords.left >= width || coords.top < 0 || coords.top >= height {
            return;
        }

        let new_rect = Rect::from(coords);
        let width = new_rect.height() as usize;
        let height = new_rect.width();

        let w8 = width / 8;
        let w7 = width & 7;
        let mut cursor = 0;
        for i in 0..height {
            for j in 0..w8 {
                let data = unsafe { glyph.get_unchecked(cursor + j) };
                for k in 0..8 {
                    let position = 0x80u8 >> k;
                    if (data & position) != 0 {
                        let point = Point::new(
                            coords.right - i as GlSInt,
                            coords.top + (j * 8 + k) as GlSInt,
                        );
                        self.set_pixel(point, color);
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { glyph.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for k in 0..w7 {
                    let position = 0x80u8 >> k;
                    if (data & position) != 0 {
                        let point = Point::new(
                            coords.right - i as GlSInt,
                            coords.top + (base_x + k) as GlSInt,
                        );
                        self.set_pixel(point, color);
                    }
                }
            }
            cursor += stride;
        }
    }
}

pub trait BltConvert<T: PixelColor>: MutableRasterImage {
    #[inline]
    fn blt_convert<U, F>(&mut self, src: &U, origin: Point, rect: Rect, mut f: F)
    where
        U: RasterImage<ColorType = T>,
        F: FnMut(T) -> Self::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        let dd = ds - width;
        let sd = ss - width;
        if dd == 0 && sd == 0 {
            for _ in 0..height * width {
                unsafe {
                    let c = src_fb.get_unchecked(src_cursor);
                    *dest_fb.get_unchecked_mut(dest_cursor) = f(*c);
                }
                src_cursor += 1;
                dest_cursor += 1;
            }
        } else {
            for _ in 0..height {
                for _ in 0..width {
                    unsafe {
                        let c = src_fb.get_unchecked(src_cursor);
                        *dest_fb.get_unchecked_mut(dest_cursor) = f(*c);
                    }
                    src_cursor += 1;
                    dest_cursor += 1;
                }
                dest_cursor += dd;
                src_cursor += sd;
            }
        }
    }

    fn blt_convert_opt<U, F>(&mut self, src: &U, origin: Point, rect: Rect, mut f: F)
    where
        U: RasterImage<ColorType = T>,
        F: FnMut(T) -> Option<Self::ColorType>,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        let dd = ds - width;
        let sd = ss - width;
        if dd == 0 && sd == 0 {
            for _ in 0..height * width {
                unsafe {
                    let c = src_fb.get_unchecked(src_cursor);
                    match f(*c) {
                        Some(c) => *dest_fb.get_unchecked_mut(dest_cursor) = c,
                        None => {}
                    }
                }
                src_cursor += 1;
                dest_cursor += 1;
            }
        } else {
            for _ in 0..height {
                for _ in 0..width {
                    unsafe {
                        let c = src_fb.get_unchecked(src_cursor);
                        match f(*c) {
                            Some(c) => *dest_fb.get_unchecked_mut(dest_cursor) = c,
                            None => {}
                        }
                    }
                    src_cursor += 1;
                    dest_cursor += 1;
                }
                dest_cursor += dd;
                src_cursor += sd;
            }
        }
    }
}

macro_rules! define_bitmap {
    ( $suffix:tt, $inner_type:ty, $color_type:ty, $slice_type:ty, ) => {
        paste! {
            // The memory layouts of BitmapRefXX, BitmapRefMutXX and OwnedBitmapXX are guaranteed to be identical.
            #[repr(C)]
            pub struct [<BitmapRef $suffix>]<'a> {
                slice: &'a [$slice_type],
                size: Size,
                stride: usize,
            }

            #[repr(C)]
            pub struct [<BitmapRefMut $suffix>]<'a> {
                slice: UnsafeCell<&'a mut [$slice_type]>,
                size: Size,
                stride: usize,
            }

            #[repr(C)]
            pub struct [<OwnedBitmap $suffix>] {
                slice: UnsafeCell<Box<[$slice_type]>>,
                size: Size,
                stride: usize,
            }

            impl Image for [<BitmapRef $suffix>]<'_> {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl Image for [<BitmapRefMut $suffix>]<'_> {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl Image for [<OwnedBitmap $suffix>] {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl<'a> [<BitmapRef $suffix>]<'a> {
                #[inline]
                pub fn from_slice(
                    slice: &'a [$slice_type],
                    size: Size,
                    stride: Option<NonZeroUsize>,
                ) -> Self
                {
                    Self {
                        size,
                        stride: match stride {
                            Some(v) => v.get(),
                            None => <Self as Image>::ColorType::stride_for(size.width()),
                        },
                        slice,
                    }
                }

                #[inline]
                pub fn from_bytes(bytes: &'a [$inner_type], size: Size) -> Self
                {
                    Self {
                        size,
                        stride: <Self as Image>::ColorType::stride_for(size.width()),
                        slice: unsafe { transmute(bytes) },
                    }
                }

                #[inline]
                pub fn clone(&'a self) -> Self {
                    Self {
                        size: self.size(),
                        stride: self.stride(),
                        slice: self.slice(),
                    }
                }
            }

            impl<'a> [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                pub fn from_slice(
                    slice: &'a mut [$slice_type],
                    size: Size,
                    stride: Option<NonZeroUsize>,
                ) -> Self
                {
                    Self {
                        size,
                        stride: match stride {
                            Some(v) => v.get(),
                            None => <Self as Image>::ColorType::stride_for(size.width()),
                        },
                        slice: UnsafeCell::new(slice),
                    }
                }

                #[inline]
                pub fn from_bytes(bytes: &'a mut [$inner_type], size: Size) -> Self
                {
                    Self {
                        size,
                        stride: <Self as Image>::ColorType::stride_for(size.width()),
                        slice: unsafe { transmute(bytes) },
                    }
                }

                #[inline]
                pub const fn as_const(&'a self) -> &'a [<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }

                #[inline]
                pub const fn into_const(self) -> [<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }

                #[inline]
                pub fn clone_mut(&'a mut self) -> [<BitmapRefMut $suffix>]<'a> {
                    let slice = unsafe { &mut *self.slice.get() };
                    Self {
                        size: self.size(),
                        stride: self.stride(),
                        slice: UnsafeCell::new(slice),
                    }
                }
            }

            impl [<BitmapRefMut $suffix>]<'static> {
                /// # Safety
                ///
                /// Must guarantee the existence of the `ptr`.
                #[inline]
                pub unsafe fn from_static(ptr: *mut $slice_type, size: Size, stride: usize) -> Self {
                    let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
                    Self {
                        size,
                        stride,
                        slice: UnsafeCell::new(slice),
                    }
                }
            }

            impl [<OwnedBitmap $suffix>] {
                #[inline]
                pub fn from_boxed_slice(slice: Box<[$slice_type]>, size: Size) -> Self {
                    Self {
                        size: size,
                        stride: <Self as Image>::ColorType::stride_for(size.width()),
                        slice: UnsafeCell::new(slice),
                    }
                }

                #[inline]
                pub fn from_vec(vec: Vec<$slice_type>, size: Size) -> Self {
                    Self::from_boxed_slice(vec.into_boxed_slice(), size)
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<BitmapRef $suffix>]<'a> {
                #[inline]
                fn as_ref(&self) -> &Self {
                    self
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn as_ref(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> AsMut<[<BitmapRefMut $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn as_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    self
                }
            }

            impl<'a> Borrow<[<BitmapRef $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn borrow(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl ToOwned for [<BitmapRefMut $suffix>]<'_> {
                type Owned = [<OwnedBitmap $suffix>];

                #[inline]
                fn to_owned(&self) -> Self::Owned {
                    let vec = self.slice().to_vec();
                    <[<OwnedBitmap $suffix>]>::from_vec(vec, self.size())
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn as_ref(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> AsMut<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn as_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> Borrow<[<BitmapRef $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> Borrow<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow(&self) -> &[<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> BorrowMut<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

        }
    };
    ( $suffix:tt, $inner_type:ty, $color_type:ty, ) => {
        define_bitmap!($suffix, $inner_type, $color_type, $color_type,);

        paste! {
            impl RasterImage for [<BitmapRef $suffix>]<'_> {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    self.slice
                }
            }

            impl RasterImage for [<BitmapRefMut $suffix>]<'_> {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    unsafe { &*self.slice.get() }
                }
            }

            impl MutableRasterImage for [<BitmapRefMut $suffix>]<'_> {
                #[inline]
                fn slice_mut(&mut self) -> &mut [Self::ColorType] {
                    self.slice.get_mut()
                }
            }

            impl RasterImage for [<OwnedBitmap $suffix>] {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    unsafe { &*self.slice.get() }
                }
            }

            impl MutableRasterImage for [<OwnedBitmap $suffix>] {
                #[inline]
                fn slice_mut(&mut self) -> &mut [Self::ColorType] {
                    self.slice.get_mut()
                }
            }

            impl<'a> [<BitmapRefMut $suffix>]<'a> {
                /// Returns a subset of the specified range of bitmaps.
                /// The function returns None if the rectangle points outside the range of the bitmap.
                pub fn sub_image(&mut self, rect: Rect) -> Option<[<BitmapRefMut $suffix>]<'a>> {
                    let Ok(coords) = Coordinates::try_from(rect) else {
                        return None;
                    };
                    let width = safe_to_int(self.width());
                    let height = safe_to_int(self.height());
                    let stride = self.stride();

                    if coords.left < 0
                        || coords.left >= width
                        || coords.right > width
                        || coords.top < 0
                        || coords.top >= height
                        || coords.bottom > height
                    {
                        return None;
                    }

                    let offset = rect.min_x() as usize + rect.min_y() as usize * stride;
                    let new_len = (rect.height() as usize - 1) * stride + rect.width() as usize;
                    Some(unsafe {
                        let p = self.slice_mut().as_mut_ptr();
                        Self {
                            size: rect.size(),
                            stride,
                            slice: UnsafeCell::new(slice::from_raw_parts_mut(p.add(offset), new_len)),
                        }
                    })
                }
            }

            impl [<BitmapRefMut $suffix>]<'_> {
                pub fn copy(&mut self, origin: Point, rect: Rect) {
                    let (dx, dy, sx, sy, width, height) =
                        _adjust_blt_coords(self.size(), self.size(), origin, rect);
                    if width <= 0 || height <= 0 {
                        return;
                    }
                    let width = width as usize;
                    let height = height as usize;
                    let stride = self.stride();

                    // TODO: overlapping
                    unsafe {
                        let dest_fb = self.slice_mut().as_mut_ptr();
                        let mut dest_ptr = dest_fb.add(dx as usize + dy as usize * stride);
                        let mut src_ptr = dest_fb.add(sx as usize + sy as usize * stride) as *const _;

                        if stride == width {
                            dest_ptr.copy_from_nonoverlapping(src_ptr, width * height);
                        } else {
                            for _ in 0..height {
                                dest_ptr.copy_from_nonoverlapping(src_ptr, width);
                                dest_ptr = dest_ptr.add(stride);
                                src_ptr = src_ptr.add(stride);
                            }
                        }
                    }
                }
            }

            impl<'a, 'b> Blt<[<BitmapRef $suffix>]<'b>> for [<BitmapRefMut $suffix>]<'a> {
                fn blt(&mut self, src: &[<BitmapRef $suffix>]<'b>, origin: Point, rect: Rect) {
                    let (dx, dy, sx, sy, width, height) =
                        _adjust_blt_coords(self.size(), src.size(), origin, rect);
                    if width <= 0 || height <= 0 {
                        return;
                    }
                    let width = width as usize;
                    let height = height as usize;

                    let ds = self.stride();
                    let ss = src.stride();
                    unsafe {
                        let mut dest_fb = self
                            .slice_mut()
                            .as_mut_ptr()
                            .add(dx as usize + dy as usize * ds);
                        let mut src_fb = src
                            .slice()
                            .as_ptr()
                            .add(sx as usize + sy as usize * ss) as *const _;

                        if ds == width && ss == width {
                            dest_fb.copy_from_nonoverlapping(src_fb, width * height);
                        } else {
                            for _ in 0..height {
                                dest_fb.copy_from_nonoverlapping(src_fb, width);
                                dest_fb = dest_fb.add(ds);
                                src_fb = src_fb.add(ss);
                            }
                        }
                    }
                }
            }

            impl DrawGlyph for [<BitmapRefMut $suffix>]<'_> {}

            impl DrawRect for [<BitmapRefMut $suffix>]<'_> {
                fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
                    let mut width = safe_to_int(rect.width());
                    let mut height = safe_to_int(rect.height());
                    let mut dx = rect.min_x();
                    let mut dy = rect.min_y();
                    let sw = safe_to_int(self.size.width());
                    let sh = safe_to_int(self.size.height());

                    if dx < 0 {
                        width += dx;
                        dx = 0;
                    }
                    if dy < 0 {
                        height += dy;
                        dy = 0;
                    }
                    let r = dx + width;
                    let b = dy + height;
                    if r >= sw {
                        width = sw - dx;
                    }
                    if b >= sh {
                        height = sh - dy;
                    }
                    if width <= 0 || height <= 0 {
                        return;
                    }

                    let width = width as usize;
                    let height = height as usize;
                    let stride = self.stride;
                    let mut cursor = dx as usize + dy as usize * stride;
                    if stride == width {
                        memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, width * height, color);
                    } else {
                        for _ in 0..height {
                            memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, width, color);
                            cursor += stride;
                        }
                    }
                }

                fn draw_hline(&mut self, origin: Point, width: GlUInt, color: Self::ColorType) {
                    let size = self.size();
                    let mut dx = origin.x;
                    let dy = origin.y;
                    let mut w = width as GlSInt;

                    if dy < 0 || dy >= size.height as GlSInt {
                        return;
                    }
                    if dx < 0 {
                        w += dx;
                        dx = 0;
                    }
                    let r = dx + w;
                    if r >= size.width as GlSInt {
                        w = size.width as GlSInt - dx;
                    }
                    if w <= 0 {
                        return;
                    }

                    let cursor = dx as usize + dy as usize * self.stride;
                    memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, w as usize, color);
                }

                fn draw_vline(&mut self, origin: Point, height: GlUInt, color: Self::ColorType) {
                    let size = self.size();
                    let dx = origin.x;
                    let mut dy = origin.y;
                    let mut h = height as GlSInt;

                    if dx < 0 || dx >= size.width as GlSInt {
                        return;
                    }
                    if dy < 0 {
                        h += dy;
                        dy = 0;
                    }
                    let b = dy + h;
                    if h >= size.height as GlSInt || b >= size.height as GlSInt {
                        h = size.height as GlSInt - dy - 1;
                    }
                    if h <= 0 {
                        return;
                    }

                    let stride = self.stride;
                    let mut cursor = dx as usize + dy as usize * stride;
                    for _ in 0..h {
                        self.slice_mut()[cursor] = color;
                        cursor += stride;
                    }
                }
            }

            impl [<OwnedBitmap $suffix>] {
                #[inline]
                pub fn new(size: Size, bg_color: <Self as Image>::ColorType) -> Self {
                    let len = size.width() as usize * size.height() as usize;
                    let mut vec = Vec::with_capacity(len);
                    vec.resize(len, bg_color);
                    Self::from_vec(vec, size)
                }
            }

        }
    };
}

define_bitmap!(8, u8, IndexedColor,);
define_bitmap!(16, u16, RGB565,);
define_bitmap!(32, u32, ARGB8888,);

impl BltConvert<ARGB8888> for BitmapRefMut8<'_> {}
impl BltConvert<IndexedColor> for BitmapRefMut8<'_> {}

impl BitmapRefMut8<'_> {
    pub fn blt_with_key(
        &mut self,
        src: &BitmapRef8,
        origin: Point,
        rect: Rect,
        color_key: <Self as Image>::ColorType,
    ) {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        for _ in 0..height {
            for i in 0..width {
                let c = src_fb[src_cursor + i];
                if c != color_key {
                    dest_fb[dest_cursor + i] = c;
                }
            }
            dest_cursor += ds;
            src_cursor += ss;
        }
    }

    #[inline]
    pub fn blt32(&mut self, src: &BitmapRef32, origin: Point, rect: Rect) {
        self.blt_convert(src, origin, rect, |c| IndexedColor::from_rgb(c.rgb()));
    }
}

impl BltConvert<ARGB8888> for BitmapRefMut32<'_> {}
impl BltConvert<IndexedColor> for BitmapRefMut32<'_> {}

impl BitmapRefMut32<'_> {
    pub fn blend_rect(&mut self, rect: Rect, color: ARGB8888) {
        let rhs = color.components();
        if rhs.is_opaque() {
            return self.fill_rect(rect, color);
        }
        if rhs.is_transparent() {
            return;
        }
        let alpha = rhs.a.as_usize();
        let alpha_n = 255 - alpha;

        let mut width = safe_to_int(rect.width());
        let mut height = safe_to_int(rect.height());
        let mut dx = rect.min_x();
        let mut dy = rect.min_y();

        if dx < 0 {
            width += dx;
            dx = 0;
        }
        if dy < 0 {
            height += dy;
            dy = 0;
        }
        let r = dx + width;
        let b = dy + height;
        if r >= self.size().width as GlSInt {
            width = self.size().width as GlSInt - dx;
        }
        if b >= self.size().height as GlSInt {
            height = self.size().height as GlSInt - dy;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        let mut cursor = dx as usize + dy as usize * self.stride();
        let stride = self.stride() - width as usize;
        let slice = self.slice_mut();

        let kernel =
            |lhs: u8, rhs: u8| (((lhs as usize) * alpha_n + (rhs as usize) * alpha) / 255) as u8;

        for _ in 0..height {
            for _ in 0..width {
                let p = unsafe { slice.get_unchecked_mut(cursor) };
                let lhs = p.components();
                *p = ColorComponents::from_rgba(
                    kernel(lhs.r, rhs.r),
                    kernel(lhs.g, rhs.g),
                    kernel(lhs.b, rhs.b),
                    lhs.a.saturating_add(rhs.a),
                )
                .into_true_color();
                cursor += 1;
            }
            cursor += stride;
        }
    }

    pub fn blt_blend(&mut self, src: &BitmapRef32, origin: Point, rect: Rect, opacity: Alpha8) {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if opacity.is_transparent() || width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        if opacity == Alpha8::OPAQUE {
            for _ in 0..height {
                memory_colors::_memcpy_blend32(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        } else {
            // TODO:
            for _ in 0..height {
                memory_colors::_memcpy_blend32(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        }
    }

    pub fn blt8(&mut self, src: &BitmapRef8, origin: Point, rect: Rect, palette: &[u32; 256]) {
        self.blt_convert(src, origin, rect, |c| {
            ARGB8888::from_argb(palette[c.0 as usize])
        });
    }

    pub fn blt_cw(&mut self, src: &BitmapRef32, origin: Point, rect: Rect) {
        let self_size = Size::new(self.height(), self.width());
        let (mut dx, mut dy, sx, sy, width, height) =
            _adjust_blt_coords(self_size, src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let temp = dx;
        dx = self_size.height() as GlSInt - dy;
        dy = temp;
        let mut p = dx as usize + dy as usize * ds - height as usize;
        let q0 = sx as usize + (sy as usize + height - 1) * ss;
        let stride_p = ds - height;
        let stride_q = ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        for x in 0..width {
            let mut q = q0 + x;
            for _ in 0..height {
                dest_fb[p] = src_fb[q];
                p += 1;
                q -= stride_q;
            }
            p += stride_p;
        }
    }

    #[inline]
    pub fn blt_transparent(
        &mut self,
        src: &BitmapRef,
        origin: Point,
        rect: Rect,
        color_key: IndexedColor,
    ) {
        match src {
            BitmapRef::Indexed(src) => self.blt_convert_opt(*src, origin, rect, |c| {
                if c == color_key {
                    None
                } else {
                    Some(c.into())
                }
            }),
            BitmapRef::Argb32(src) => self.blt_blend(src, origin, rect, Alpha8::OPAQUE),
        }
    }
}

impl BitmapRef32<'_> {
    pub fn scale(&self, target_size: Size) -> Result<OwnedBitmap32, ()> {
        if self.width() > target_size.width() && self.height() > target_size.height() {
            self.scale_reduction(target_size)
        } else {
            self.scale_linear(target_size)
        }
    }

    /// Resize a image using nearest neighbor interpolation
    pub fn scale_nn(&self, target_size: Size) -> Result<OwnedBitmap32, ()> {
        let mut vec = Vec::new();
        vec.try_reserve(target_size.width_height_usize())
            .map_err(|_| ())?;

        let sw = self.width() as f64;
        let sh = self.height() as f64;
        let dw = target_size.width() as f64;
        let dh = target_size.height() as f64;

        for y in 0..target_size.height() {
            let vy = y as f64 * sh / dh;
            for x in 0..target_size.width() {
                let vx = x as f64 * sw / dw;
                let new_pixel = unsafe {
                    self.get_pixel_unchecked(Point::new(floor(vx) as i32, floor(vy) as i32))
                };
                vec.push(new_pixel);
            }
        }

        let target = OwnedBitmap32::from_vec(vec, target_size);
        Ok(target)
    }

    #[inline(always)]
    fn _scale_main<COLOR, F>(
        source_size: Size,
        target_size: Size,
        mut kernel: F,
    ) -> Option<Vec<COLOR>>
    where
        F: FnMut(f64, f64, f64, f64) -> COLOR,
    {
        let mut vec = Vec::new();
        vec.try_reserve(target_size.width_height_usize()).ok()?;

        let sw = source_size.width() as f64;
        let sh = source_size.height() as f64;
        let dw = target_size.width() as f64 - 1.0;
        let dh = target_size.height() as f64 - 1.0;

        for y in 0..target_size.height() {
            let vy = y as f64 * sh / dh;
            for x in 0..target_size.width() {
                let vx = x as f64 * sw / dw;
                vec.push(kernel(vx, vy, sw, sh));
            }
        }

        Some(vec)
    }

    /// Resize a image using bilinear interpolation
    pub fn scale_linear(&self, target_size: Size) -> Result<OwnedBitmap32, ()> {
        let vec = Self::_scale_main(self.size(), target_size, |vx, vy, sw, sh| {
            let vx = (vx - 0.5).max(0.0);
            let vy = (vy - 0.5).max(0.0);

            let lx = floor(vx);
            let ly = floor(vy);
            let x_frac = vx - lx;
            let y_frac = vy - ly;

            let hx = floor(lx + 1.0).clamp(0.0, sw - 1.0) as i32;
            let hy = floor(ly + 1.0).clamp(0.0, sh - 1.0) as i32;
            let lx = lx as i32;
            let ly = ly as i32;

            let vll = unsafe { self.get_pixel_unchecked(Point::new(lx, ly)) }
                .components()
                .into_array();
            let vlh = unsafe { self.get_pixel_unchecked(Point::new(lx, hy)) }
                .components()
                .into_array();
            let vhl = unsafe { self.get_pixel_unchecked(Point::new(hx, ly)) }
                .components()
                .into_array();
            let vhh = unsafe { self.get_pixel_unchecked(Point::new(hx, hy)) }
                .components()
                .into_array();

            let mut result = [0u8; 4];
            for i in 0..4 {
                let a = vll[i] as f64;
                let b = vhl[i] as f64;
                let c = vlh[i] as f64;
                let d = vhh[i] as f64;

                let q = a * (1.0 - x_frac) * (1.0 - y_frac)
                    + b * (x_frac) * (1.0 - y_frac)
                    + c * (y_frac) * (1.0 - x_frac)
                    + d * (x_frac * y_frac);

                result[i] = q.clamp(0.0, 255.0) as u8;
            }

            ColorComponents::from_array(result).into_true_color()
        })
        .ok_or(())?;
        let target = OwnedBitmap32::from_vec(vec, target_size);
        Ok(target)
    }

    /// Resize a image using bicubic interpolation
    pub fn scale_cubic(&self, target_size: Size) -> Result<OwnedBitmap32, ()> {
        let vec = Self::_scale_main(self.size(), target_size, |vx, vy, sw, sh| {
            let vx = vx - 0.5;
            let vy = vy - 0.5;

            let lx = floor(vx);
            let ly = floor(vy);
            let x_frac = vx - lx;
            let y_frac = vy - ly;

            let lxm1 = (lx - 1.0).clamp(0.0, sw - 1.0) as i32;
            let lx_0 = (lx).clamp(0.0, sw - 1.0) as i32;
            let lxp1 = (lx + 1.0).clamp(0.0, sw - 1.0) as i32;
            let lxp2 = (lx + 2.0).clamp(0.0, sw - 1.0) as i32;

            let lym1 = (ly - 1.0).clamp(0.0, sh - 1.0) as i32;
            let ly_0 = (ly).clamp(0.0, sh - 1.0) as i32;
            let lyp1 = (ly + 1.0).clamp(0.0, sh - 1.0) as i32;
            let lyp2 = (ly + 2.0).clamp(0.0, sh - 1.0) as i32;

            let p00 = unsafe { self.get_pixel_unchecked(Point::new(lxm1, lym1)) }
                .components()
                .into_array();
            let p10 = unsafe { self.get_pixel_unchecked(Point::new(lx_0, lym1)) }
                .components()
                .into_array();
            let p20 = unsafe { self.get_pixel_unchecked(Point::new(lxp1, lym1)) }
                .components()
                .into_array();
            let p30 = unsafe { self.get_pixel_unchecked(Point::new(lxp2, lym1)) }
                .components()
                .into_array();

            let p01 = unsafe { self.get_pixel_unchecked(Point::new(lxm1, ly_0)) }
                .components()
                .into_array();
            let p11 = unsafe { self.get_pixel_unchecked(Point::new(lx_0, ly_0)) }
                .components()
                .into_array();
            let p21 = unsafe { self.get_pixel_unchecked(Point::new(lxp1, ly_0)) }
                .components()
                .into_array();
            let p31 = unsafe { self.get_pixel_unchecked(Point::new(lxp2, ly_0)) }
                .components()
                .into_array();

            let p02 = unsafe { self.get_pixel_unchecked(Point::new(lxm1, lyp1)) }
                .components()
                .into_array();
            let p12 = unsafe { self.get_pixel_unchecked(Point::new(lx_0, lyp1)) }
                .components()
                .into_array();
            let p22 = unsafe { self.get_pixel_unchecked(Point::new(lxp1, lyp1)) }
                .components()
                .into_array();
            let p32 = unsafe { self.get_pixel_unchecked(Point::new(lxp2, lyp1)) }
                .components()
                .into_array();

            let p03 = unsafe { self.get_pixel_unchecked(Point::new(lxm1, lyp2)) }
                .components()
                .into_array();
            let p13 = unsafe { self.get_pixel_unchecked(Point::new(lx_0, lyp2)) }
                .components()
                .into_array();
            let p23 = unsafe { self.get_pixel_unchecked(Point::new(lxp1, lyp2)) }
                .components()
                .into_array();
            let p33 = unsafe { self.get_pixel_unchecked(Point::new(lxp2, lyp2)) }
                .components()
                .into_array();

            let mut result = [0u8; 4];
            #[inline]
            fn cubic_hermite(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
                let c0 = -a / 2.0 + (3.0 * b) / 2.0 - (3.0 * c) / 2.0 + d / 2.0;
                let c1 = a - (5.0 * b) / 2.0 + 2.0 * c - d / 2.0;
                let c2 = -a / 2.0 + c / 2.0;

                c0 * t * t * t + c1 * t * t + c2 * t + b
            }
            for i in 0..4 {
                let c0 = cubic_hermite(
                    p00[i] as f64,
                    p10[i] as f64,
                    p20[i] as f64,
                    p30[i] as f64,
                    x_frac,
                );
                let c1 = cubic_hermite(
                    p01[i] as f64,
                    p11[i] as f64,
                    p21[i] as f64,
                    p31[i] as f64,
                    x_frac,
                );
                let c2 = cubic_hermite(
                    p02[i] as f64,
                    p12[i] as f64,
                    p22[i] as f64,
                    p32[i] as f64,
                    x_frac,
                );
                let c3 = cubic_hermite(
                    p03[i] as f64,
                    p13[i] as f64,
                    p23[i] as f64,
                    p33[i] as f64,
                    x_frac,
                );
                let q = cubic_hermite(c0, c1, c2, c3, y_frac);

                result[i] = q.clamp(0.0, 255.0) as u8;
            }

            ColorComponents::from_array(result).into_true_color()
        })
        .ok_or(())?;
        let target = OwnedBitmap32::from_vec(vec, target_size);
        Ok(target)
    }

    pub fn scale_reduction(&self, target_size: Size) -> Result<OwnedBitmap32, ()> {
        let mut vec = Vec::new();
        vec.try_reserve(target_size.width_height_usize())
            .map_err(|_| ())?;

        let sw = self.size.width() as f64;
        let sh = self.size.height() as f64;
        let dw = target_size.width() as f64 - 1.0;
        let dh = target_size.height() as f64 - 1.0;

        #[inline(always)]
        fn kernel(
            ctx: &BitmapRef32,
            x: u32,
            y: u32,
            sw: f64,
            sh: f64,
            dw: f64,
            dh: f64,
        ) -> TrueColor {
            let vx = x as f64 * sw / dw;
            let vy = y as f64 * sh / dh;

            let lx = floor(vx) as i32;
            let ly = floor(vy) as i32;
            let hx = ceil(vx + sw / dw).min(sw - 1.0) as i32;
            let hy = ceil(vy + sh / dh).min(sh - 1.0) as i32;

            let mut acc = [0.0; 4];
            for y in ly..hy {
                for x in lx..hx {
                    let p = unsafe { ctx.get_pixel_unchecked(Point::new(x, y)) }
                        .components()
                        .into_array();
                    for ch in 0..4 {
                        acc[ch] += p[ch] as f64;
                    }
                }
            }

            let mut result = [0; 4];
            let count = (hy as f64 - ly as f64) * (hx as f64 - lx as f64);
            for i in 0..4 {
                result[i] = (acc[i] / count).clamp(0.0, 255.0) as u8
            }
            ColorComponents::from_array(result).into_true_color()
        }

        for y in 0..target_size.height() {
            for x in 0..target_size.width() {
                let new_pixel = kernel(self, x, y, sw, sh, dw, dh);
                vec.push(new_pixel);
            }
        }

        let target = OwnedBitmap32::from_vec(vec, target_size);
        Ok(target)
    }
}

impl OwnedBitmap32 {
    pub fn from_vec_rgba(mut vec: Vec<u8>, size: Size) -> Self {
        const MAGIC_NUMBER: usize = 4;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        let slice = unsafe {
            vec.resize(count * MAGIC_NUMBER, 0);
            let slice = vec.into_boxed_slice();
            let mut slice = ManuallyDrop::new(slice);
            let mut slice = Box::from_raw(slice_from_raw_parts_mut(
                slice.as_mut_ptr() as *mut u32,
                count,
            ));
            for pixel in slice.iter_mut() {
                let rgba = pixel.to_le_bytes();
                *pixel = u32::from_le_bytes([rgba[2], rgba[1], rgba[0], rgba[3]]);
            }
            transmute::<_, Box<[ARGB8888]>>(slice)
        };
        Self::from_boxed_slice(slice, size)
    }

    pub fn from_bytes_rgba(bytes: &[u8], size: Size) -> Option<Self> {
        const MAGIC_NUMBER: usize = 4;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        if bytes.len() < count * MAGIC_NUMBER {
            return None;
        }
        let mut vec = Vec::with_capacity(count);
        for rgba in bytes.chunks_exact(MAGIC_NUMBER).take(count) {
            let rgba: [u8; MAGIC_NUMBER] = rgba.try_into().unwrap();
            vec.push(
                ColorComponents::from_rgba(rgba[0], rgba[1], rgba[2], Alpha8::new(rgba[3]))
                    .into_true_color(),
            );
        }
        Some(Self::from_vec(vec, size))
    }

    pub fn from_bytes_rgb(bytes: &[u8], size: Size) -> Option<Self> {
        const MAGIC_NUMBER: usize = 3;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        if bytes.len() < count * MAGIC_NUMBER {
            return None;
        }
        let mut vec = Vec::with_capacity(count);
        for rgb in bytes.chunks_exact(MAGIC_NUMBER).take(count) {
            let rgb: [u8; MAGIC_NUMBER] = rgb.try_into().unwrap();
            let argb = ColorComponents::from_rgba(rgb[0], rgb[1], rgb[2], Alpha8::OPAQUE)
                .into_true_color();
            vec.push(argb);
        }
        Some(Self::from_vec(vec, size))
    }
}

#[derive(Clone, Copy)]
pub enum BitmapRef<'a> {
    Indexed(&'a BitmapRef8<'a>),
    Argb32(&'a BitmapRef32<'a>),
}

impl Image for BitmapRef<'_> {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(v) => v.size(),
            Self::Argb32(v) => v.size(),
        }
    }
}

impl GetPixel for BitmapRef<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl<'a> From<&'a BitmapRef8<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRef8<'a>) -> BitmapRef<'a> {
        BitmapRef::Indexed(val)
    }
}

impl<'a> From<&'a BitmapRefMut8<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRefMut8<'a>) -> Self {
        BitmapRef::Indexed(unsafe { transmute(val) })
    }
}

impl<'a> From<&'a BitmapRef32<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRef32<'a>) -> BitmapRef {
        BitmapRef::Argb32(val)
    }
}

impl<'a> From<&'a BitmapRefMut32<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRefMut32<'a>) -> Self {
        BitmapRef::Argb32(unsafe { transmute(val) })
    }
}

impl<'a> AsRef<BitmapRef<'a>> for BitmapRef<'a> {
    fn as_ref(&self) -> &BitmapRef<'a> {
        self
    }
}

pub enum BitmapRefMut<'a> {
    Indexed(BitmapRefMut8<'a>),
    Argb32(BitmapRefMut32<'a>),
}

impl Image for BitmapRefMut<'_> {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(ref v) => v.size(),
            Self::Argb32(ref v) => v.size(),
        }
    }
}

impl<'a> BitmapRefMut<'a> {
    #[inline]
    pub fn as_const(&'a self) -> BitmapRef<'a> {
        match self {
            BitmapRefMut::Indexed(v) => BitmapRef::Indexed(v.as_ref()),
            BitmapRefMut::Argb32(v) => BitmapRef::Argb32(v.as_ref()),
        }
    }

    /// Returns a subset of the specified range of bitmaps.
    /// The function returns None if the rectangle points outside the range of the bitmap.
    pub fn sub_image(&mut self, rect: Rect) -> Option<Self> {
        match self {
            BitmapRefMut::Indexed(v) => v.sub_image(rect).map(|v| BitmapRefMut::Indexed(v)),
            BitmapRefMut::Argb32(v) => v.sub_image(rect).map(|v| BitmapRefMut::Argb32(v)),
        }
    }
}

impl BitmapRefMut<'_> {
    #[inline]
    pub fn copy(&mut self, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut v) => v.copy(origin, rect),
            Self::Argb32(ref mut v) => v.copy(origin, rect),
        }
    }

    #[inline]
    pub fn blt_transparent(
        &mut self,
        src: &BitmapRef,
        origin: Point,
        rect: Rect,
        color_key: IndexedColor,
    ) {
        match self {
            BitmapRefMut::Indexed(bitmap) => match src {
                BitmapRef::Indexed(src) => bitmap.blt_with_key(src, origin, rect, color_key),
                BitmapRef::Argb32(src) => bitmap.blt_convert_opt(*src, origin, rect, |c| {
                    if c.is_transparent() {
                        None
                    } else {
                        Some(c.into())
                    }
                }),
            },
            BitmapRefMut::Argb32(bitmap) => bitmap.blt_transparent(src, origin, rect, color_key),
        }
    }

    #[inline]
    pub fn map_indexed<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut BitmapRefMut8) -> R,
    {
        match self {
            Self::Indexed(ref mut v) => Some(f(v)),
            Self::Argb32(_) => None,
        }
    }

    #[inline]
    pub fn map_argb32<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut BitmapRefMut32) -> R,
    {
        match self {
            Self::Indexed(_) => None,
            Self::Argb32(ref mut v) => Some(f(v)),
        }
    }

    #[inline]
    pub const fn color_mode(&self) -> usize {
        match self {
            Self::Indexed(_) => 8,
            Self::Argb32(_) => 32,
        }
    }
}

impl GetPixel for BitmapRefMut<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(ref v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(ref v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl SetPixel for BitmapRefMut<'_> {
    #[inline]
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
            Self::Argb32(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
        }
    }
}

impl DrawGlyph for BitmapRefMut<'_> {
    #[inline]
    fn draw_glyph(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_glyph(glyph, size, origin, color.into()),
            Self::Argb32(ref mut v) => v.draw_glyph(glyph, size, origin, color.into()),
        }
    }

    fn draw_glyph_cw(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_glyph_cw(glyph, size, origin, color.into()),
            Self::Argb32(ref mut v) => v.draw_glyph_cw(glyph, size, origin, color.into()),
        }
    }
}

impl DrawRect for BitmapRefMut<'_> {
    #[inline]
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.fill_rect(rect, color.into()),
            Self::Argb32(ref mut v) => v.fill_rect(rect, color.into()),
        }
    }

    #[inline]
    fn draw_hline(&mut self, origin: Point, width: GlUInt, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_hline(origin, width, color.into()),
            Self::Argb32(ref mut v) => v.draw_hline(origin, width, color.into()),
        }
    }

    #[inline]
    fn draw_vline(&mut self, origin: Point, height: GlUInt, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_vline(origin, height, color.into()),
            Self::Argb32(ref mut v) => v.draw_vline(origin, height, color.into()),
        }
    }
}

impl<'a, 'b> Blt<BitmapRef<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef<'b>, origin: Point, rect: Rect) {
        match self {
            BitmapRefMut::Indexed(ref mut bitmap) => match src {
                BitmapRef::Indexed(ref src) => bitmap.blt(src, origin, rect),
                BitmapRef::Argb32(ref src) => bitmap.blt32(src, origin, rect),
            },
            BitmapRefMut::Argb32(ref mut bitmap) => match src {
                BitmapRef::Indexed(ref src) => {
                    bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
                }
                BitmapRef::Argb32(ref src) => bitmap.blt(src, origin, rect),
            },
        }
    }
}

impl<'a, 'b> Blt<BitmapRef8<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef8<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt(src, origin, rect),
            Self::Argb32(ref mut bitmap) => {
                bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
            }
        }
    }
}

impl<'a, 'b> Blt<BitmapRef32<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef32<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt32(src, origin, rect),
            Self::Argb32(ref mut bitmap) => bitmap.blt(src, origin, rect),
        }
    }
}

impl<'a> From<BitmapRefMut8<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn from(val: BitmapRefMut8<'a>) -> BitmapRefMut<'a> {
        Self::Indexed(val)
    }
}

impl<'a> From<BitmapRefMut32<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn from(val: BitmapRefMut32<'a>) -> BitmapRefMut<'a> {
        Self::Argb32(val)
    }
}

impl<'a> AsMut<BitmapRefMut<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut BitmapRefMut<'a> {
        self
    }
}

pub enum OwnedBitmap {
    Indexed(OwnedBitmap8),
    Argb32(OwnedBitmap32),
}

impl Image for OwnedBitmap {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(ref v) => v.size(),
            Self::Argb32(ref v) => v.size(),
        }
    }
}

impl OwnedBitmap {
    #[inline]
    pub fn new<'b, T: AsRef<BitmapRef<'b>>>(
        template_bitmap: &T,
        size: Size,
        bg_color: Color,
    ) -> OwnedBitmap {
        match template_bitmap.as_ref() {
            BitmapRef::Indexed(_) => Self::Indexed(OwnedBitmap8::new(size, bg_color.into())),
            BitmapRef::Argb32(_) => Self::Argb32(OwnedBitmap32::new(size, bg_color.into())),
        }
    }

    #[inline]
    pub fn same_format(&self, size: Size, bg_color: Color) -> OwnedBitmap {
        match self {
            Self::Indexed(_) => Self::Indexed(OwnedBitmap8::new(size, bg_color.into())),
            Self::Argb32(_) => Self::Argb32(OwnedBitmap32::new(size, bg_color.into())),
        }
    }

    #[inline]
    pub fn into_bitmap<'a>(self) -> BitmapRefMut<'a> {
        unsafe { transmute(self) }
    }

    #[inline]
    pub fn as_const<'a>(&'a self) -> BitmapRef<'a> {
        match self {
            OwnedBitmap::Indexed(v) => BitmapRef::Indexed(v.as_ref()),
            OwnedBitmap::Argb32(v) => BitmapRef::Argb32(v.as_ref()),
        }
    }
}

impl From<OwnedBitmap8> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap8) -> Self {
        Self::Indexed(val)
    }
}

impl From<OwnedBitmap32> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap32) -> Self {
        Self::Argb32(val)
    }
}

impl<'a> AsMut<BitmapRefMut<'a>> for OwnedBitmap {
    #[inline]
    fn as_mut(&mut self) -> &mut BitmapRefMut<'a> {
        unsafe { transmute(self) }
    }
}

/// A special bitmap type that can be used for operations such as transparency and shading.
pub struct OperationalBitmap {
    size: Size,
    vec: Vec<u8>,
}

impl PixelColor for u8 {}

impl Image for OperationalBitmap {
    type ColorType = u8;

    #[inline]
    fn size(&self) -> Size {
        self.size
    }
}

impl RasterImage for OperationalBitmap {
    #[inline]
    fn stride(&self) -> usize {
        self.width() as usize
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        self.vec.as_slice()
    }
}

impl MutableRasterImage for OperationalBitmap {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.vec.as_mut_slice()
    }
}

impl OperationalBitmap {
    #[inline]
    pub const fn new(size: Size) -> Self {
        let vec = Vec::new();
        Self { size, vec }
    }

    #[inline]
    pub fn from_slice(slice: &[u8], size: Size) -> Self {
        let vec = Vec::from(slice);
        Self { size, vec }
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>, size: Size) -> Self {
        Self { size, vec }
    }

    #[inline]
    pub fn from_pixels<F, T>(data: &T, mut kernel: F) -> OperationalBitmap
    where
        T: GetPixel + ?Sized,
        F: FnMut(<T as Image>::ColorType) -> u8,
    {
        let size = data.size();
        let vec = data.all_pixels().map(|v| kernel(v)).collect();
        OperationalBitmap { size, vec }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.fill(0);
    }

    pub fn fill(&mut self, color: u8) {
        let count = self.stride() * self.height() as usize;
        if self.vec.capacity() >= count {
            self.vec.fill(color);
        } else {
            self.vec.resize(count, color);
        }
    }

    /// Draws a straight line at high speed using Bresenham's line algorithm.
    #[inline]
    pub fn draw_line<F>(&mut self, c1: Point, c2: Point, mut kernel: F)
    where
        F: FnMut(&mut OperationalBitmap, Point),
    {
        c1.line_to(c2, |point| kernel(self, point));
    }

    /// Draws an anti-aliased line using Xiaolin Wu's algorithm.
    pub fn draw_line_anti_aliasing_f<F>(&mut self, c1: Point, c2: Point, mut kernel: F)
    where
        F: FnMut(&mut OperationalBitmap, Point, f64),
    {
        let mut plot = |bitmap: &mut OperationalBitmap, x: f64, y: f64, level: f64| {
            kernel(bitmap, Point::new(x as GlSInt, y as GlSInt), level);
        };
        #[inline]
        fn ipart(v: f64) -> f64 {
            libm::floor(v)
        }
        #[inline]
        fn fpart(v: f64) -> f64 {
            v - ipart(v)
        }
        #[inline]
        fn rfpart(v: f64) -> f64 {
            1.0 - fpart(v)
        }

        let mut x1 = c1.x as f64;
        let mut x2 = c2.x as f64;
        let mut y1 = c1.y as f64;
        let mut y2 = c2.y as f64;

        let width = f64::max(x1, x2) - f64::min(x1, x2);
        let height = f64::max(y1, y2) - f64::min(y1, y2);
        let steep = height > width;

        if steep {
            swap(&mut x1, &mut y1);
            swap(&mut x2, &mut y2);
        }
        if x1 > x2 {
            swap(&mut x1, &mut x2);
            swap(&mut y1, &mut y2);
        }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

        let xend = libm::round(x1);
        let yend = y1 + gradient * (xend - x1);
        let xgap = rfpart(x1 + 0.5);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            plot(self, ypxl1, xpxl1, rfpart(yend) * xgap);
            plot(self, ypxl1 + 1.0, xpxl1, fpart(yend) * xgap);
        } else {
            plot(self, xpxl1, ypxl1, rfpart(yend) * xgap);
            plot(self, xpxl1, ypxl1 + 1.0, fpart(yend) * xgap);
        }
        let mut intery = yend + gradient;

        let xend = libm::round(x2);
        let yend = y2 + gradient * (xend - x2);
        let xgap = fpart(x2 + 0.5);
        let xpxl2 = xend;
        let ypxl2 = ipart(yend);
        if steep {
            plot(self, ypxl2, xpxl2, rfpart(yend) * xgap);
            plot(self, ypxl2 + 1.0, xpxl2, fpart(yend) * xgap);
        } else {
            plot(self, xpxl2, ypxl2, rfpart(yend) * xgap);
            plot(self, xpxl2, ypxl2 + 1.0, fpart(yend) * xgap);
        }

        if steep {
            for i in (xpxl1 as isize + 1)..(xpxl2 as isize) {
                let y = i as f64;
                plot(self, intery, y, rfpart(intery));
                plot(self, intery + 1.0, y, fpart(intery));
                intery += gradient;
            }
        } else {
            for i in (xpxl1 as isize + 1)..(xpxl2 as isize) {
                let x = i as f64;
                plot(self, x, intery, rfpart(intery));
                plot(self, x, intery + 1.0, fpart(intery));
                intery += gradient;
            }
        }
    }

    #[deprecated]
    pub fn draw_line_anti_aliasing_i<F>(
        &mut self,
        c1: Point,
        c2: Point,
        scale: GlSInt,
        mut kernel: F,
    ) where
        F: FnMut(&mut OperationalBitmap, Point, u8),
    {
        const FRAC_SHIFT: u32 = 6;
        const ONE: GlSInt = 1 << FRAC_SHIFT;
        const FRAC_MASK: GlSInt = ONE - 1;
        const FRAC_HALF: GlSInt = ONE / 2;
        const IPART_MASK: GlSInt = !FRAC_MASK;

        let mut plot = |bitmap: &mut OperationalBitmap, x: GlSInt, y: GlSInt, level: GlSInt| {
            kernel(
                bitmap,
                Point::new(x >> FRAC_SHIFT, y >> FRAC_SHIFT),
                (0xFF * level >> FRAC_SHIFT) as u8,
            );
        };
        #[inline]
        fn ipart(v: GlSInt) -> GlSInt {
            v & IPART_MASK
        }
        #[inline]
        fn round(v: GlSInt) -> GlSInt {
            ipart(v + FRAC_HALF)
        }
        #[inline]
        fn fpart(v: GlSInt) -> GlSInt {
            v & FRAC_MASK
        }
        #[inline]
        fn rfpart(v: GlSInt) -> GlSInt {
            ONE - fpart(v)
        }
        #[inline]
        fn mul(a: GlSInt, b: GlSInt) -> GlSInt {
            (a * b) >> FRAC_SHIFT
        }
        #[inline]
        fn div(a: GlSInt, b: GlSInt) -> Option<GlSInt> {
            (a << FRAC_SHIFT).checked_div(b)
        }

        let mut x1 = (c1.x() << FRAC_SHIFT) / scale;
        let mut x2 = (c2.x() << FRAC_SHIFT) / scale;
        let mut y1 = (c1.y() << FRAC_SHIFT) / scale;
        let mut y2 = (c2.y() << FRAC_SHIFT) / scale;

        let width = x1.max(x2) - x1.min(x2);
        let height = y1.max(y2) - y1.min(y2);
        let steep = height > width;

        if steep {
            swap(&mut x1, &mut y1);
            swap(&mut x2, &mut y2);
        }
        if x1 > x2 {
            swap(&mut x1, &mut x2);
            swap(&mut y1, &mut y2);
        }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let gradient = div(dy, dx).unwrap_or(ONE);
        //if dx == 0 { ONE } else { div(dy, dx) };

        let xend = round(x1);
        let yend = y1 + mul(gradient, xend - x1);
        let xgap = rfpart(x1 + FRAC_HALF);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            plot(self, ypxl1, xpxl1, mul(rfpart(yend), xgap));
            plot(self, ypxl1 + ONE, xpxl1, mul(fpart(yend), xgap));
        } else {
            plot(self, xpxl1, ypxl1, mul(rfpart(yend), xgap));
            plot(self, xpxl1, ypxl1 + ONE, mul(fpart(yend), xgap));
        }
        let mut intery = yend + gradient;

        let xend = round(x2);
        let yend = y2 + mul(gradient, xend - x2);
        let xgap = fpart(x2 + FRAC_HALF);
        let xpxl2 = xend;
        let ypxl2 = ipart(yend);
        if steep {
            plot(self, ypxl2, xpxl2, mul(rfpart(yend), xgap));
            plot(self, ypxl2 + ONE, xpxl2, mul(fpart(yend), xgap));
        } else {
            plot(self, xpxl2, ypxl2, mul(rfpart(yend), xgap));
            plot(self, xpxl2, ypxl2 + ONE, mul(fpart(yend), xgap));
        }

        if steep {
            for i in (xpxl1 >> FRAC_SHIFT) + 1..(xpxl2 >> FRAC_SHIFT) {
                let y = i << FRAC_SHIFT;
                plot(self, intery, y, rfpart(intery));
                plot(self, intery + ONE, y, fpart(intery));
                intery += gradient;
            }
        } else {
            for i in (xpxl1 >> FRAC_SHIFT) + 1..(xpxl2 >> FRAC_SHIFT) {
                let x = i << FRAC_SHIFT;
                plot(self, x, intery, rfpart(intery));
                plot(self, x, intery + ONE, fpart(intery));
                intery += gradient;
            }
        }
    }

    pub fn draw_polygon<T>(&mut self, polygon: &[Vec2<T>], color: u8)
    where
        Vec2<T>: Into<Vec2<GlSInt>>,
        T: Copy,
    {
        let mut polygon = polygon.iter();
        let len = polygon.len();
        let Some(vertex0) = polygon.next() else {
            return;
        };
        let mut vertex1 = vertex0;
        while let Some(vertex2) = polygon.next() {
            self.draw_line_anti_aliasing_f(
                (*vertex1).into(),
                (*vertex2).into(),
                |bitmap, point, level| unsafe {
                    bitmap.process_pixel_unchecked(point, |c| {
                        c.saturating_add((color as f64 * level) as u8)
                    });
                },
            );
            vertex1 = vertex2;
        }
        if len > 2 {
            self.draw_line_anti_aliasing_f(
                (*vertex0).into(),
                (*vertex1).into(),
                |bitmap, point, level| unsafe {
                    bitmap.process_pixel_unchecked(point, |c| {
                        c.saturating_add((color as f64 * level) as u8)
                    });
                },
            );
        }
    }

    /// Like box linear filter
    pub fn blur(&mut self, radius: GlUInt, level: usize) {
        let bounds = self.bounds();
        let length = radius * 2 + 1;

        for y in (length..bounds.height()).rev() {
            for x in 0..bounds.width() {
                let mut acc = 0;
                for r in 0..length {
                    acc += unsafe {
                        self.get_pixel_unchecked(Point::new(x as GlSInt, (y - r) as GlSInt))
                            as usize
                    };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x as GlSInt, y as GlSInt),
                        (acc / length as usize) as u8,
                    );
                }
            }
        }
        for y in (0..length).rev() {
            for x in 0..bounds.width() {
                let mut acc = 0;
                for r in 0..y {
                    acc += unsafe {
                        self.get_pixel_unchecked(Point::new(x as GlSInt, (y - r) as GlSInt))
                            as usize
                    };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x as GlSInt, y as GlSInt),
                        (acc / length as usize) as u8,
                    );
                }
            }
        }

        for y in 0..bounds.height() {
            for x in (length..bounds.width()).rev() {
                let mut acc = 0;
                for r in 0..length {
                    acc += unsafe {
                        self.get_pixel_unchecked(Point::new((x - r) as GlSInt, y as GlSInt))
                            as usize
                    };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x as GlSInt, y as GlSInt),
                        usize::min(255, (acc / length as usize) * level / 256) as u8,
                    );
                }
            }
            for x in (0..length).rev() {
                let mut acc = 0;
                for r in 0..x {
                    acc += unsafe {
                        self.get_pixel_unchecked(Point::new((x - r) as GlSInt, y as GlSInt))
                            as usize
                    };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x as GlSInt, y as GlSInt),
                        usize::min(255, (acc / length as usize) * level / 256) as u8,
                    );
                }
            }
        }
    }

    pub fn blt_to<T, F>(&self, dest: &mut T, origin: Point, rect: Rect, mut kernel: F)
    where
        T: Image + GetPixelMut,
        F: FnMut(u8, <T as Image>::ColorType) -> <T as Image>::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(dest.size(), self.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let dp = Point::new(dx + x, dy + y);
                let sp = Point::new(sx + x, sy + y);
                unsafe {
                    let p = dest.get_pixel_unchecked_mut(dp);
                    *p = kernel(self.get_pixel_unchecked(sp), *p);
                }
            }
        }
    }

    #[inline]
    pub fn blt_shadow(&self, dest: &mut BitmapRefMut32, origin: Point, rect: Rect) {
        self.blt_to(dest, origin, rect, |a, b| b.shadowed(a))
    }

    pub fn blt_from<T, F>(&mut self, src: &T, origin: Point, rect: Rect, mut kernel: F)
    where
        T: GetPixel,
        F: FnMut(<T as Image>::ColorType, u8) -> u8,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let dp = Point::new(dx + x, dy + y);
                let sp = Point::new(sx + x, sy + y);
                unsafe {
                    let p = self.get_pixel_unchecked_mut(dp);
                    *p = kernel(src.get_pixel_unchecked(sp), *p);
                }
            }
        }
    }

    pub fn draw_to(&self, dest: &mut BitmapRefMut, origin: Point, rect: Rect, color: Color) {
        match dest {
            BitmapRefMut::Indexed(_) => {
                // TODO:
            }
            BitmapRefMut::Argb32(bitmap) => {
                let color = color.into_true_color();
                self.blt_to(bitmap, origin, rect, |a, b| {
                    let mut c = color.components();
                    c.a = Alpha8::new(a);
                    b.blending(c.into())
                });
            }
        }
    }
}

/// Adjust the coordinates for blt.
///
/// Returns the adjusted destination x, y, source x, y, width and height.
fn _adjust_blt_coords(
    dest_size: Size,
    src_size: Size,
    origin: Point,
    rect: Rect,
) -> (GlSInt, GlSInt, GlSInt, GlSInt, GlSInt, GlSInt) {
    let mut dx = origin.x;
    let mut dy = origin.y;
    let mut sx = rect.min_x();
    let mut sy = rect.min_y();
    let mut width = safe_to_int(rect.width());
    let mut height = safe_to_int(rect.height());
    let dw = safe_to_int(dest_size.width());
    let dh = safe_to_int(dest_size.height());
    let sw = safe_to_int(src_size.width());
    let sh = safe_to_int(src_size.height());

    if sx < 0 {
        dx = dx.saturating_sub(sx);
        width = width.saturating_add(sx);
        sx = 0;
    }
    if sy < 0 {
        dy = dy.saturating_sub(sy);
        height = height.saturating_add(sy);
        sy = 0;
    }
    if dx < 0 {
        sx = sx.saturating_sub(dx);
        width = width.saturating_add(dx);
        dx = 0;
    }
    if dy < 0 {
        sy = sy.saturating_sub(dy);
        height = height.saturating_add(dy);
        dy = 0;
    }
    if sx.saturating_add(width) > sw {
        width = sw.saturating_sub(sx);
    }
    if sy.saturating_add(height) > sh {
        height = sh.saturating_sub(sy);
    }
    if dx.saturating_add(width) >= dw {
        width = dw.saturating_sub(dx);
    }
    if dy.saturating_add(height) >= dh {
        height = dh.saturating_sub(dy);
    }

    (dx, dy, sx, sy, width, height)
}

mod memory_colors {
    use super::*;

    #[inline]
    pub fn _memset_colors8(
        slice: &mut [IndexedColor],
        cursor: usize,
        count: usize,
        color: IndexedColor,
    ) {
        unsafe {
            slice.get_unchecked_mut(cursor..cursor + count).fill(color);
        }
    }

    #[inline]
    pub fn _memset_colors16(slice: &mut [RGB565], cursor: usize, count: usize, color: RGB565) {
        unsafe {
            slice.get_unchecked_mut(cursor..cursor + count).fill(color);
        }
    }

    #[inline]
    pub fn _memset_colors32(slice: &mut [ARGB8888], cursor: usize, count: usize, color: ARGB8888) {
        unsafe {
            slice.get_unchecked_mut(cursor..cursor + count).fill(color);
        }
    }

    // Alpha blending
    #[inline(never)]
    pub fn _memcpy_blend32(
        dest: &mut [ARGB8888],
        dest_cursor: usize,
        src: &[ARGB8888],
        src_cursor: usize,
        count: usize,
    ) {
        let dest = unsafe { &mut dest.get_unchecked_mut(dest_cursor..dest_cursor + count) };
        let src = unsafe { &src.get_unchecked(src_cursor..src_cursor + count) };
        for (dest, src) in dest.iter_mut().zip(src.iter()) {
            dest.blend(*src);
        }
    }
}

define_bitmap!(1, u8, Monochrome, Octet,);

impl BitmapRef1<'_> {
    #[inline]
    fn slice(&self) -> &[Octet] {
        self.slice
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl GetPixel for BitmapRef1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        let bit_position = point.x as usize & 7;
        let index = ((point.x as usize) >> 3) + self.stride * point.y as usize;
        self.slice.get_unchecked(index).get(bit_position)
    }
}

impl BitmapRefMut1<'_> {
    #[inline]
    fn slice(&self) -> &[Octet] {
        unsafe { &mut *self.slice.get() }
    }

    #[inline]
    fn slice_mut(&mut self) -> &mut [Octet] {
        self.slice.get_mut()
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }

    pub fn copy_from<'a, T: AsRef<BitmapRef1<'a>>>(&mut self, other: &T) {
        unsafe {
            let p = self.slice_mut();
            let q = other.as_ref().slice();
            let count = p.len();
            copy_nonoverlapping(q.as_ptr(), p.as_mut_ptr(), count);
        }
    }
}

impl GetPixel for BitmapRefMut1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        self.as_ref().get_pixel_unchecked(point)
    }
}

impl SetPixel for BitmapRefMut1<'_> {
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        let bit_position = point.x as usize & 7;
        let index = ((point.x as usize) >> 3) + self.stride * point.y as usize;
        self.slice
            .get_mut()
            .get_unchecked_mut(index)
            .set(bit_position, pixel);
    }
}

define_bitmap!(4, u8, IndexedColor4, IndexedColorPair44,);

impl BitmapRef4<'_> {
    #[inline]
    pub fn slice(&self) -> &[IndexedColorPair44] {
        self.slice
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl BitmapRefMut4<'_> {
    #[inline]
    pub fn slice(&self) -> &[IndexedColorPair44] {
        unsafe { &*self.slice.get() }
    }

    #[inline]
    pub fn slice_mut(&mut self) -> &mut [IndexedColorPair44] {
        self.slice.get_mut()
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl GetPixel for BitmapRef4<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        let x = point.x as usize;
        let y = point.y as usize;
        let pair = self.slice.get_unchecked(y * self.stride + x / 2);
        if (x & 1) == 0 {
            pair.lhs()
        } else {
            pair.rhs()
        }
    }
}
