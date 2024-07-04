use crate::vec::{Vec3, Vec4};
use crate::*;
use core::fmt;
use core::mem::transmute;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Common color trait
pub trait PixelColor: Sized + Copy + Clone + PartialEq + Eq + Default {
    /// This value is used to calculate the address of a raster image that supports this color format.
    #[inline]
    fn stride_for(width: GlUInt) -> usize {
        width as usize
    }
}

pub trait Translucent: PixelColor {
    const TRANSPARENT: Self;

    fn is_transparent(&self) -> bool;

    fn is_opaque(&self) -> bool;
}

pub trait KeyColor: PixelColor {
    const KEY_COLOR: Self;
}

pub trait PrimaryColor: PixelColor {
    /// RGB (0, 0, 0)
    const PRIMARY_BLACK: Self;
    /// RGB (0, 0, 1)
    const PRIMARY_BLUE: Self;
    /// RGB (0, 1, 0)
    const PRIMARY_GREEN: Self;
    /// RGB (0, 1, 1)
    const PRIMARY_CYAN: Self;
    /// RGB (1, 0, 0)
    const PRIMARY_RED: Self;
    /// RGB (1, 0, 1)
    const PRIMARY_MAGENTA: Self;
    /// RGB (1, 1, 0)
    const PRIMARY_YELLOW: Self;
    /// RGB (1, 1, 1)
    const PRIMARY_WHITE: Self;
}

pub enum ColorFormat {
    ARGB,
    RGBA,
    ABGR,
    BGRA,
    XRGB,
    RGBX,
    XBGR,
    BGRX,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct IndexedColor(pub u8);

impl PixelColor for IndexedColor {}

impl KeyColor for IndexedColor {
    const KEY_COLOR: Self = Self(u8::MAX);
}

impl PrimaryColor for IndexedColor {
    const PRIMARY_BLACK: Self = Self::from_rgb(0x00_00_00);
    const PRIMARY_BLUE: Self = Self::from_rgb(0x00_00_FF);
    const PRIMARY_GREEN: Self = Self::from_rgb(0x00_FF_00);
    const PRIMARY_CYAN: Self = Self::from_rgb(0x00_FF_FF);
    const PRIMARY_RED: Self = Self::from_rgb(0xFF_00_00);
    const PRIMARY_MAGENTA: Self = Self::from_rgb(0xFF_00_FF);
    const PRIMARY_YELLOW: Self = Self::from_rgb(0xFF_FF_00);
    const PRIMARY_WHITE: Self = Self::from_rgb(0xFF_FF_FF);
}

impl IndexedColor {
    pub const MIN: Self = Self(u8::MIN);
    pub const MAX: Self = Self(u8::MAX);

    pub const BLACK: Self = Self(0);
    pub const BLUE: Self = Self(1);
    pub const GREEN: Self = Self(2);
    pub const CYAN: Self = Self(3);
    pub const RED: Self = Self(4);
    pub const MAGENTA: Self = Self(5);
    pub const BROWN: Self = Self(6);
    pub const LIGHT_GRAY: Self = Self(7);
    pub const DARK_GRAY: Self = Self(8);
    pub const LIGHT_BLUE: Self = Self(9);
    pub const LIGHT_GREEN: Self = Self(10);
    pub const LIGHT_CYAN: Self = Self(11);
    pub const LIGHT_RED: Self = Self(12);
    pub const LIGHT_MAGENTA: Self = Self(13);
    pub const YELLOW: Self = Self(14);
    pub const WHITE: Self = Self(15);

