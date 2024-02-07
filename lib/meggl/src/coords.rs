use core::convert::TryFrom;
use core::mem::swap;
use core::ops::*;
pub use num_traits::Zero;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Point {
    pub x: isize,
    pub y: isize,
}

impl Point {
    #[inline]
    pub const fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.y
    }

    #[inline]
    pub fn swap(&mut self) {
        swap(&mut self.x, &mut self.y);
    }

    #[inline]
    pub const fn swapped(&self) -> Self {
        Self {
            x: self.y,
            y: self.x,
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
        self.sub(other).distance2()
    }
}

impl Add<isize> for Point {
    type Output = Self;

    #[inline]
    fn add(self, rhs: isize) -> Self {
        Point {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl AddAssign<isize> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: isize) {
        *self = self.add(rhs);
    }
}

impl Sub<isize> for Point {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: isize) -> Self {
        Point {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl SubAssign<isize> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: isize) {
        *self = self.sub(rhs);
    }
}

impl Sub<Self> for Point {
    type Output = Movement;

    #[inline]
    fn sub(self, rhs: Self) -> Movement {
        Movement {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Movement {
    pub x: isize,
    pub y: isize,
}

impl Movement {
    #[inline]
    pub const fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.y
    }

    #[inline]
    pub fn swap(&mut self) {
        swap(&mut self.x, &mut self.y);
    }

    #[inline]
    pub const fn swapped(&self) -> Self {
        Self {
            x: self.y,
            y: self.x,
        }
    }

    #[inline]
    pub const fn distance2(&self) -> Distance2 {
        Distance2((self.x * self.x + self.y * self.y) as usize)
    }
}

impl Zero for Movement {
    #[inline]
    fn zero() -> Self {
        Self { x: 0, y: 0 }
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }
}

impl From<Point> for Movement {
    #[inline]
    fn from(v: Point) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<Movement> for Point {
    #[inline]
    fn from(v: Movement) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl Add<Self> for Movement {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Movement) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign<Self> for Movement {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl Add<Movement> for Point {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Movement) -> Self::Output {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign<Movement> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Movement) {
        *self = self.add(rhs);
    }
}

impl Add<Movement> for Rect {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Movement) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x + rhs.x,
                y: self.origin.y + rhs.y,
            },
            size: self.size,
        }
    }
}

impl AddAssign<Movement> for Rect {
    #[inline]
    fn add_assign(&mut self, rhs: Movement) {
        *self = self.add(rhs);
    }
}

impl Sub<Self> for Movement {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign<Self> for Movement {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs);
    }
}

impl Sub<Movement> for Point {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Movement) -> Self::Output {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign<Movement> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: Movement) {
        *self = self.sub(rhs);
    }
}

impl Sub<Movement> for Rect {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Movement) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x - rhs.x,
                y: self.origin.y - rhs.y,
            },
            size: self.size,
        }
    }
}

impl SubAssign<Movement> for Rect {
    #[inline]
    fn sub_assign(&mut self, rhs: Movement) {
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

impl From<Movement> for Distance2 {
    #[inline]
    fn from(v: Movement) -> Self {
        v.distance2()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Size {
    pub width: isize,
    pub height: isize,
}

impl Size {
    #[inline]
    pub const fn new(width: isize, height: isize) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.height
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
            width: self.width + rhs.left + rhs.right,
            height: self.height + rhs.top + rhs.bottom,
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
            width: self.width - (rhs.left + rhs.left),
            height: self.height - (rhs.top + rhs.bottom),
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

impl Mul<isize> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: isize) -> Self::Output {
        Self {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

impl MulAssign<isize> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: isize) {
        *self = self.mul(rhs);
    }
}

impl Mul<usize> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: usize) -> Self::Output {
        Self {
            width: self.width * rhs as isize,
            height: self.height * rhs as isize,
        }
    }
}

impl MulAssign<usize> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: usize) {
        *self = self.mul(rhs);
    }
}

impl Div<isize> for Size {
    type Output = Self;

    #[inline]
    fn div(self, rhs: isize) -> Self::Output {
        Self {
            width: self.width / rhs,
            height: self.height / rhs,
        }
    }
}

impl DivAssign<isize> for Size {
    #[inline]
    fn div_assign(&mut self, rhs: isize) {
        *self = self.div(rhs);
    }
}

impl Div<usize> for Size {
    type Output = Self;

    #[inline]
    fn div(self, rhs: usize) -> Self::Output {
        Self {
            width: self.width / rhs as isize,
            height: self.height / rhs as isize,
        }
    }
}

impl DivAssign<usize> for Size {
    #[inline]
    fn div_assign(&mut self, rhs: usize) {
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
        origin: Point::new(isize::MAX, isize::MAX),
        size: Size::new(0, 0),
    };

    #[inline]
    pub const fn new(x: isize, y: isize, width: isize, height: isize) -> Self {
        Self {
            origin: Point {
                x: if width >= 0 { x } else { x + width },
                y: if height >= 0 { y } else { y + height },
            },
            size: Size {
                width: abs(width),
                height: abs(height),
            },
        }
    }

