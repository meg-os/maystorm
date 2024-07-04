//! MEG-OS Standard Graphics Drawing Library
#![no_std]
#![feature(cfg_match)]

extern crate alloc;
extern crate libm;

/// Preferred Signed Integer
pub type GlSInt = i32;
/// Preferred Unsigned Integer
pub type GlUInt = u32;
/// Preferred Floating Point Number
pub type GlFloat = f64;

mod bitmap;
mod color;
mod coords;
pub use bitmap::*;
pub use color::*;
pub use coords::*;

pub mod rotation;
pub mod vec;

#[cfg(test)]
pub mod tests;

#[inline]
pub fn safe_to_int(val: GlUInt) -> GlSInt {
    safe_clip(val, GlSInt::MAX) as GlSInt
}

#[inline]
pub fn safe_clip(val: GlUInt, limit: GlSInt) -> GlUInt {
    val.min(limit as GlUInt)
}
