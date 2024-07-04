use crate::fs::{devfs::*, *};
use crate::*;
use megstd::io::{ErrorKind, Result};

/// Storage Full Device `/dev/full`
pub struct Full;

impl Full {
    pub fn init() {
        DevFs::install_minor_device(Arc::new(Self)).unwrap();
    }
}

impl DeviceFileDriver for Full {
    fn name(&self) -> String {
        "full".to_owned()
    }

    fn open(&self) -> Result<Arc<dyn DeviceAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl DeviceAccessToken for Full {
    fn read_data(&self, _offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Err(ErrorKind::StorageFull.into())
    }
}
