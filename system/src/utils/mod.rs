//! Utilities

#[macro_use]
mod log;
pub use log::*;

#[repr(transparent)]
pub struct HexDump<'a>(pub &'a [u8]);

impl core::fmt::Debug for HexDump<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for c in self.0.iter() {
            write!(f, " {:02x}", *c)?;
        }
        Ok(())
    }
}

// like bitflags
#[macro_export]
macro_rules! my_bitflags {
    (
        $(#[$outer:meta])*
        $vis:vis struct $class:ident: $ty:ty {
            $(
                $(#[$attr:ident $($args:tt)*])*
                const $flag:ident = $value:expr;
            )*
        }
    ) => {

        $(#[$outer])*
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq)]
        $vis struct $class($ty);

        impl $class {
            const __EMPTY: Self = Self(0);

            $(
                $(#[$attr $($args)*])*
                $vis const $flag: Self = Self($value);
            )*

            const __ALL: Self = Self(0
                $(| $value)*
            );
        }

        #[allow(dead_code)]
        impl $class {

            #[inline]
            pub const fn from_bits_retain(bits: $ty) -> Self {
                Self(bits)
            }

            #[inline]
            pub const fn from_bits_truncate(bits: $ty) -> Self {
                Self(bits & Self::__ALL.0)
            }

            #[inline]
            pub const fn bits(&self) -> $ty {
                self.0
            }

            #[inline]
            pub const fn empty() -> Self {
                Self::__EMPTY
            }

            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.bits() == Self::__EMPTY.bits()
            }

            #[inline]
            pub const fn all() -> Self {
                Self::__ALL
            }

            #[inline]
            pub const fn is_all(&self) -> bool {
                (self.bits() & Self::__ALL.bits()) == Self::__ALL.bits()
            }

            #[inline]
            pub const fn contains(&self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            #[inline]
            pub const fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }

            #[inline]
            pub const fn remove(&mut self, other: Self) {
                self.0 &= !other.0;
            }

            #[inline]
            pub const fn toggle(&mut self, other: Self) {
                self.0 ^= other.0;
            }

            #[inline]
            pub const fn set(&mut self, other: Self, value: bool) {
                if value {
                    self.insert(other);
                } else {
                    self.remove(other);
                }
            }

            #[inline]
            pub const fn intersects(&self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            #[inline]
            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }

            #[inline]
            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            #[inline]
            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }

            #[inline]
            #[must_use]
            pub const fn symmetric_difference(self, other: Self) -> Self {
                Self(self.0 ^ other.0)
            }

            #[inline]
            #[must_use]
            pub const fn complement(self) -> Self {
                Self(!self.0 & Self::__ALL.0)
            }
        }

        impl core::ops::Not for $class {
            type Output = Self;

            #[inline]
            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl core::ops::BitAnd<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.bits() & rhs.bits())
            }
        }

        impl core::ops::BitAndAssign<Self> for $class {
            #[inline]
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl core::ops::BitOr<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.bits() | rhs.bits())
            }
        }

        impl core::ops::BitOrAssign<Self> for $class {
            #[inline]
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl core::ops::BitXor<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.bits() ^ rhs.bits())
            }
        }

        impl core::ops::BitXorAssign<Self> for $class {
            #[inline]
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }

        impl core::ops::Sub<Self> for $class {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 & !rhs.0)
            }
        }

        impl core::ops::SubAssign<Self> for $class {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                self.0 &= !rhs.0;
            }
        }

        impl From<$ty> for $class {
            #[inline]
            fn from(val: $ty) -> $class {
                $class::from_bits_retain(val)
            }
        }

        impl From<$class> for $ty {
            #[inline]
            fn from(val: $class) -> $ty {
                val.0
            }
        }

    };
}