    pub const COLOR_PALETTE: [u32; 256] = [
        0xFF000000, 0xFF0000AA, 0xFF00AA00, 0xFF00AAAA, 0xFFAA0000, 0xFFAA00AA, 0xFFAA5500,
        0xFFAAAAAA, 0xFF555555, 0xFF5555FF, 0xFF55FF55, 0xFF55FFFF, 0xFFFF5555, 0xFFFF55FF,
        0xFFFFFF55, 0xFFFFFFFF, 0xFF000000, 0xFF330000, 0xFF660000, 0xFF990000, 0xFFCC0000,
        0xFFFF0000, 0xFF003300, 0xFF333300, 0xFF663300, 0xFF993300, 0xFFCC3300, 0xFFFF3300,
        0xFF006600, 0xFF336600, 0xFF666600, 0xFF996600, 0xFFCC6600, 0xFFFF6600, 0xFF009900,
        0xFF339900, 0xFF669900, 0xFF999900, 0xFFCC9900, 0xFFFF9900, 0xFF00CC00, 0xFF33CC00,
        0xFF66CC00, 0xFF99CC00, 0xFFCCCC00, 0xFFFFCC00, 0xFF00FF00, 0xFF33FF00, 0xFF66FF00,
        0xFF99FF00, 0xFFCCFF00, 0xFFFFFF00, 0xFF000033, 0xFF330033, 0xFF660033, 0xFF990033,
        0xFFCC0033, 0xFFFF0033, 0xFF003333, 0xFF333333, 0xFF663333, 0xFF993333, 0xFFCC3333,
        0xFFFF3333, 0xFF006633, 0xFF336633, 0xFF666633, 0xFF996633, 0xFFCC6633, 0xFFFF6633,
        0xFF009933, 0xFF339933, 0xFF669933, 0xFF999933, 0xFFCC9933, 0xFFFF9933, 0xFF00CC33,
        0xFF33CC33, 0xFF66CC33, 0xFF99CC33, 0xFFCCCC33, 0xFFFFCC33, 0xFF00FF33, 0xFF33FF33,
        0xFF66FF33, 0xFF99FF33, 0xFFCCFF33, 0xFFFFFF33, 0xFF000066, 0xFF330066, 0xFF660066,
        0xFF990066, 0xFFCC0066, 0xFFFF0066, 0xFF003366, 0xFF333366, 0xFF663366, 0xFF993366,
        0xFFCC3366, 0xFFFF3366, 0xFF006666, 0xFF336666, 0xFF666666, 0xFF996666, 0xFFCC6666,
        0xFFFF6666, 0xFF009966, 0xFF339966, 0xFF669966, 0xFF999966, 0xFFCC9966, 0xFFFF9966,
        0xFF00CC66, 0xFF33CC66, 0xFF66CC66, 0xFF99CC66, 0xFFCCCC66, 0xFFFFCC66, 0xFF00FF66,
        0xFF33FF66, 0xFF66FF66, 0xFF99FF66, 0xFFCCFF66, 0xFFFFFF66, 0xFF000099, 0xFF330099,
        0xFF660099, 0xFF990099, 0xFFCC0099, 0xFFFF0099, 0xFF003399, 0xFF333399, 0xFF663399,
        0xFF993399, 0xFFCC3399, 0xFFFF3399, 0xFF006699, 0xFF336699, 0xFF666699, 0xFF996699,
        0xFFCC6699, 0xFFFF6699, 0xFF009999, 0xFF339999, 0xFF669999, 0xFF999999, 0xFFCC9999,
        0xFFFF9999, 0xFF00CC99, 0xFF33CC99, 0xFF66CC99, 0xFF99CC99, 0xFFCCCC99, 0xFFFFCC99,
        0xFF00FF99, 0xFF33FF99, 0xFF66FF99, 0xFF99FF99, 0xFFCCFF99, 0xFFFFFF99, 0xFF0000CC,
        0xFF3300CC, 0xFF6600CC, 0xFF9900CC, 0xFFCC00CC, 0xFFFF00CC, 0xFF0033CC, 0xFF3333CC,
        0xFF6633CC, 0xFF9933CC, 0xFFCC33CC, 0xFFFF33CC, 0xFF0066CC, 0xFF3366CC, 0xFF6666CC,
        0xFF9966CC, 0xFFCC66CC, 0xFFFF66CC, 0xFF0099CC, 0xFF3399CC, 0xFF6699CC, 0xFF9999CC,
        0xFFCC99CC, 0xFFFF99CC, 0xFF00CCCC, 0xFF33CCCC, 0xFF66CCCC, 0xFF99CCCC, 0xFFCCCCCC,
        0xFFFFCCCC, 0xFF00FFCC, 0xFF33FFCC, 0xFF66FFCC, 0xFF99FFCC, 0xFFCCFFCC, 0xFFFFFFCC,
        0xFF0000FF, 0xFF3300FF, 0xFF6600FF, 0xFF9900FF, 0xFFCC00FF, 0xFFFF00FF, 0xFF0033FF,
        0xFF3333FF, 0xFF6633FF, 0xFF9933FF, 0xFFCC33FF, 0xFFFF33FF, 0xFF0066FF, 0xFF3366FF,
        0xFF6666FF, 0xFF9966FF, 0xFFCC66FF, 0xFFFF66FF, 0xFF0099FF, 0xFF3399FF, 0xFF6699FF,
        0xFF9999FF, 0xFFCC99FF, 0xFFFF99FF, 0xFF00CCFF, 0xFF33CCFF, 0xFF66CCFF, 0xFF99CCFF,
        0xFFCCCCFF, 0xFFFFCCFF, 0xFF00FFFF, 0xFF33FFFF, 0xFF66FFFF, 0xFF99FFFF, 0xFFCCFFFF,
        0xFFFFFFFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        let b = (((rgb & 0xFF) + 25) / 51) as u8;
        let g = ((((rgb >> 8) & 0xFF) + 25) / 51) as u8;
        let r = ((((rgb >> 16) & 0xFF) + 25) / 51) as u8;
        Self(16 + r + g * 6 + b * 36)
    }

    #[inline]
    pub const fn as_rgb(self) -> u32 {
        Self::COLOR_PALETTE[self.0 as usize] & 0xFF_FF_FF
    }

    #[inline]
    pub const fn as_argb(self) -> u32 {
        Self::COLOR_PALETTE[self.0 as usize]
    }

    #[inline]
    pub const fn as_true_color(self) -> ARGB8888 {
        ARGB8888::from_argb(self.as_argb())
    }

    #[inline]
    pub const fn brightness(self) -> Option<u8> {
        self.as_true_color().brightness()
    }
}

impl From<u8> for IndexedColor {
    #[inline]
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<IndexedColor> for ARGB8888 {
    #[inline]
    fn from(val: IndexedColor) -> Self {
        val.as_true_color()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Alpha8(u8);

impl PixelColor for Alpha8 {}

impl Translucent for Alpha8 {
    const TRANSPARENT: Self = Self(0);

    #[inline]
    fn is_transparent(&self) -> bool {
        self.0 == Self::TRANSPARENT.0
    }

    #[inline]
    fn is_opaque(&self) -> bool {
        self.0 == Self::OPAQUE.0
    }
}

impl Alpha8 {
    pub const OPAQUE: Self = Self(u8::MAX);

    #[inline]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_u8(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn into_f32(self) -> f32 {
        self.0 as f32 / Self::OPAQUE.0 as f32
    }

    #[inline]
    pub fn into_f64(self) -> f64 {
        self.0 as f64 / Self::OPAQUE.0 as f64
    }

    #[inline]
    pub fn from_f32(value: f32) -> Self {
        Self((value * 255.0).clamp(0.0, 255.0) as u8)
    }

    #[inline]
    pub fn from_f64(value: f64) -> Self {
        Self((value * 255.0).clamp(0.0, 255.0) as u8)
    }

    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    #[inline]
    pub const fn is_transparent(&self) -> bool {
        self.0 == Self::TRANSPARENT.0
    }

    #[inline]
    pub const fn is_opaque(&self) -> bool {
        self.0 == Self::OPAQUE.0
    }
}

impl Default for Alpha8 {
    #[inline]
    fn default() -> Self {
        Self::OPAQUE
    }
}

impl From<u8> for Alpha8 {
    #[inline]
    fn from(value: u8) -> Self {
        Alpha8(value)
    }
}

impl From<Alpha8> for u8 {
    #[inline]
    fn from(value: Alpha8) -> Self {
        value.0
    }
}

impl From<f32> for Alpha8 {
    #[inline]
    fn from(value: f32) -> Self {
        Self::from_f32(value)
    }
}

impl From<Alpha8> for f32 {
    #[inline]
    fn from(value: Alpha8) -> Self {
        value.into_f32()
    }
}

impl From<f64> for Alpha8 {
    #[inline]
    fn from(value: f64) -> Self {
        Self::from_f64(value)
    }
}

impl From<Alpha8> for f64 {
    #[inline]
    fn from(value: Alpha8) -> Self {
        value.into_f64()
    }
}

impl Add<Self> for Alpha8 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign<Self> for Alpha8 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}

impl Sub<Self> for Alpha8 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self.saturating_sub(rhs)
    }
}

