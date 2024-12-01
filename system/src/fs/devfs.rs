use super::*;
use crate::sync::RwLock;
use crate::*;
use core::mem::MaybeUninit;
use core::num::NonZeroU32;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicUsize, Ordering};
use megstd::fs::FileType;
use megstd::io::{ErrorKind, Result};

const ROOT_INODE: INodeType = unsafe { INodeType::new_unchecked(1) };

static mut SHARED: MaybeUninit<DevFs> = MaybeUninit::uninit();

/// Device Filesystem
pub struct DevFs {
    minor_devices: RwLock<BTreeMap<MinorDevNo, Arc<ThisFsInodeEntry>>>,
    // next_major_device: AtomicUsize,
    next_minor_device: AtomicUsize,
}

impl DevFs {
    // const MAX_MAJOR_DEVICE: usize = 0x0000_FFFF;
    const MAX_MINOR_DEVICE: usize = 0x0000_FFFF;

    pub unsafe fn init() -> Arc<dyn FsDriver> {
        assert_call_once!();

        (&mut *addr_of_mut!(SHARED)).write(Self {
            minor_devices: RwLock::new(BTreeMap::new()),
            // next_major_device: AtomicUsize::new(0),
            next_minor_device: AtomicUsize::new(1 + ROOT_INODE.get() as usize),
        });

        dev::install_drivers();

        let driver = DevFsDriver;
        Arc::new(driver)
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { (&*addr_of!(SHARED)).assume_init_ref() }
    }

    // fn _next_major_device_no(&self) -> Option<MajorDevNo> {
    //     self.next_major_device
    //         .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
    //             (v < Self::MAX_MAJOR_DEVICE).then(|| v + 1)
    //         })
    //         .ok()
    //         .and_then(|v| NonZeroU32::new(v as u32))
    //         .map(|v| MajorDevNo(v))
    // }

