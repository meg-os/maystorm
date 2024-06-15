use super::*;
use crate::vec::Vec2;
use core::mem::swap;
use core::ops::*;
pub use num_traits::Zero;

pub type Point = Vec2<GlSInt>;

impl Point {
    #[inline]
    pub fn swap(&mut self) {
        swap(&mut self.x, &mut self.y);
    }

    #[inline]
    pub const fn swapped(&self) -> Self {
        Self {
            x: self.y(),
            y: self.x(),
        }
    }

    pub fn line_to<F>(&self, other: Point, mut f: F)
    where
        F: FnMut(Self),
    {
        let c0 = *self;
        let c1 = other;

        let d = Point::new(
            if c1.x > c0.x {
                c1.x - c0.x
            } else {
                c0.x - c1.x
            },
            if c1.y > c0.y {
                c1.y - c0.y
            } else {
                c0.y - c1.y
            },
        );

        let s = Self::new(
            if c1.x > c0.x { 1 } else { -1 },
            if c1.y > c0.y { 1 } else { -1 },
        );

        let mut c0 = c0;
        let mut e = d.x - d.y;
        loop {
            f(c0);
            if c0.x == c1.x && c0.y == c1.y {
                break;
            }
            let e2 = e + e;
            if e2 > -d.y {
                e -= d.y;
                c0.x += s.x;
            }
            if e2 < d.x {
                e += d.x;
                c0.y += s.y;
            }
        }
    }

    #[inline]
    pub fn distance2(&self, other: Point) -> Distance2 {
        let movement = self.sub(other);
        Distance2((movement.x * movement.x + movement.y * movement.y) as usize)
    }
}

impl Add<GlSInt> for Point {
    type Output = Self;

    #[inline]
    fn add(self, rhs: GlSInt) -> Self {
        Point {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl AddAssign<GlSInt> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: GlSInt) {
        *self = self.add(rhs);
    }
}

impl Sub<GlSInt> for Point {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: GlSInt) -> Self {
        Point {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl SubAssign<GlSInt> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: GlSInt) {
        *self = self.sub(rhs);
    }
}

impl Add<Point> for Rect {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Point) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x + rhs.x,
                y: self.origin.y + rhs.y,
            },
            size: self.size,
        }
    }
}

impl AddAssign<Point> for Rect {
    #[inline]
    fn add_assign(&mut self, rhs: Point) {
        *self = self.add(rhs);
    }
}

impl Sub<Point> for Rect {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Point) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x - rhs.x,
                y: self.origin.y - rhs.y,
            },
            size: self.size,
        }
    }
}

impl SubAssign<Point> for Rect {
    #[inline]
    fn sub_assign(&mut self, rhs: Point) {
        *self = self.sub(rhs);
    }
}

/// Type of Squared Distance
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance2(pub usize);

impl Distance2 {
    #[inline]
    pub const fn from_scalar(v: isize) -> Self {
        Self((v * v) as usize)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Size {
    pub width: GlUInt,
    pub height: GlUInt,
}

impl Size {
    #[inline]
    pub const fn new(width: GlUInt, height: GlUInt) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn width(&self) -> GlUInt {
        self.width
    }

    #[inline]
    pub const fn height(&self) -> GlUInt {
        self.height
    }

    #[inline]
    pub const fn width_height_usize(&self) -> usize {
        self.width as usize * self.height as usize
    }

    #[inline]
    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    #[inline]
    pub fn swap(&mut self) {
        swap(&mut self.width, &mut self.height);
    }

    #[inline]
    pub const fn swapped(&self) -> Self {
        Self {
            width: self.height,
            height: self.width,
        }
    }
}

impl Zero for Size {
    #[inline]
    fn zero() -> Self {
        Self {
            width: 0,
            height: 0,
        }
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.width == 0 && self.height == 0
    }
}

impl Add<Self> for Size {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Size {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl Add<EdgeInsets> for Size {
    type Output = Self;

    #[inline]
    fn add(self, rhs: EdgeInsets) -> Self {
        Size {
            width: ((self.width as GlSInt) + rhs.left + rhs.right) as GlUInt,
            height: ((self.height as GlSInt) + rhs.top + rhs.bottom) as GlUInt,
        }
    }
}

impl AddAssign<Self> for Size {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl AddAssign<EdgeInsets> for Size {
    #[inline]
    fn add_assign(&mut self, rhs: EdgeInsets) {
        *self = self.add(rhs);
    }
}

impl Sub<Self> for Size {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Size {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

impl Sub<EdgeInsets> for Size {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: EdgeInsets) -> Self {
        Size {
            width: ((self.width as GlSInt) - (rhs.left + rhs.left)) as GlUInt,
            height: ((self.height as GlSInt) - (rhs.top + rhs.bottom)) as GlUInt,
        }
    }
}

impl SubAssign<Self> for Size {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs);
    }
}

impl SubAssign<EdgeInsets> for Size {
    #[inline]
    fn sub_assign(&mut self, rhs: EdgeInsets) {
        *self = self.sub(rhs);
    }
}

impl Mul<Self> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            width: self.width * rhs.width,
            height: self.height * rhs.height,
        }
    }
}

impl MulAssign<Self> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul(rhs);
    }
}