impl SubAssign<Self> for Alpha8 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.saturating_sub(rhs);
    }
}

impl Add<u8> for Alpha8 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u8) -> Self::Output {
        self.saturating_add(Self(rhs))
    }
}

impl AddAssign<u8> for Alpha8 {
    #[inline]
    fn add_assign(&mut self, rhs: u8) {
        *self = self.saturating_add(Self(rhs));
    }
}

impl Sub<u8> for Alpha8 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u8) -> Self::Output {
        self.saturating_sub(Self(rhs))
    }
}

impl SubAssign<u8> for Alpha8 {
    #[inline]
    fn sub_assign(&mut self, rhs: u8) {
        *self = self.saturating_sub(Self(rhs));
    }
}

pub type TrueColor = ARGB8888;

/// 32bit TrueColor
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct ARGB8888(u32);

impl PixelColor for ARGB8888 {}

impl Translucent for ARGB8888 {
    const TRANSPARENT: Self = Self(0);

    #[inline]
    fn is_transparent(&self) -> bool {
        self.opacity().is_transparent()
    }

    #[inline]
    fn is_opaque(&self) -> bool {
        self.opacity().is_opaque()
    }
}

impl PrimaryColor for ARGB8888 {
    const PRIMARY_BLACK: Self = Self::from_rgb(0x00_00_00);
    const PRIMARY_BLUE: Self = Self::from_rgb(0x00_00_FF);
    const PRIMARY_GREEN: Self = Self::from_rgb(0x00_FF_00);
    const PRIMARY_CYAN: Self = Self::from_rgb(0x00_FF_FF);
    const PRIMARY_RED: Self = Self::from_rgb(0xFF_00_00);
    const PRIMARY_MAGENTA: Self = Self::from_rgb(0xFF_00_FF);
    const PRIMARY_YELLOW: Self = Self::from_rgb(0xFF_FF_00);
    const PRIMARY_WHITE: Self = Self::from_rgb(0xFF_FF_FF);
}

impl ARGB8888 {
    pub const BLACK: Self = Self::from_rgb(0x212121);
    pub const BLUE: Self = Self::from_rgb(0x0D47A1);
    pub const GREEN: Self = Self::from_rgb(0x1B5E20);
    pub const CYAN: Self = Self::from_rgb(0x006064);
    pub const RED: Self = Self::from_rgb(0xB71C1C);
    pub const MAGENTA: Self = Self::from_rgb(0x4A148C);
    pub const BROWN: Self = Self::from_rgb(0x795548);
    pub const LIGHT_GRAY: Self = Self::from_rgb(0xBDBDBD);
    pub const DARK_GRAY: Self = Self::from_rgb(0x616161);
    pub const LIGHT_BLUE: Self = Self::from_rgb(0x2196F3);
    pub const LIGHT_GREEN: Self = Self::from_rgb(0x4CAF50);
    pub const LIGHT_CYAN: Self = Self::from_rgb(0x00BCD4);
    pub const LIGHT_RED: Self = Self::from_rgb(0xF44336);
    pub const LIGHT_MAGENTA: Self = Self::from_rgb(0x9C27B0);
    pub const YELLOW: Self = Self::from_rgb(0xFFEB3B);
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self(rgb | 0xFF000000)
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self(argb)
    }

    #[inline]
    pub const fn from_gray(white: u8, alpha: Alpha8) -> Self {
        Self(white as u32 * 0x00_01_01_01 + alpha.0 as u32 * 0x01_00_00_00)
    }

    #[inline]
    #[cfg(target_endian = "little")]
    pub const fn components(&self) -> ColorComponents {
        ColorComponents::from_true_color(*self)
    }

    #[inline]
    pub const fn rgb(&self) -> u32 {
        self.0 & 0x00FFFFFF
    }

    #[inline]
    pub const fn argb(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn brightness(&self) -> Option<u8> {
        let cc = self.components();
        if cc.is_transparent() {
            None
        } else {
            Some(
                ((cc.r as usize * 19589 + cc.g as usize * 38444 + cc.b as usize * 7502 + 32767)
                    >> 16) as u8,
            )
        }
    }

    #[inline]
    pub const fn opacity(&self) -> Alpha8 {
        ColorComponents::from_true_color(*self).a
    }

    #[inline]
    pub const fn with_opacity(&self, alpha: Alpha8) -> Self {
        let mut components = self.components();
        components.a = alpha;
        components.into_true_color()
    }

    #[inline]
    pub fn shadowed(&self, shadow: u8) -> Self {
        let shadow = 256 - shadow as u32;
        let r = (((self.0 & 0x00FF0000) * shadow) / 256) & 0x00FF0000;
        let g = (((self.0 & 0x0000FF00) * shadow) / 256) & 0x0000FF00;
        let b = (((self.0 & 0x000000FF) * shadow) / 256) & 0x000000FF;
        let argb = (self.0 & 0xFF000000) | r | g | b;
        Self(argb)
    }

    pub fn blending(&self, rhs: Self) -> Self {
        let rhs_ = rhs.components();
        if rhs_.a.is_opaque() {
            return rhs;
        }
        if rhs_.a.is_transparent() {
            return *self;
        }
        let lhs_ = self.components();
        let alpha_r = rhs_.a.0 as usize;
        let alpha_l = lhs_.a.0 as usize * (256 - alpha_r) / 256;
        let alpha_s = alpha_r + alpha_l;
        let alpha_ls = (alpha_l * 256).checked_div(alpha_s).unwrap_or(0) as u32;
        let alpha_rs = (alpha_r * 256).checked_div(alpha_s).unwrap_or(0) as u32;

        let l_rb = self.0 & 0xFF00FF;
        let l_g = self.0 & 0x00FF00;
        let r_rb = rhs.0 & 0xFF00FF;
        let r_g = rhs.0 & 0x00FF00;

        Self(
            (((((l_rb * alpha_ls) + (r_rb * alpha_rs)) & 0xFF00FF00)
                + (((l_g * alpha_ls) + (r_g * alpha_rs)) & 0x00FF0000))
                >> 8)
                + ((alpha_s as u32) << 24),
        )
    }

    #[inline]
    pub fn blend(&mut self, rhs: Self) {
        *self = self.blending(rhs);
    }

    #[inline]
    pub const fn is_transparent(&self) -> bool {
        self.opacity().is_transparent()
    }

    #[inline]
    pub const fn is_opaque(&self) -> bool {
        self.opacity().is_opaque()
    }
}

