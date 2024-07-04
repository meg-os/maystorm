use crate::fs::{devfs::*, *};
use crate::*;
use megstd::io::Result;

/// Zero Device `/dev/zero`
pub struct Zero;

impl Zero {
    pub fn init() {
        DevFs::install_minor_device(Arc::new(Self)).unwrap();
    }
}

impl DeviceFileDriver for Zero {
    fn name(&self) -> String {
        "zero".to_owned()
    }

    fn open(&self) -> Result<Arc<dyn DeviceAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl DeviceAccessToken for Zero {
    fn read_data(&self, _offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }
}