    #[inline]
    pub const fn from_diagonal(c1: Point, c2: Point) -> Self {
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
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub const fn min_x(&self) -> isize {
        self.origin.x
    }

    #[inline]
    pub const fn max_x(&self) -> isize {
        self.origin.x() + self.width()
    }

    #[inline]
    pub const fn mid_x(&self) -> isize {
        (self.min_x() + self.max_x()) / 2
    }

    #[inline]
    pub const fn min_y(&self) -> isize {
        self.origin.y
    }

    #[inline]
    pub const fn max_y(&self) -> isize {
        self.origin.y() + self.height()
    }

    #[inline]
    pub const fn mid_y(&self) -> isize {
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
                width: self.size.width - (insets.left + insets.right),
                height: self.size.height - (insets.top + insets.bottom),
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
                width: self.size.width + (rhs.left + rhs.right),
                height: self.size.height + (rhs.top + rhs.bottom),
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

impl Mul<isize> for Rect {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: isize) -> Self::Output {
        Self::new(
            self.origin.x() * rhs,
            self.origin.y() * rhs,
            self.width() * rhs,
            self.height() * rhs,
        )
    }
}

impl MulAssign<isize> for Rect {
    #[inline]
    fn mul_assign(&mut self, rhs: isize) {
        *self = self.mul(rhs);
    }
}

impl Mul<usize> for Rect {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: usize) -> Self::Output {
        Self::new(
            self.origin.x() * rhs as isize,
            self.origin.y() * rhs as isize,
            self.width() * rhs as isize,
            self.height() * rhs as isize,
        )
    }
}

impl MulAssign<usize> for Rect {
    #[inline]
    fn mul_assign(&mut self, rhs: usize) {
        *self = self.mul(rhs);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Coordinates {
    pub left: isize,
    pub top: isize,
    pub right: isize,
    pub bottom: isize,
}

impl Coordinates {
    pub const VOID: Self = Self::new(isize::MAX, isize::MAX, isize::MIN, isize::MIN);

    #[inline]
    pub const fn new(left: isize, top: isize, right: isize, bottom: isize) -> Self {
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
    pub const fn from_diagonal(c1: Point, c2: Point) -> Self {
        let a = c1.x();
        let b = c2.x();
        let left = min(a, b);
        let right = max(a, b);

        let a = c1.y();
        let b = c2.y();
        let top = min(a, b);
        let bottom = max(a, b);

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
        Size::new(self.right - self.left, self.bottom - self.top)
    }

    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.left < self.right && self.top < self.bottom
    }

    #[inline]
    #[must_use]
    pub const fn merged(&self, other: Self) -> Self {
        Self {
            left: min(self.left, other.left),
            top: min(self.top, other.top),
            right: max(self.right, other.right),
            bottom: max(self.bottom, other.bottom),
        }
    }

    #[inline]
    pub fn merge(&mut self, other: Self) {
        *self = self.merged(other);
    }

    #[inline]
    #[must_use]
    pub const fn trimmed(&self, other: Self) -> Self {
        Self {
            left: max(self.left, other.left),
            top: max(self.top, other.top),
            right: min(self.right, other.right),
            bottom: min(self.bottom, other.bottom),
        }
    }

    #[inline]
    pub fn trim(&mut self, other: Self) {
        *self = self.trimmed(other);
    }

    #[inline]
    pub const fn from_size(size: Size) -> Self {
        Self {
            left: 0,
            top: 0,
            right: size.width(),
            bottom: size.height(),
        }
    }

    #[inline]
    pub const fn from_rect(rect: Rect) -> Result<Coordinates, ()> {
        if rect.size.width == 0 || rect.size.height == 0 {
            Err(())
        } else {
            Ok(unsafe { Self::from_rect_unchecked(rect) })
        }
    }

    #[inline]
    pub const unsafe fn from_rect_unchecked(rect: Rect) -> Coordinates {
        let left: isize;
        let right: isize;
        if rect.size.width > 0 {
            left = rect.origin.x;
            right = left + rect.size.width;
        } else {
            right = rect.origin.x;
            left = right + rect.size.width;
        }

        let top: isize;
        let bottom: isize;
        if rect.size.height > 0 {
            top = rect.origin.y;
            bottom = top + rect.size.height;
        } else {
            bottom = rect.origin.y;
            top = bottom + rect.size.height;
        }

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
    pub top: isize,
    pub left: isize,
    pub bottom: isize,
    pub right: isize,
}

impl EdgeInsets {
    #[inline]
    pub const fn new(top: isize, left: isize, bottom: isize, right: isize) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    #[inline]
    pub const fn padding_each(value: isize) -> Self {
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

#[inline]
const fn abs(val: isize) -> isize {
    if val >= 0 {
        val
    } else {
        0 - val
    }
}

#[inline]
const fn min(lhs: isize, rhs: isize) -> isize {
    if lhs < rhs {
        lhs
    } else {
        rhs
    }
}

#[inline]
const fn max(lhs: isize, rhs: isize) -> isize {
    if lhs > rhs {
        lhs
    } else {
        rhs
    }
}