impl From<u32> for ARGB8888 {
    #[inline]
    fn from(argb: u32) -> Self {
        Self::from_argb(argb)
    }
}

impl From<ARGB8888> for IndexedColor {
    #[inline]
    fn from(color: ARGB8888) -> Self {
        Self::from_rgb(color.rgb())
    }
}

impl From<Vec3<u8>> for ARGB8888 {
    #[inline]
    fn from(value: Vec3<u8>) -> Self {
        ColorComponents::from_rgb(value.x, value.y, value.z).into_true_color()
    }
}

impl From<Vec4<u8>> for ARGB8888 {
    #[inline]
    fn from(value: Vec4<u8>) -> Self {
        ColorComponents::from_rgba(value.x, value.y, value.z, Alpha8::new(value.w))
            .into_true_color()
    }
}

impl From<Vec3<f64>> for ARGB8888 {
    #[inline]
    fn from(value: Vec3<f64>) -> Self {
        ColorComponents::from_rgb(
            (value.x * 255.99).clamp(0.0, 255.0) as u8,
            (value.y * 255.99).clamp(0.0, 255.0) as u8,
            (value.z * 255.99).clamp(0.0, 255.0) as u8,
        )
        .into_true_color()
    }
}

impl From<Vec4<f64>> for ARGB8888 {
    #[inline]
    fn from(value: Vec4<f64>) -> Self {
        ColorComponents::from_rgba(
            (value.x * 255.99).clamp(0.0, 255.0) as u8,
            (value.y * 255.99).clamp(0.0, 255.0) as u8,
            (value.z * 255.99).clamp(0.0, 255.0) as u8,
            Alpha8::from_f64(value.w),
        )
        .into_true_color()
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ColorComponents {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: Alpha8,
}

impl ColorComponents {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: Alpha8::OPAQUE,
        }
    }

    #[inline]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: Alpha8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn into_array(self) -> [u8; 4] {
        unsafe { transmute(self) }
    }

    #[inline]
    pub const fn from_array(value: [u8; 4]) -> Self {
        unsafe { transmute(value) }
    }

    #[inline]
    #[cfg(target_endian = "little")]
    pub const fn from_true_color(val: ARGB8888) -> Self {
        unsafe { transmute(val) }
    }

    #[inline]
    #[cfg(target_endian = "little")]
    pub const fn into_true_color(self) -> ARGB8888 {
        unsafe { transmute(self) }
    }

    #[inline]
    pub const fn is_opaque(self) -> bool {
        self.a.is_opaque()
    }

    #[inline]
    pub const fn is_transparent(self) -> bool {
        self.a.is_transparent()
    }
}

#[cfg(target_endian = "little")]
impl From<ARGB8888> for ColorComponents {
    #[inline]
    fn from(color: ARGB8888) -> Self {
        unsafe { transmute(color) }
    }
}

#[cfg(target_endian = "little")]
impl From<ColorComponents> for ARGB8888 {
    #[inline]
    fn from(components: ColorComponents) -> Self {
        unsafe { transmute(components) }
    }
}

#[cfg(target_endian = "little")]
impl Into<u32> for ColorComponents {
    #[inline]
    fn into(self) -> u32 {
        unsafe { transmute(self) }
    }
}

impl From<[u8; 4]> for ColorComponents {
    #[inline]
    fn from(value: [u8; 4]) -> Self {
        ColorComponents::from_array(value)
    }
}

impl From<ColorComponents> for [u8; 4] {
    #[inline]
    fn from(value: ColorComponents) -> Self {
        value.into_array()
    }
}

/// 32bit Color (RGBA 8888)
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct RGBA8888(pub(crate) u32);

impl PixelColor for RGBA8888 {}

impl Translucent for RGBA8888 {
    const TRANSPARENT: Self = Self(0);

    #[inline]
    fn is_transparent(&self) -> bool {
        self.components().is_transparent()
    }

    #[inline]
    fn is_opaque(&self) -> bool {
        self.components().is_opaque()
    }
}

impl PrimaryColor for RGBA8888 {
    const PRIMARY_BLACK: Self = ColorComponentsRGBA::from_rgb(0x00, 0x00, 0x00).into_rgba();
    const PRIMARY_BLUE: Self = ColorComponentsRGBA::from_rgb(0x00, 0x00, 0xFF).into_rgba();
    const PRIMARY_GREEN: Self = ColorComponentsRGBA::from_rgb(0x00, 0xFF, 0x00).into_rgba();
    const PRIMARY_CYAN: Self = ColorComponentsRGBA::from_rgb(0x00, 0xFF, 0xFF).into_rgba();
    const PRIMARY_RED: Self = ColorComponentsRGBA::from_rgb(0xFF, 0x00, 0x00).into_rgba();
    const PRIMARY_MAGENTA: Self = ColorComponentsRGBA::from_rgb(0xFF, 0x00, 0xFF).into_rgba();
    const PRIMARY_YELLOW: Self = ColorComponentsRGBA::from_rgb(0xFF, 0xFF, 0xFF).into_rgba();
    const PRIMARY_WHITE: Self = ColorComponentsRGBA::from_rgb(0xFF, 0xFF, 0xFF).into_rgba();
}

impl RGBA8888 {
    pub const WHITE: Self = Self(0xFFFFFFFF);

