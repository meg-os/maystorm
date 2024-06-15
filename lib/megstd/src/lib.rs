//! MEG-OS standard library like `std`
#![cfg_attr(not(test), no_std)]
#![feature(alloc_error_handler)]
#![feature(asm_experimental_arch)]
#![feature(cfg_match)]

#[macro_use]
pub mod sys;

pub use meggl as drawing;
pub mod error;
pub mod fs;
pub mod game;
pub mod io;
pub mod mem;
pub mod osstr;
pub mod path;
pub mod rand;
pub mod string;
pub mod time;

pub use uuid;

#[cfg(feature = "window")]
#[allow(unused_imports)]
pub mod window {
    pub use crate::sys::window::*;
}

extern crate alloc;

#[allow(unused_imports)]
pub mod prelude {
    pub use crate::osstr::*;
    pub use crate::sys::prelude::*;
    pub use crate::*;
    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::collections::btree_map::BTreeMap;
    pub use alloc::format;
    pub use alloc::rc::Rc;
    pub use alloc::string::String;
    pub use alloc::string::ToString;
    pub use alloc::sync::{Arc, Weak};
    pub use alloc::vec::Vec;
}
