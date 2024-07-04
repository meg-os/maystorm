use crate::prelude::*;

pub mod hid;

mod error;
pub use error::*;

pub type Result<T> = core::result::Result<T, Error>;

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize>;

    //fn read_to_string(&mut self, buf: &mut String) -> Result<usize>

    //fn take(self, limit: u64) -> Take<Self>
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    fn flush(&mut self) -> Result<()>;

    //fn write_all(&mut self, buf: &[u8]) -> Result<()>
}