    #[inline]
    pub fn components(&self) -> ColorComponentsRGBA {
        ColorComponentsRGBA::from(*self)
    }

    #[inline]
    pub fn opacity(&self) -> Alpha8 {
        self.components().a
    }
}

#[cfg(target_endian = "little")]
impl RGBA8888 {
    #[inline]
    pub const fn from_gray(white: u8, alpha: Alpha8) -> Self {
        Self(white as u32 * 0x00_01_01_01 + alpha.0 as u32 * 0x01_00_00_00)
    }
}

impl From<ARGB8888> for RGBA8888 {
    #[inline]
    fn from(v: ARGB8888) -> Self {
        Self::from(ColorComponentsRGBA::from(v.components()))
    }
}

impl From<RGBA8888> for ARGB8888 {
    #[inline]
    fn from(v: RGBA8888) -> Self {
        Self::from(ColorComponents::from(v.components()))
    }
}

/// Color components (R8 G8 B8 A8)
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ColorComponentsRGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: Alpha8,
}

impl ColorComponentsRGBA {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: Alpha8::OPAQUE,
        }
    }

    #[inline]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: Alpha8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn into_rgba(self) -> RGBA8888 {
        unsafe { transmute(self) }
    }

    #[inline]
    pub fn is_opaque(self) -> bool {
        self.a.is_opaque()
    }

    #[inline]
    pub fn is_transparent(self) -> bool {
        self.a.is_transparent()
    }
}

impl From<RGBA8888> for ColorComponentsRGBA {
    #[inline]
    fn from(color: RGBA8888) -> Self {
        unsafe { transmute(color) }
    }
}

impl From<ColorComponentsRGBA> for RGBA8888 {
    #[inline]
    fn from(components: ColorComponentsRGBA) -> Self {
        unsafe { transmute(components) }
    }
}

impl From<ColorComponentsRGBA> for ColorComponents {
    #[inline]
    fn from(v: ColorComponentsRGBA) -> Self {
        Self {
            b: v.b,
            g: v.g,
            r: v.r,
            a: v.a,
        }
    }
}

impl From<ColorComponents> for ColorComponentsRGBA {
    #[inline]
    fn from(v: ColorComponents) -> Self {
        Self {
            r: v.r,
            g: v.g,
            b: v.b,
            a: v.a,
        }
    }
}

impl From<&[u8; 4]> for ColorComponentsRGBA {
    #[inline]
    fn from(value: &[u8; 4]) -> Self {
        unsafe { transmute(*value) }
    }
}

impl From<[u8; 4]> for ColorComponentsRGBA {
    #[inline]
    fn from(value: [u8; 4]) -> Self {
        unsafe { transmute(value) }
    }
}

/// A type that represents a generic color.
///
/// The [Color] type is convertible to the [PackedColor] type and each other, with some exceptions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Color {
    Transparent,
    Indexed(IndexedColor),
    Argb32(ARGB8888),
}

impl PixelColor for Color {}

impl Translucent for Color {
    const TRANSPARENT: Self = Self::Transparent;

    #[inline]
    fn is_transparent(&self) -> bool {
        match self {
            Color::Transparent => true,
            Color::Indexed(c) => match *c {
                IndexedColor::KEY_COLOR => true,
                _ => false,
            },
            Color::Argb32(c) => c.is_transparent(),
        }
    }

    #[inline]
    fn is_opaque(&self) -> bool {
        match self {
            Color::Transparent => false,
            Color::Indexed(c) => match *c {
                IndexedColor::KEY_COLOR => false,
                _ => true,
            },
            Color::Argb32(c) => c.is_opaque(),
        }
    }
}

impl KeyColor for Color {
    const KEY_COLOR: Self = Self::Indexed(IndexedColor::KEY_COLOR);
}

impl PrimaryColor for Color {
    const PRIMARY_BLACK: Self = Self::from_rgb(0x00_00_00);
    const PRIMARY_BLUE: Self = Self::from_rgb(0x00_00_FF);
    const PRIMARY_GREEN: Self = Self::from_rgb(0x00_FF_00);
    const PRIMARY_CYAN: Self = Self::from_rgb(0x00_FF_FF);
    const PRIMARY_RED: Self = Self::from_rgb(0xFF_00_00);
    const PRIMARY_MAGENTA: Self = Self::from_rgb(0xFF_00_FF);
    const PRIMARY_YELLOW: Self = Self::from_rgb(0xFF_FF_00);
    const PRIMARY_WHITE: Self = Self::from_rgb(0xFF_FF_FF);
}

impl Color {
    pub const BLACK: Self = Self::Argb32(ARGB8888::BLACK);
    pub const BLUE: Self = Self::Argb32(ARGB8888::BLUE);
    pub const GREEN: Self = Self::Argb32(ARGB8888::GREEN);
    pub const CYAN: Self = Self::Argb32(ARGB8888::CYAN);
    pub const RED: Self = Self::Argb32(ARGB8888::RED);
    pub const MAGENTA: Self = Self::Argb32(ARGB8888::MAGENTA);
    pub const BROWN: Self = Self::Argb32(ARGB8888::BROWN);
    pub const LIGHT_GRAY: Self = Self::Argb32(ARGB8888::LIGHT_GRAY);
    pub const DARK_GRAY: Self = Self::Argb32(ARGB8888::DARK_GRAY);
    pub const LIGHT_BLUE: Self = Self::Argb32(ARGB8888::LIGHT_BLUE);
    pub const LIGHT_GREEN: Self = Self::Argb32(ARGB8888::LIGHT_GREEN);
    pub const LIGHT_CYAN: Self = Self::Argb32(ARGB8888::LIGHT_CYAN);
    pub const LIGHT_RED: Self = Self::Argb32(ARGB8888::LIGHT_RED);
    pub const LIGHT_MAGENTA: Self = Self::Argb32(ARGB8888::LIGHT_MAGENTA);
    pub const YELLOW: Self = Self::Argb32(ARGB8888::YELLOW);
    pub const WHITE: Self = Self::Argb32(ARGB8888::WHITE);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self::Argb32(ARGB8888::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self::Argb32(ARGB8888::from_argb(argb))
    }