    fn _next_minor_device_no(&self) -> Option<MinorDevNo> {
        self.next_minor_device
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                (v < Self::MAX_MINOR_DEVICE).then(|| v + 1)
            })
            .ok()
            .and_then(|v| NonZeroU32::new(v as u32))
            .map(|v| MinorDevNo(v))
    }

    pub fn install_minor_device(
        driver: Arc<dyn DeviceFileDriver>,
    ) -> core::result::Result<MinorDevNo, InstallDeviceError> {
        let shared = Self::shared();
        let mut devices = shared.minor_devices.write().unwrap();

        let name = driver.name();
        if Self::_find_file(&devices, name.as_str()).is_some() {
            // TODO: conflict name
            return Err(InstallDeviceError::NameAlreadyInUse);
        }

        let Some(dev_no) = shared._next_minor_device_no() else {
            return Err(InstallDeviceError::OutOfDeviceNumber);
        };
        let entry = ThisFsInodeEntry {
            file_type: driver.info().file_type,
            dev_no,
            name,
            driver,
        };
        devices.insert(dev_no, Arc::new(entry));
        Ok(dev_no)
    }

    #[inline]
    fn _find_file(
        dir: &BTreeMap<MinorDevNo, Arc<ThisFsInodeEntry>>,
        lpc: &str,
    ) -> Option<INodeType> {
        dir.values().find(|v| v.name() == lpc).map(|v| v.inode())
    }

    fn find_file(&self, dir: INodeType, lpc: &str) -> Result<INodeType> {
        if dir == ROOT_INODE {
            Self::_find_file(&self.minor_devices.read().unwrap(), lpc)
                .ok_or(ErrorKind::NotFound.into())
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    #[inline]
    fn get_file(dev_no: MinorDevNo) -> Option<Arc<ThisFsInodeEntry>> {
        DevFs::shared()
            .minor_devices
            .read()
            .unwrap()
            .get(&dev_no)
            .map(|v| v.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstallDeviceError {
    NameAlreadyInUse,
    OutOfDeviceNumber,
}

struct DevFsDriver;

impl FsDriver for DevFsDriver {
    fn device_name(&self) -> String {
        "devfs".to_owned()
    }

    fn description(&self) -> Option<String> {
        None
    }

    fn root_dir(&self) -> INodeType {
        ROOT_INODE
    }

    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry> {
        if dir == ROOT_INODE {
            let shared = DevFs::shared();
            shared
                .minor_devices
                .read()
                .unwrap()
                .values()
                .nth(index)
                .map(|dir_ent| {
                    FsRawDirEntry::new(dir_ent.inode(), dir_ent.name(), dir_ent.as_ref().into())
                })
        } else {
            None
        }
    }

    fn lookup(&self, dir: INodeType, lpc: &str) -> Result<INodeType> {
        DevFs::shared().find_file(dir, lpc)
    }

    fn open(self: Arc<Self>, inode: INodeType) -> Result<Arc<dyn FsAccessToken>> {
        let Ok(dev_no) = inode.try_into() else {
            return Err(ErrorKind::NotFound.into());
        };
        DevFs::get_file(dev_no)
            .ok_or(ErrorKind::NotFound.into())
            .and_then(|v| v.driver.open())
            .map(|v| Arc::new(DevFsAccessToken::new(v, inode)) as Arc<dyn FsAccessToken>)
    }

    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData> {
        if inode == ROOT_INODE {
            Some(FsRawMetaData::new(
                ROOT_INODE,
                FileType::Dir,
                DevFs::shared().minor_devices.read().unwrap().len() as OffsetType,
            ))
        } else {
            inode
                .try_into()
                .ok()
                .and_then(|dev_no| DevFs::get_file(dev_no))
                .map(|v| v.as_ref().into())
        }
    }
}

struct ThisFsInodeEntry {
    file_type: FileType,
    dev_no: MinorDevNo,
    name: String,
    driver: Arc<dyn DeviceFileDriver>,
}

impl ThisFsInodeEntry {
    #[inline]
    pub fn inode(&self) -> INodeType {
        self.dev_no.into()
    }

    #[inline]
    pub fn name<'a>(&'a self) -> &'a str {
        self.name.as_str()
    }
}

impl From<&ThisFsInodeEntry> for FsRawMetaData {
    fn from(src: &ThisFsInodeEntry) -> Self {
        Self::new(
            src.dev_no.into(),
            src.file_type,
            src.driver.info().size as i64,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MajorDevNo(NonZeroU32);

impl From<MajorDevNo> for INodeType {
    #[inline]
    fn from(value: MajorDevNo) -> Self {
        unsafe { INodeType::new_unchecked((value.0.get() as u128) << 64) }
    }
}

impl TryFrom<INodeType> for MajorDevNo {
    type Error = ();

    #[inline]
    fn try_from(value: INodeType) -> core::result::Result<Self, Self::Error> {
        let value = value.get() >> 64;
        ((value as usize) < DevFs::MAX_MINOR_DEVICE)
            .then(|| NonZeroU32::new(value as u32))
            .flatten()
            .map(|v| MajorDevNo(v))
            .ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MinorDevNo(NonZeroU32);

impl From<MinorDevNo> for INodeType {
    #[inline]
    fn from(value: MinorDevNo) -> Self {
        unsafe { INodeType::new_unchecked(value.0.get() as u128) }
    }
}

impl TryFrom<INodeType> for MinorDevNo {
    type Error = ();

    #[inline]
    fn try_from(value: INodeType) -> core::result::Result<Self, Self::Error> {
        ((value.get() as usize) < DevFs::MAX_MINOR_DEVICE)
            .then(|| MinorDevNo(unsafe { NonZeroU32::new_unchecked(value.get() as u32) }))
            .ok_or(())
    }
}

pub trait DeviceFileDriver {
    fn name(&self) -> String;

    fn info(&self) -> &DeviceCharacteristics {
        &DEFAULT_DEV_INFO
    }

    fn open(&self) -> Result<Arc<dyn DeviceAccessToken>>;
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceCharacteristics {
    pub file_type: FileType,
    pub size: usize,
}

impl DeviceCharacteristics {
    #[inline]
    pub const fn default() -> Self {
        Self {
            file_type: FileType::CharDev,
            size: 0,
        }
    }
}

impl Default for DeviceCharacteristics {
    #[inline]
    fn default() -> Self {
        Self::default()
    }
}

static DEFAULT_DEV_INFO: DeviceCharacteristics = DeviceCharacteristics::default();

pub trait DeviceAccessToken {
    fn info(&self) -> &DeviceCharacteristics {
        &DEFAULT_DEV_INFO
    }

    fn read_data(&self, _offset: OffsetType, _buf: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Ok(0)
    }

    fn lseek(&self, _offset: OffsetType, _whence: Whence) -> Result<OffsetType> {
        Err(ErrorKind::NotSeekable.into())
    }
}

struct DevFsAccessToken {
    device: Arc<dyn DeviceAccessToken>,
    inode: INodeType,
}

impl DevFsAccessToken {
    fn new(device: Arc<dyn DeviceAccessToken>, inode: INodeType) -> Self {
        Self { device, inode }
    }
}

impl FsAccessToken for DevFsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData> {
        let info = self.device.info();
        Some(FsRawMetaData::new(
            self.inode,
            info.file_type,
            info.size as OffsetType,
        ))
    }

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        self.device.read_data(offset, buf)
    }

    fn write_data(&self, offset: OffsetType, buf: &[u8]) -> Result<usize> {
        self.device.write_data(offset, buf)
    }

    fn lseek(&self, offset: OffsetType, whence: Whence) -> Result<OffsetType> {
        self.device.lseek(offset, whence)
    }
}
