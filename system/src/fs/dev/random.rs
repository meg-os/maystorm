use crate::fs::{devfs::*, *};
use crate::*;
use megstd::io::Result;

/// Random Device `/dev/random`
pub struct Random;

impl Random {
    pub fn init() {
        DevFs::install_minor_device(Arc::new(Self)).unwrap();
    }
}

impl DeviceFileDriver for Random {
    fn name(&self) -> String {
        "random".to_owned()
    }

    fn open(&self) -> Result<Arc<dyn DeviceAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl DeviceAccessToken for Random {
    fn read_data(&self, _offset: OffsetType, _buf: &mut [u8]) -> Result<usize> {
        todo!()
        // Ok(buf.len())
    }
}