    #[inline]
    pub const fn into_indexed(&self) -> IndexedColor {
        match self {
            Color::Transparent => IndexedColor::KEY_COLOR,
            Color::Indexed(v) => *v,
            Color::Argb32(v) => IndexedColor::from_rgb(v.rgb()),
        }
    }

    #[inline]
    pub const fn into_true_color(&self) -> ARGB8888 {
        match self {
            Color::Transparent => ARGB8888::TRANSPARENT,
            Color::Indexed(v) => v.as_true_color(),
            Color::Argb32(v) => *v,
        }
    }

    #[inline]
    pub fn brightness(&self) -> Option<u8> {
        match self {
            Color::Transparent => None,
            Color::Indexed(c) => c.brightness(),
            Color::Argb32(c) => c.brightness(),
        }
    }
}

impl Default for Color {
    #[inline]
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl Into<IndexedColor> for Color {
    #[inline]
    fn into(self) -> IndexedColor {
        self.into_indexed()
    }
}

impl Into<ARGB8888> for Color {
    #[inline]
    fn into(self) -> ARGB8888 {
        self.into_true_color()
    }
}

impl From<IndexedColor> for Color {
    #[inline]
    fn from(val: IndexedColor) -> Self {
        Self::Indexed(val)
    }
}

impl From<ARGB8888> for Color {
    #[inline]
    fn from(val: ARGB8888) -> Self {
        Self::Argb32(val)
    }
}

/// A color type that packed into 32 bits
///
/// The [PackedColor] type is convertible to the [Color] type and each other, with some exceptions.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedColor(u32);

impl PixelColor for PackedColor {}

impl Translucent for PackedColor {
    const TRANSPARENT: Self = Self(Self::INDEX_COLOR_MAX + 1);

    #[inline]
    fn is_transparent(&self) -> bool {
        matches!(*self, Self::TRANSPARENT)
    }

    #[inline]
    fn is_opaque(&self) -> bool {
        match self.as_color() {
            Color::Transparent => false,
            Color::Indexed(_) => true,
            Color::Argb32(c) => c.is_opaque(),
        }
    }
}

impl PrimaryColor for PackedColor {
    const PRIMARY_BLACK: Self = Self::from_true_color(ARGB8888::PRIMARY_BLACK);
    const PRIMARY_BLUE: Self = Self::from_true_color(ARGB8888::PRIMARY_BLUE);
    const PRIMARY_GREEN: Self = Self::from_true_color(ARGB8888::PRIMARY_GREEN);
    const PRIMARY_CYAN: Self = Self::from_true_color(ARGB8888::PRIMARY_CYAN);
    const PRIMARY_RED: Self = Self::from_true_color(ARGB8888::PRIMARY_RED);
    const PRIMARY_MAGENTA: Self = Self::from_true_color(ARGB8888::PRIMARY_MAGENTA);
    const PRIMARY_YELLOW: Self = Self::from_true_color(ARGB8888::PRIMARY_YELLOW);
    const PRIMARY_WHITE: Self = Self::from_true_color(ARGB8888::PRIMARY_WHITE);
}

impl PackedColor {
    const INDEX_COLOR_MIN: u32 = 0;
    const INDEX_COLOR_MAX: u32 = Self::INDEX_COLOR_MIN + 0xFF;

    pub const BLACK: Self = Self::from_indexed(IndexedColor::BLACK);
    pub const BLUE: Self = Self::from_indexed(IndexedColor::BLUE);
    pub const GREEN: Self = Self::from_indexed(IndexedColor::GREEN);
    pub const CYAN: Self = Self::from_indexed(IndexedColor::CYAN);
    pub const RED: Self = Self::from_indexed(IndexedColor::RED);
    pub const MAGENTA: Self = Self::from_indexed(IndexedColor::MAGENTA);
    pub const BROWN: Self = Self::from_indexed(IndexedColor::BROWN);
    pub const LIGHT_GRAY: Self = Self::from_indexed(IndexedColor::LIGHT_GRAY);
    pub const DARK_GRAY: Self = Self::from_indexed(IndexedColor::DARK_GRAY);
    pub const LIGHT_BLUE: Self = Self::from_indexed(IndexedColor::LIGHT_BLUE);
    pub const LIGHT_GREEN: Self = Self::from_indexed(IndexedColor::LIGHT_GREEN);
    pub const LIGHT_CYAN: Self = Self::from_indexed(IndexedColor::LIGHT_CYAN);
    pub const LIGHT_RED: Self = Self::from_indexed(IndexedColor::LIGHT_RED);
    pub const LIGHT_MAGENTA: Self = Self::from_indexed(IndexedColor::LIGHT_MAGENTA);
    pub const YELLOW: Self = Self::from_indexed(IndexedColor::YELLOW);
    pub const WHITE: Self = Self::from_indexed(IndexedColor::WHITE);

    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn into_raw(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self::from_true_color(ARGB8888::from_argb(argb))
    }

