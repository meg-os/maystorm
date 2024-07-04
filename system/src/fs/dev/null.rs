use crate::fs::devfs::*;
use crate::*;
use megstd::io::Result;

/// Null Device `/dev/null`
pub struct Null;

impl Null {
    pub fn init() {
        DevFs::install_minor_device(Arc::new(Self)).unwrap();
    }
}

impl DeviceFileDriver for Null {
    fn name(&self) -> String {
        "null".to_owned()
    }

    fn open(&self) -> Result<Arc<dyn DeviceAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl DeviceAccessToken for Null {}