impl Mul<GlUInt> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: GlUInt) -> Self::Output {
        Self {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

impl MulAssign<GlUInt> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: GlUInt) {
        *self = self.mul(rhs);
    }
}

impl Div<GlUInt> for Size {
    type Output = Self;

    #[inline]
    fn div(self, rhs: GlUInt) -> Self::Output {
        Self {
            width: self.width / rhs,
            height: self.height / rhs,
        }
    }
}

impl DivAssign<GlUInt> for Size {
    #[inline]
    fn div_assign(&mut self, rhs: GlUInt) {
        *self = self.div(rhs);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const VOID: Self = Self {
        origin: Point::new(GlSInt::MAX, GlSInt::MAX),
        size: Size::new(0, 0),
    };

    #[inline]
    pub const fn new(x: GlSInt, y: GlSInt, width: GlUInt, height: GlUInt) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    #[inline]
    pub fn from_diagonal(c1: Point, c2: Point) -> Self {
        Self::from_coordinates(Coordinates::from_diagonal(c1, c2))
    }

    #[inline]
    pub const fn from_coordinates(coods: Coordinates) -> Rect {
        Rect {
            origin: coods.left_top(),
            size: coods.size(),
        }
    }

    #[inline]
    pub const fn void() -> Self {
        Self::VOID
    }

    #[inline]
    pub const fn origin(&self) -> Point {
        self.origin
    }

    #[inline]
    pub const fn size(&self) -> Size {
        self.size
    }

    #[inline]
    pub const fn width(&self) -> GlUInt {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> GlUInt {
        self.size.height
    }

    #[inline]
    pub const fn min_x(&self) -> GlSInt {
        self.origin.x
    }

    #[inline]
    pub const fn max_x(&self) -> GlSInt {
        self.origin.x() + (self.width() as GlSInt)
    }

    #[inline]
    pub const fn mid_x(&self) -> GlSInt {
        (self.min_x() + self.max_x()) / 2
    }

    #[inline]
    pub const fn min_y(&self) -> GlSInt {
        self.origin.y
    }

    #[inline]
    pub const fn max_y(&self) -> GlSInt {
        self.origin.y() + (self.height() as GlSInt)
    }

    #[inline]
    pub const fn mid_y(&self) -> GlSInt {
        (self.min_y() + self.max_y()) / 2
    }

    #[inline]
    pub const fn insets_by(self, insets: EdgeInsets) -> Self {
        Self {
            origin: Point {
                x: self.origin.x + insets.left,
                y: self.origin.y + insets.top,
            },
            size: Size {
                width: ((self.size.width as GlSInt) - (insets.left + insets.right)) as GlUInt,
                height: ((self.size.height as GlSInt) - (insets.top + insets.bottom)) as GlUInt,
            },
        }
    }

    #[inline]
    pub const fn overlaps(self, rhs: Self) -> bool {
        let Ok(cl) = Coordinates::from_rect(self) else {
            return false;
        };
        let Ok(cr) = Coordinates::from_rect(rhs) else {
            return false;
        };

        cl.left < cr.right && cr.left < cl.right && cl.top < cr.bottom && cr.top < cl.bottom
    }

    #[inline]
    pub const fn center(&self) -> Point {
        Point::new(self.mid_x(), self.mid_y())
    }

    #[inline]
    pub const fn bounds(&self) -> Rect {
        Rect {
            origin: Point::new(0, 0),
            size: self.size,
        }
    }
}

pub trait Contains<T> {
    fn contains(&self, other: T) -> bool;
}

impl Contains<Point> for Rect {
    #[inline]
    fn contains(&self, other: Point) -> bool {
        if let Ok(coords) = Coordinates::from_rect(*self) {
            coords.left <= other.x
                && coords.right > other.x
                && coords.top <= other.y
                && coords.bottom > other.y
        } else {
            false
        }
    }
}

impl Contains<Rect> for Rect {
    #[inline]
    fn contains(&self, other: Rect) -> bool {
        let Ok(cl) = Coordinates::from_rect(*self) else {
            return false;
        };
        let Ok(cr) = Coordinates::from_rect(other) else {
            return false;
        };

        cl.left <= cr.left && cl.right >= cr.right && cl.top <= cr.top && cl.bottom >= cr.bottom
    }
}

impl From<Size> for Rect {
    #[inline]
    fn from(size: Size) -> Self {
        size.bounds()
    }
}

impl From<(Point, Size)> for Rect {
    #[inline]
    fn from(value: (Point, Size)) -> Self {
        Self::new(value.0.x, value.0.y, value.1.width, value.1.height)
    }
}

impl Add<Size> for Rect {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Size) -> Self::Output {
        Self {
            origin: self.origin,
            size: self.size + rhs,
        }
    }
}

impl AddAssign<Size> for Rect {
    #[inline]
    fn add_assign(&mut self, rhs: Size) {
        self.size += rhs;
    }
}

impl Sub<Size> for Rect {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Size) -> Self::Output {
        Self {
            origin: self.origin,
            size: self.size - rhs,
        }
    }
}