    #[inline]
    pub const fn from_safe_rgb(rgb: u32) -> Self {
        Self::from_indexed(IndexedColor::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_true_color(argb: ARGB8888) -> Self {
        match argb.is_transparent() {
            true => Self::TRANSPARENT,
            false => Self(argb.argb()),
        }
    }

    #[inline]
    pub const fn from_indexed(index: IndexedColor) -> Self {
        match index {
            IndexedColor::KEY_COLOR => Self::TRANSPARENT,
            _ => Self(Self::INDEX_COLOR_MIN + index.0 as u32),
        }
    }

    #[inline]
    pub const fn from_color(color: Color) -> Self {
        match color {
            Color::Transparent => Self::TRANSPARENT,
            Color::Indexed(index) => Self::from_indexed(index),
            Color::Argb32(argb) => Self::from_true_color(argb),
        }
    }

    #[inline]
    pub const fn as_color(&self) -> Color {
        if self.0 == Self::TRANSPARENT.0 {
            Color::Transparent
        } else if self.0 >= Self::INDEX_COLOR_MIN && self.0 <= Self::INDEX_COLOR_MAX {
            Color::Indexed(IndexedColor((self.0 - Self::INDEX_COLOR_MIN) as u8))
        } else {
            Color::from_argb(self.0)
        }
    }

    #[inline]
    pub const fn into_true_color(self) -> ARGB8888 {
        self.as_color().into_true_color()
    }

    #[inline]
    pub const fn into_indexed(self) -> IndexedColor {
        self.as_color().into_indexed()
    }
}

impl From<ARGB8888> for PackedColor {
    #[inline]
    fn from(color: ARGB8888) -> Self {
        Self::from_true_color(color)
    }
}

impl From<IndexedColor> for PackedColor {
    #[inline]
    fn from(index: IndexedColor) -> Self {
        Self::from_indexed(index)
    }
}

impl From<Color> for PackedColor {
    #[inline]
    fn from(color: Color) -> Self {
        Self::from_color(color)
    }
}

impl From<PackedColor> for Color {
    #[inline]
    fn from(color: PackedColor) -> Self {
        color.as_color()
    }
}

/// 15bit High Color (RGB 555)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RGB555(pub(crate) u16);

impl PixelColor for RGB555 {}

impl PrimaryColor for RGB555 {
    const PRIMARY_BLACK: Self = Self::from_rgb(0x00_00_00);
    const PRIMARY_BLUE: Self = Self::from_rgb(0x00_00_FF);
    const PRIMARY_GREEN: Self = Self::from_rgb(0x00_FF_00);
    const PRIMARY_CYAN: Self = Self::from_rgb(0x00_FF_FF);
    const PRIMARY_RED: Self = Self::from_rgb(0xFF_00_00);
    const PRIMARY_MAGENTA: Self = Self::from_rgb(0xFF_00_FF);
    const PRIMARY_YELLOW: Self = Self::from_rgb(0xFF_FF_00);
    const PRIMARY_WHITE: Self = Self::from_rgb(0xFF_FF_FF);
}

impl RGB555 {
    #[inline]
    pub const fn components(&self) -> (u8, u8, u8) {
        let b = (self.0 & 0x1F) as u8;
        let g = ((self.0 >> 5) & 0x1F) as u8;
        let r = ((self.0 >> 10) & 0x1F) as u8;
        (r, g, b)
    }

    #[inline]
    pub const fn from_components(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16) << 10) | ((g as u16) << 5) | (b as u16))
    }

    #[inline]
    pub const fn as_true_color(&self) -> ARGB8888 {
        let components = self.components();
        let components = ColorComponents {
            a: Alpha8::OPAQUE,
            r: Self::c5c8(components.0),
            g: Self::c5c8(components.1),
            b: Self::c5c8(components.2),
        };
        components.into_true_color()
    }

    #[inline]
    const fn c5c8(c: u8) -> u8 {
        (c << 3) | (c >> 2)
    }

    #[inline]
    const fn from_rgb(rgb: u32) -> Self {
        Self::from_true_color(ARGB8888::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_true_color(color: ARGB8888) -> Self {
        let components = color.components();
        Self(
            ((components.b >> 3) as u16)
                | (((components.g >> 3) as u16) << 5)
                | (((components.r >> 3) as u16) << 10),
        )
    }
}

impl From<ARGB8888> for RGB555 {
    #[inline]
    fn from(color: ARGB8888) -> Self {
        Self::from_true_color(color)
    }
}

impl From<RGB555> for ARGB8888 {
    #[inline]
    fn from(color: RGB555) -> Self {
        color.as_true_color()
    }
}

impl From<Color> for RGB555 {
    #[inline]
    fn from(color: Color) -> Self {
        Self::from_true_color(color.into_true_color())
    }
}

impl From<RGB555> for Color {
    #[inline]
    fn from(color: RGB555) -> Self {
        Color::Argb32(color.as_true_color())
    }
}

/// 16bit High Color (RGB 565)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RGB565(u16);

impl PixelColor for RGB565 {}

impl PrimaryColor for RGB565 {
    const PRIMARY_BLACK: Self = Self::from_rgb(0x00_00_00);
    const PRIMARY_BLUE: Self = Self::from_rgb(0x00_00_FF);
    const PRIMARY_GREEN: Self = Self::from_rgb(0x00_FF_00);
    const PRIMARY_CYAN: Self = Self::from_rgb(0x00_FF_FF);
    const PRIMARY_RED: Self = Self::from_rgb(0xFF_00_00);
    const PRIMARY_MAGENTA: Self = Self::from_rgb(0xFF_00_FF);
    const PRIMARY_YELLOW: Self = Self::from_rgb(0xFF_FF_00);
    const PRIMARY_WHITE: Self = Self::from_rgb(0xFF_FF_FF);
}

impl RGB565 {
    #[inline]
    pub const fn components(&self) -> (u8, u8, u8) {
        let b = (self.0 & 0x1F) as u8;
        let g = ((self.0 >> 5) & 0x3F) as u8;
        let r = ((self.0 >> 11) & 0x1F) as u8;
        (r, g, b)
    }

    #[inline]
    pub const fn from_components(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16) << 11) | ((g as u16) << 5) | (b as u16))
    }

    #[inline]
    pub const fn as_true_color(&self) -> ARGB8888 {
        let components = self.components();
        let components = ColorComponents {
            a: Alpha8::OPAQUE,
            r: Self::c5c8(components.0),
            g: Self::c6c8(components.1),
            b: Self::c5c8(components.2),
        };
        components.into_true_color()
    }

    #[inline]
    const fn c5c8(c: u8) -> u8 {
        (c << 3) | (c >> 2)
    }

    #[inline]
    const fn c6c8(c: u8) -> u8 {
        (c << 2) | (c >> 4)
    }

    #[inline]
    const fn from_rgb(rgb: u32) -> Self {
        Self::from_true_color(ARGB8888::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_true_color(color: ARGB8888) -> Self {
        let components = color.components();
        Self(
            ((components.b >> 3) as u16)
                | (((components.g >> 2) as u16) << 5)
                | (((components.r >> 3) as u16) << 11),
        )
    }
}

impl From<ARGB8888> for RGB565 {
    #[inline]
    fn from(color: ARGB8888) -> Self {
        Self::from_true_color(color)
    }
}

impl From<RGB565> for ARGB8888 {
    #[inline]
    fn from(color: RGB565) -> Self {
        color.as_true_color()
    }
}

impl From<Color> for RGB565 {
    #[inline]
    fn from(color: Color) -> Self {
        Self::from_true_color(color.into_true_color())
    }
}

impl From<RGB565> for Color {
    #[inline]
    fn from(color: RGB565) -> Self {
        Color::Argb32(color.as_true_color())
    }
}

/// 4bit indexed color
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexedColor4 {
    Color0000 = 0,
    Color0001,
    Color0010,
    Color0011,
    Color0100,
    Color0101,
    Color0110,
    Color0111,
    Color1000,
    Color1001,
    Color1010,
    Color1011,
    Color1100,
    Color1101,
    Color1110,
    Color1111,
}

impl PixelColor for IndexedColor4 {
    #[inline]
    fn stride_for(width: GlUInt) -> usize {
        (width as usize + 1) / 2
    }
}

impl IndexedColor4 {
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        unsafe { transmute(value & 15) }
    }

    #[inline]
    pub const fn into_u8(self) -> u8 {
        self as u8
    }
}

