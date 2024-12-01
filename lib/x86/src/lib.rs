//! My x86 libraries

#![cfg_attr(not(test), no_std)]
#![feature(negative_impls)]
// #![deny(unsafe_op_in_unsafe_fn)]
// #![feature(cfg_match)]

extern crate alloc;
pub mod cpuid;
pub mod cr;
pub mod efer;
pub mod gpr;
pub mod msr;
pub mod prot;