impl SubAssign<Size> for Rect {
    #[inline]
    fn sub_assign(&mut self, rhs: Size) {
        self.size -= rhs;
    }
}

impl Add<EdgeInsets> for Rect {
    type Output = Self;

    #[inline]
    fn add(self, rhs: EdgeInsets) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x - rhs.left,
                y: self.origin.y - rhs.top,
            },
            size: Size {
                width: ((self.size.width as GlSInt) + (rhs.left + rhs.right)) as GlUInt,
                height: ((self.size.height as GlSInt) + (rhs.top + rhs.bottom)) as GlUInt,
            },
        }
    }
}

impl AddAssign<EdgeInsets> for Rect {
    #[inline]
    fn add_assign(&mut self, rhs: EdgeInsets) {
        *self = self.add(rhs);
    }
}

impl Sub<EdgeInsets> for Rect {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: EdgeInsets) -> Self::Output {
        self.insets_by(rhs)
    }
}

impl SubAssign<EdgeInsets> for Rect {
    #[inline]
    fn sub_assign(&mut self, rhs: EdgeInsets) {
        *self = self.sub(rhs);
    }
}

impl Mul<GlUInt> for Rect {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: GlUInt) -> Self::Output {
        Self::new(
            self.origin.x() * rhs as GlSInt,
            self.origin.y() * rhs as GlSInt,
            self.width() * rhs,
            self.height() * rhs,
        )
    }
}