impl From<IndexedColor> for IndexedColor4 {
    #[inline]
    fn from(value: IndexedColor) -> Self {
        Self::from_u8(value.0)
    }
}

impl From<IndexedColor4> for IndexedColor {
    #[inline]
    fn from(value: IndexedColor4) -> Self {
        Self(value.into_u8())
    }
}

impl Default for IndexedColor4 {
    #[inline]
    fn default() -> Self {
        Self::Color0000
    }
}

/// Pair of 4bit indexed color (44)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IndexedColorPair44(u8);

impl IndexedColorPair44 {
    #[inline]
    pub const fn into_pair(self) -> (IndexedColor4, IndexedColor4) {
        let left = IndexedColor4::from_u8(self.0 >> 4);
        let right = IndexedColor4::from_u8(self.0 & 15);
        (left, right)
    }

    #[inline]
    pub const fn lhs(&self) -> IndexedColor4 {
        self.into_pair().0
    }

    #[inline]
    pub const fn rhs(&self) -> IndexedColor4 {
        self.into_pair().1
    }

    #[inline]
    pub const fn from_pair(pair: (IndexedColor4, IndexedColor4)) -> Self {
        let left = (pair.0 as u8) << 4;
        let right = pair.1 as u8;
        Self(left + right)
    }

    #[inline]
    pub fn replace<F>(&mut self, kernel: F)
    where
        F: FnOnce((IndexedColor4, IndexedColor4)) -> (IndexedColor4, IndexedColor4),
    {
        *self = Self::from_pair(kernel(self.into_pair()));
    }

    #[inline]
    pub fn replace_lhs(&mut self, lhs: IndexedColor4) {
        self.replace(|(_lhs, rhs)| (lhs, rhs))
    }

    #[inline]
    pub fn replace_rhs(&mut self, rhs: IndexedColor4) {
        self.replace(|(lhs, _rhs)| (lhs, rhs))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Monochrome {
    Zero,
    One,
}

impl PixelColor for Monochrome {
    #[inline]
    fn stride_for(width: GlUInt) -> usize {
        (width as usize + 7) >> 3
    }
}

impl Monochrome {
    #[inline]
    pub const fn new(value: u8) -> Self {
        match value {
            0 => Self::Zero,
            _ => Self::One,
        }
    }

    #[inline]
    pub const fn from_bool(value: bool) -> Self {
        if value {
            Self::One
        } else {
            Self::Zero
        }
    }

    #[inline]
    pub const fn into_bool(self) -> bool {
        match self {
            Monochrome::Zero => false,
            Monochrome::One => true,
        }
    }
}

impl From<Monochrome> for u8 {
    #[inline]
    fn from(value: Monochrome) -> Self {
        match value {
            Monochrome::Zero => 0,
            Monochrome::One => 1,
        }
    }
}

impl From<Monochrome> for usize {
    #[inline]
    fn from(value: Monochrome) -> Self {
        match value {
            Monochrome::Zero => 0,
            Monochrome::One => 1,
        }
    }
}

impl From<u8> for Monochrome {
    #[inline]
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

impl From<Monochrome> for bool {
    #[inline]
    fn from(value: Monochrome) -> Self {
        value.into_bool()
    }
}

impl From<bool> for Monochrome {
    #[inline]
    fn from(value: bool) -> Self {
        Self::new(value as u8)
    }
}

impl Default for Monochrome {
    #[inline]
    fn default() -> Self {
        Self::Zero
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Octet(u8);

impl Octet {
    #[inline]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_u8(&self) -> u8 {
        self.0
    }

    #[inline]
    pub fn get(&self, at: usize) -> Monochrome {
        Monochrome::new(self.0 & (0x80u8.wrapping_shr(at as u32)))
    }

    #[inline]
    pub fn set(&mut self, at: usize, value: Monochrome) {
        let mask = 0x80u8.wrapping_shr(at as u32);
        if value == Monochrome::One {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    #[inline]
    pub fn from_array(array: &[Monochrome]) -> Self {
        array
            .iter()
            .take(8)
            .enumerate()
            .filter_map(|(v, bit)| bit.into_bool().then(|| 0x80u8 >> v))
            .reduce(|a, b| a + b)
            .map(Self)
            .unwrap_or_default()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Monochrome> {
        let raw = self.0;
        (0..8)
            .map(|v| 0x80u8 >> v)
            .map(move |v| Monochrome::new(raw & v))
    }

    #[inline]
    pub fn into_array(self) -> [Monochrome; 8] {
        let mut result = [Monochrome::default(); 8];
        result.iter_mut().zip(self.iter()).for_each(|(a, b)| *a = b);
        result
    }
}

impl Default for Octet {
    #[inline]
    fn default() -> Self {
        Self(0)
    }
}

impl From<Octet> for u8 {
    #[inline]
    fn from(value: Octet) -> Self {
        value.as_u8()
    }
}

impl fmt::Debug for Octet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08b}", self.0)
    }
}
