use super::initramfs::*;
use crate::*;
use alloc::{
    borrow::ToOwned,
    format,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    num::{NonZeroU64, NonZeroUsize},
};
use megstd::{
    io::{self, Result},
    sys::fs_imp::FileType,
};

static mut FS: UnsafeCell<FileManager> = UnsafeCell::new(FileManager::new());

pub type OffsetType = i64;

pub struct FileManager {
    rootfs: Option<Arc<dyn FsDriver>>,
}

impl FileManager {
    pub const PATH_SEPARATOR: &'static str = "/";

    #[inline]
    const fn new() -> Self {
        Self { rootfs: None }
    }

    pub unsafe fn init(initrd_base: usize, initrd_size: usize) {
        let shared = FS.get_mut();
        shared.rootfs = InitRamfs::from_static(initrd_base as *mut u8, initrd_size);
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*FS.get() }
    }

    #[inline]
    pub fn canonical_path(path: &str) -> io::Result<String> {
        let path_components = Self::_canonical_path_components("", "", path)?;
        let path_components = path_components.iter().map(|v| v.as_str()).collect();
        Ok(Self::_join_path(&path_components))
    }

    #[inline]
    fn _join_path(path_components: &Vec<&str>) -> String {
        format!(
            "{}{}",
            Self::PATH_SEPARATOR,
            path_components.join(Self::PATH_SEPARATOR)
        )
    }

    fn _canonical_path_components(
        security_domain: &str,
        prefix: &str,
        path: &str,
    ) -> io::Result<Vec<String>> {
        let path = if path.starts_with("/") {
            path.to_owned()
        } else {
            format!("{prefix}{}{path}", Self::PATH_SEPARATOR)
        };

        let mut components = Vec::new();
        for component in path.split("/") {
            if component.is_empty() || component == "." {
            } else if component == ".." {
                let _ = components.pop();
            } else {
                components.push(component);
            }
        }

        if !Self::_join_path(&components).starts_with(security_domain) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }

        Ok(components.into_iter().map(|v| v.to_owned()).collect())
    }

    fn resolv(path_components: &Vec<String>) -> io::Result<(Arc<dyn FsDriver>, INodeType)> {
        let shared = FileManager::shared();
        let fs = match shared.rootfs.as_ref() {
            Some(v) => v.clone(),
            None => return Err(io::ErrorKind::NotConnected.into()),
        };
        let mut dir = fs.root_dir();
        for lpc in path_components {
            dir = fs.find_file(dir, lpc.as_str())?;
        }
        Ok((fs, dir))
    }

    pub fn read_dir(path: &str) -> io::Result<FsRawReadDir> {
        let path = Self::_canonical_path_components("", "", path)?;
        let (fs, dir) = Self::resolv(&path)?;
        Ok(FsRawReadDir::new(&fs, dir))
    }

    pub fn open(path: &str) -> io::Result<FsRawFileControlBlock> {
        let path = Self::_canonical_path_components("", "", path)?;
        let (fs, inode) = Self::resolv(&path)?;
        let inode = fs.open(inode)?;

        let stat = match fs.stat(inode) {
            Some(v) => v,
            None => return Err(io::ErrorKind::InvalidData.into()),
        };

        let fcb = FsRawFileControlBlock::new(&fs, inode, stat.len());

        Ok(fcb)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct INodeType(NonZeroU64);

impl INodeType {
    #[inline]
    pub const unsafe fn new_unchecked(val: u64) -> Self {
        Self(NonZeroU64::new_unchecked(val))
    }

    #[inline]
    pub const fn new(val: u64) -> Option<Self> {
        match NonZeroU64::new(val) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn get(&self) -> u64 {
        self.0.get()
    }
}

pub trait FsDriver {
    /// Returns the inode of the root directory
    fn root_dir(&self) -> INodeType;
    /// Reads the specified directory
    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry>;
    /// Finds the specified file name in the specified directory
    fn find_file(&self, dir: INodeType, name: &str) -> Result<INodeType>;
    /// Opens a file with the specified inode. If necessary, the re-mapped inode may be returned.
    fn open(&self, inode: INodeType) -> Result<INodeType>;
    /// Closes the file of the specified inode
    fn close(&self, inode: INodeType) -> Result<()>;
    /// Obtains metadata for the specified inode
    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData>;
    /// Reads data from the specified inode
    fn read_data(&self, inode: INodeType, offset: OffsetType, buf: &mut [u8]) -> io::Result<usize>;
    /// Writes data to the specified inode
    fn write_data(&self, inode: INodeType, offset: OffsetType, buf: &[u8]) -> io::Result<usize>;
}

pub struct FsRawReadDir {
    fs: Weak<dyn FsDriver>,
    dir: INodeType,
    index: usize,
}

impl FsRawReadDir {
    fn new(fs: &Arc<dyn FsDriver>, dir: INodeType) -> Self {
        Self {
            fs: Arc::downgrade(fs),
            dir,
            index: 0,
        }
    }
}

impl Iterator for FsRawReadDir {
    type Item = FsRawDirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.fs
            .upgrade()
            .as_ref()
            .and_then(|v| v.read_dir(self.dir, self.index))
            .map(|v| {
                self.index += 1;
                v
            })
    }
}

pub struct FsRawDirEntry {
    inode: INodeType,
    name: String,
    metadata: Option<FsRawMetaData>,
}

impl FsRawDirEntry {
    #[inline]
    pub fn new(inode: INodeType, name: &str, metadata: Option<FsRawMetaData>) -> Self {
        Self {
            inode,
            name: name.to_owned(),
            metadata,
        }
    }

    #[inline]
    pub const fn inode(&self) -> INodeType {
        self.inode
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn metadata(&self) -> Option<&FsRawMetaData> {
        self.metadata.as_ref()
    }

    #[inline]
    pub fn into_metadata(self) -> Option<FsRawMetaData> {
        self.metadata
    }
}

pub struct FsRawMetaData {
    file_type: FileType,
    len: OffsetType,
}

impl FsRawMetaData {
    #[inline]
    pub const fn new(file_type: FileType, len: OffsetType) -> Self {
        Self { file_type, len }
    }

    #[inline]
    pub const fn len(&self) -> OffsetType {
        self.len
    }

    #[inline]
    pub const fn file_type(&self) -> FileType {
        self.file_type
    }
}

pub struct FsRawFileControlBlock {
    fs: Weak<dyn FsDriver>,
    inode: INodeType,
    file_pos: OffsetType,
    file_size: OffsetType,
}

impl FsRawFileControlBlock {
    #[inline]
    fn new(fs: &Arc<dyn FsDriver>, inode: INodeType, file_size: OffsetType) -> Self {
        Self {
            fs: Arc::downgrade(fs),
            inode,
            file_pos: 0,
            file_size,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let fs = match self.fs.upgrade() {
            Some(v) => v,
            None => return Err(io::ErrorKind::NotFound.into()),
        };
        fs.read_data(self.inode, self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    pub fn read_to_end(&mut self, vec: &mut Vec<u8>) -> io::Result<usize> {
        let size = (self.file_size - self.file_pos) as usize;
        vec.resize(size, 0);
        self.read(vec.as_mut_slice()).map(|v| {
            vec.resize(v, 0);
            v
        })
    }

    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let fs = match self.fs.upgrade() {
            Some(v) => v,
            None => return Err(io::ErrorKind::NotFound.into()),
        };
        fs.write_data(self.inode, self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    pub fn lseek(&mut self, offset: OffsetType, whence: Whence) -> OffsetType {
        match whence {
            Whence::SeekSet => self.file_pos = offset,
            Whence::SeekCur => self.file_pos = self.file_pos + offset,
            Whence::SeekEnd => self.file_pos = self.file_size + offset,
        }
        self.file_pos
    }

    pub fn stat(&self) -> Option<FsRawMetaData> {
        self.fs.upgrade().and_then(|v| v.as_ref().stat(self.inode))
    }
}

impl Drop for FsRawFileControlBlock {
    fn drop(&mut self) {
        let _ = self.fs.upgrade().map(|v| v.as_ref().close(self.inode));
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FsFileHandle(NonZeroUsize);

impl FsFileHandle {
    // TODO:
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Whence {
    SeekSet = 0,
    SeekCur,
    SeekEnd,
}

impl From<usize> for Whence {
    fn from(v: usize) -> Self {
        match v {
            1 => Self::SeekCur,
            2 => Self::SeekEnd,
            _ => Self::SeekSet,
        }
    }
}