impl MulAssign<GlUInt> for Rect {
    #[inline]
    fn mul_assign(&mut self, rhs: GlUInt) {
        *self = self.mul(rhs);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Coordinates {
    pub left: GlSInt,
    pub top: GlSInt,
    pub right: GlSInt,
    pub bottom: GlSInt,
}

impl Coordinates {
    pub const VOID: Self = Self::new(GlSInt::MAX, GlSInt::MAX, GlSInt::MIN, GlSInt::MIN);

    #[inline]
    pub const fn new(left: GlSInt, top: GlSInt, right: GlSInt, bottom: GlSInt) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub const fn void() -> Self {
        Self::VOID
    }

    #[inline]
    pub fn from_diagonal(c1: Point, c2: Point) -> Self {
        let a = c1.x();
        let b = c2.x();
        let left = a.min(b);
        let right = a.max(b);

        let a = c1.y();
        let b = c2.y();
        let top = a.min(b);
        let bottom = a.max(b);

        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub const fn left_top(&self) -> Point {
        Point::new(self.left, self.top)
    }

    #[inline]
    pub const fn right_bottom(&self) -> Point {
        Point::new(self.right, self.bottom)
    }

    #[inline]
    pub const fn left_bottom(&self) -> Point {
        Point::new(self.left, self.bottom)
    }

    #[inline]
    pub const fn right_top(&self) -> Point {
        Point::new(self.right, self.top)
    }

    #[inline]
    pub const fn size(&self) -> Size {
        Size::new(
            (self.right - self.left) as GlUInt,
            (self.bottom - self.top) as GlUInt,
        )
    }

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.left < self.right && self.top < self.bottom
    }

    #[inline]
    #[must_use]
    pub fn merged(&self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    #[inline]
    pub fn merge(&mut self, other: Self) {
        *self = self.merged(other);
    }

    #[inline]
    #[must_use]
    pub fn trimmed(&self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
    }

    #[inline]
    pub fn trim(&mut self, other: Self) {
        *self = self.trimmed(other);
    }

    #[inline]
    pub fn from_size(size: Size) -> Self {
        Self {
            left: 0,
            top: 0,
            right: safe_to_int(size.width()),
            bottom: safe_to_int(size.height()),
        }
    }

    #[inline]
    pub const fn from_rect(rect: Rect) -> Result<Coordinates, ()> {
        if (rect.size.width as GlSInt) <= 0 || (rect.size.height as GlSInt) <= 0 {
            Err(())
        } else {
            Ok(unsafe { Self::from_rect_unchecked(rect) })
        }
    }

    #[inline]
    pub const unsafe fn from_rect_unchecked(rect: Rect) -> Coordinates {
        let left = rect.origin.x;
        let right = left + rect.size.width as GlSInt;
        let top = rect.origin.y;
        let bottom = top + rect.size.height as GlSInt;

        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl Add<Self> for Coordinates {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.merged(rhs)
    }
}

impl AddAssign<Self> for Coordinates {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.merge(rhs)
    }
}

impl TryFrom<Rect> for Coordinates {
    type Error = ();

    #[inline]
    fn try_from(value: Rect) -> Result<Self, Self::Error> {
        Self::from_rect(value)
    }
}

impl From<Coordinates> for Rect {
    #[inline]
    fn from(coods: Coordinates) -> Rect {
        Rect {
            origin: coods.left_top(),
            size: coods.size(),
        }
    }
}

impl From<Size> for Coordinates {
    #[inline]
    fn from(size: Size) -> Self {
        Self::from_size(size)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct EdgeInsets {
    pub top: GlSInt,
    pub left: GlSInt,
    pub bottom: GlSInt,
    pub right: GlSInt,
}

impl EdgeInsets {
    #[inline]
    pub const fn new(top: GlSInt, left: GlSInt, bottom: GlSInt, right: GlSInt) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    #[inline]
    pub const fn padding_each(value: GlSInt) -> Self {
        Self {
            top: value,
            left: value,
            bottom: value,
            right: value,
        }
    }
}

impl Zero for EdgeInsets {
    #[inline]
    fn zero() -> Self {
        Self {
            top: 0,
            left: 0,
            bottom: 0,
            right: 0,
        }
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.top == 0 && self.left == 0 && self.bottom == 0 && self.right == 0
    }
}

impl Add<Self> for EdgeInsets {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            top: self.top + rhs.top,
            left: self.left + rhs.left,
            bottom: self.bottom + rhs.bottom,
            right: self.right + rhs.right,
        }
    }
}

impl AddAssign<Self> for EdgeInsets {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl Sub<Self> for EdgeInsets {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            top: self.top - rhs.top,
            left: self.left - rhs.left,
            bottom: self.bottom - rhs.bottom,
            right: self.right - rhs.right,
        }
    }
}

impl SubAssign<Self> for EdgeInsets {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs);
    }
}
