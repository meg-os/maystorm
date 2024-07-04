use super::devfs::DevFs;
use crate::fs::ramfs::RamFs;
use crate::sync::{RwLock, RwLockReadGuard};
use crate::task::scheduler::Scheduler;
use crate::*;
use core::fmt::{self, Display};
use core::num::NonZeroU128;
use megstd::fs::FileType;
use megstd::io::{Error, ErrorKind, Read, Result, Write};
use myos_archive::ArchiveReader;

pub use megstd::sys::fs_imp::OpenOptions;

static FS: FileManager = FileManager::new();

pub type OffsetType = i64;

pub struct FileManager {
    mount_points: RwLock<BTreeMap<String, Arc<dyn FsDriver>>>,
}

unsafe impl Send for FileManager {}

unsafe impl Sync for FileManager {}

impl FileManager {
    pub const PATH_SEPARATOR: &'static str = "/";

    #[inline]
    const fn new() -> Self {
        Self {
            mount_points: RwLock::new(BTreeMap::new()),
        }
    }

    #[inline]
    fn _unable_to_create(path: &str, err: Error) -> ! {
        panic!("Unable to create {path}: {err}");
    }

    #[inline]
    fn _unable_to_create_initrd(path: &str, err: Error) -> ! {
        panic!("Failed to process file in the initrd: {path}: {err}");
    }

    #[inline]
    fn _unable_to_write_to(path: &str, err: Error) -> ! {
        panic!("Unable to write to {path}: {err}");
    }

    pub unsafe fn init(initrd_base: *mut u8, initrd_size: usize) {
        assert_call_once!();

        macro_rules! mount {
            ( $mount_points:expr, $path:expr, $driver:expr ) => {
                $mount_points.insert($path.to_owned(), $driver);
            };
        }

        {
            let mut mount_points = Self::shared().mount_points.write().unwrap();
            mount!(mount_points, Self::PATH_SEPARATOR, RamFs::new());
            drop(mount_points);

            for path in ["boot", "system", "home", "bin", "dev", "etc", "tmp", "var"] {
                Self::mkdir(path).unwrap_or_else(|err| Self::_unable_to_create(path, err))
            }

            let mut mount_points = Self::shared().mount_points.write().unwrap();
            mount!(mount_points, "/dev/", DevFs::init());
        }

        {
            let path_initramfs = "/boot/";
            let reader = ArchiveReader::from_static(initrd_base, initrd_size)
                .expect("Unable to access initramfs");

            let mut cwd = path_initramfs.to_owned();
            for entry in reader {
                match entry {
                    myos_archive::Entry::Namespace(path, _xattr) => {
                        let path = Self::_join_path(&Self::_canonical_path_components(
                            path_initramfs,
                            path,
                        ));
                        Self::mkdir2(&path)
                            .unwrap_or_else(|err| Self::_unable_to_create_initrd(&path, err));
                        cwd = path;
                    }
                    myos_archive::Entry::File(name, _xattr, content) => {
                        let path = Self::_join_path(&Self::_canonical_path_components(&cwd, name));
                        // log!("FILE {path}");
                        let mut file = Self::creat(&path)
                            .unwrap_or_else(|err| Self::_unable_to_create_initrd(&path, err));
                        file.write(content).unwrap_or_else(|err| {
                            Self::_unable_to_write_to(&path, err);
                        });
                    }

                    myos_archive::Entry::End => break,
                    _ => unreachable!(),
                }
            }
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        &FS
    }

    fn _join_path(path_components: &Vec<String>) -> String {
        format!(
            "{}{}",
            Self::PATH_SEPARATOR,
            path_components.join(Self::PATH_SEPARATOR)
        )
    }

    fn _canonical_path_components(base: &str, path: &str) -> Vec<String> {
        let path = if path.starts_with("/") {
            path.to_owned()
        } else {
            format!("{}{}{}", base, Self::PATH_SEPARATOR, path)
        };

        let mut path_components = Vec::new();
        for component in path.split("/") {
            if component.is_empty() || component == "." {
            } else if component == ".." {
                let _ = path_components.pop();
            } else {
                path_components.push(component);
            }
        }

        path_components.into_iter().map(|v| v.to_owned()).collect()
    }

    pub fn canonical_path_components(path: &str) -> Vec<String> {
        Self::_canonical_path_components(Scheduler::current_pid().cwd().as_str(), path)
    }

    pub fn canonicalize(path: &str) -> String {
        Self::_join_path(&Self::canonical_path_components(path))
    }

    fn _parent_path(path: &str) -> Option<String> {
        let mut path_components = Self::_canonical_path_components("", path);
        path_components
            .pop()
            .map(|_| Self::_join_path(&path_components))
    }

    /// Resolve all path components, including the last path component
    fn resolve_all(path: &str) -> Result<(Arc<dyn FsDriver>, INodeType)> {
        let shared = FileManager::shared();
        let mount_points = shared.mount_points.read().unwrap();

        let fq_path = format!("{}{}", Self::canonicalize(path), Self::PATH_SEPARATOR);

        let mut prefixes = mount_points.keys().collect::<Vec<_>>();
        prefixes.sort();
        let prefix = prefixes
            .into_iter()
            .rev()
            .find(|&v| fq_path.starts_with(v))
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;
        let fs = mount_points
            .get(prefix)
            .map(|v| v.clone())
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;

        let resolved = &fq_path[prefix.len() - 1..];
        let mut dir = fs.root_dir();
        let components = Self::_canonical_path_components(Self::PATH_SEPARATOR, resolved);
        for pc in components {
            dir = fs.lookup(dir, pc.as_str())?;
        }
        Ok((fs, dir))
    }

    /// Resolve path components except the last path component
    fn resolve_parent(path: &str) -> Result<(Arc<dyn FsDriver>, INodeType, Option<String>)> {
        let shared = FileManager::shared();
        let mount_points = shared.mount_points.read().unwrap();

        let mut components = Self::canonical_path_components(path);
        let lpc = components.pop();
        let fq_path = format!("{}{}", Self::_join_path(&components), Self::PATH_SEPARATOR);

        let mut prefixes = mount_points.keys().collect::<Vec<_>>();
        prefixes.sort();
        let prefix = prefixes
            .into_iter()
            .rev()
            .find(|&v| fq_path.starts_with(v))
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;
        let fs = mount_points
            .get(prefix)
            .map(|v| v.clone())
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;

        let resolved = &fq_path[prefix.len() - 1..];
        let mut dir = fs.root_dir();
        let components = Self::_canonical_path_components(Self::PATH_SEPARATOR, resolved);
        for pc in components {
            dir = fs.lookup(dir, pc.as_str())?;
        }
        Ok((fs, dir, lpc))
    }

    pub fn chdir(path: &str) -> Result<()> {
        let path_components = Self::canonical_path_components(path);
        let (fs, inode) = Self::resolve_all(path)?;
        let stat = fs.stat(inode).ok_or(ErrorKind::NotFound)?;
        if !stat.file_type().is_dir() {
            return Err(ErrorKind::NotADirectory.into());
        }

        Scheduler::current_pid().set_cwd(Self::_join_path(&path_components).as_str());

        Ok(())
    }

    pub fn read_dir(path: &str) -> Result<impl Iterator<Item = FsRawDirEntry>> {
        let (fs, dir) = Self::resolve_all(path)?;
        Ok(FsRawReadDir::new(fs, dir))
    }

    pub fn open(path: &str, options: &OpenOptions) -> Result<FsRawFileControlBlock> {
        let (fs, inode) = Self::resolve_all(path)?;

        let Some(stat) = fs.stat(inode) else {
            return Err(ErrorKind::NotFound.into());
        };
        if stat.file_type().is_dir() {
            return Err(ErrorKind::IsADirectory.into());
        }

        let access_token = fs.open(inode)?;

        Ok(FsRawFileControlBlock::new(
            access_token,
            options,
            stat.file_type().is_char_device() || stat.file_type().is_block_device(),
        ))
    }

    pub fn creat(path: &str) -> Result<FsRawFileControlBlock> {
        let (fs, dir, lpc) = Self::resolve_parent(path)?;
        let Some(name) = lpc else {
            return Err(ErrorKind::NotFound.into());
        };
        let name = name.as_str();

        let access_token = fs.creat(dir, name)?;

        Ok(FsRawFileControlBlock::new(
            access_token,
            OpenOptions::new().read(true).write(true).create(true),
            false,
        ))
    }

    pub fn mkdir(path: &str) -> Result<()> {
        let (fs, dir, lpc) = Self::resolve_parent(path)?;
        if let Some(name) = lpc {
            fs.mkdir(dir, &name)
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    pub fn mkdir2(path: &str) -> Result<()> {
        match Self::mkdir(path) {
            Ok(v) => Ok(v),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => {
                    if let Some(parent) = Self::_parent_path(path) {
                        Self::mkdir2(&parent).and_then(|_| Self::mkdir(path))
                    } else {
                        Err(err)
                    }
                }
                _ => Err(err),
            },
        }
    }

    pub fn unlink(path: &str) -> Result<()> {
        let (fs, dir, lpc) = Self::resolve_parent(path)?;
        let Some(name) = lpc else {
            return Err(ErrorKind::NotFound.into());
        };

        fs.unlink(dir, &name)
    }

    pub fn stat(path: &str) -> Result<FsRawMetaData> {
        let (fs, mut inode, lpc) = Self::resolve_parent(path)?;
        if let Some(lpc) = lpc {
            inode = fs.lookup(inode, &lpc)?;
        }
        fs.stat(inode).ok_or(ErrorKind::NotFound.into())
    }

    pub fn rename(old_path: &str, new_path: &str) -> Result<()> {
        let old_path = format!("{}{}", Self::canonicalize(old_path), Self::PATH_SEPARATOR);
        let new_path = format!("{}{}", Self::canonicalize(new_path), Self::PATH_SEPARATOR);

        if old_path == new_path {
            return Ok(());
        } else if new_path.starts_with(&old_path) {
            return Err(ErrorKind::InvalidInput.into());
        }

        let (fs1, old_dir, old_name) = Self::resolve_parent(&old_path)?;
        let Some(old_name) = old_name else {
            return Err(ErrorKind::NotFound.into());
        };

        let (fs2, mut new_dir, new_name) = Self::resolve_parent(&new_path)?;
        let new_name = match new_name {
            Some(new_name) => match fs2.lookup(new_dir, &new_name) {
                Ok(inode) => match fs2.stat(inode) {
                    Some(stat) => {
                        if stat.file_type().is_dir() {
                            new_dir = inode;
                            old_name.clone()
                        } else {
                            new_name
                        }
                    }
                    None => new_name,
                },
                Err(_) => new_name,
            },
            None => old_name.clone(),
        };

        if Arc::ptr_eq(&fs1, &fs2) {
            fs1.rename(old_dir, &old_name, new_dir, &new_name, true)
        } else {
            Err(ErrorKind::CrossesDevices.into())
        }
    }

    pub fn mount_points<'a>() -> RwLockReadGuard<'a, BTreeMap<String, Arc<dyn FsDriver>>> {
        let shared = FileManager::shared();
        shared.mount_points.read().unwrap()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct INodeType(NonZeroU128);

impl INodeType {
    #[inline]
    pub const unsafe fn new_unchecked(val: u128) -> Self {
        Self(NonZeroU128::new_unchecked(val))
    }

    #[inline]
    pub const fn new(val: u128) -> Option<Self> {
        match NonZeroU128::new(val) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn get(&self) -> u128 {
        self.0.get()
    }
}

impl Display for INodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

pub trait FsDriver {
    /// Device name if mounted on physical device, otherwise name of file system driver
    fn device_name(&self) -> String;
    /// Return mount options as string
    fn description(&self) -> Option<String>;
    /// Returns the inode of the root directory
    fn root_dir(&self) -> INodeType;
    /// Reads the specified directory
    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry>;
    /// Searches for a file system object with the specified file name in the specified directory.
    fn lookup(&self, dir: INodeType, name: &str) -> Result<INodeType>;
    /// Opens a file with the specified inode
    fn open(self: Arc<Self>, inode: INodeType) -> Result<Arc<dyn FsAccessToken>>;
    /// Obtains metadata for the specified inode
    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData>;

    fn creat(self: Arc<Self>, _dir: INodeType, _name: &str) -> Result<Arc<dyn FsAccessToken>> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    fn mkdir(self: Arc<Self>, _dir: INodeType, _name: &str) -> Result<()> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    fn rename(
        &self,
        _old_dir: INodeType,
        _old_name: &str,
        _new_dir: INodeType,
        _new_name: &str,
        _replace: bool,
    ) -> Result<()> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    // fn link(&self, _old_inode: INodeType, _new_dir: INodeType, _new_name: &str) -> Result<()> {
    //     Err(ErrorKind::ReadOnlyFilesystem.into())
    // }

    fn unlink(&self, _dir: INodeType, _name: &str) -> Result<()> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }
}

pub trait FsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData>;

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize>;

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    fn lseek(&self, _offset: OffsetType, _whence: Whence) -> Result<OffsetType> {
        Err(ErrorKind::Unsupported.into())
    }

    fn truncate(&self, _length: OffsetType) -> Result<()> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

pub struct FsRawReadDir {
    fs: Arc<dyn FsDriver>,
    dir: INodeType,
    index: usize,
}

impl FsRawReadDir {
    fn new(fs: Arc<dyn FsDriver>, dir: INodeType) -> Self {
        Self { fs, dir, index: 0 }
    }
}

impl Iterator for FsRawReadDir {
    type Item = FsRawDirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.fs.as_ref().read_dir(self.dir, self.index).map(|v| {
            self.index += 1;
            v
        })
    }
}

pub struct FsRawDirEntry {
    inode: INodeType,
    name: String,
    metadata: FsRawMetaData,
}

impl FsRawDirEntry {
    #[inline]
    pub fn new(inode: INodeType, name: &str, metadata: FsRawMetaData) -> Self {
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
    pub fn metadata(&self) -> &FsRawMetaData {
        &self.metadata
    }
}

pub struct FsRawMetaData {
    inode: INodeType,
    file_type: FileType,
    len: OffsetType,
}

impl FsRawMetaData {
    #[inline]
    pub const fn new(inode: INodeType, file_type: FileType, len: OffsetType) -> Self {
        Self {
            inode,
            file_type,
            len,
        }
    }

    #[inline]
    pub const fn inode(&self) -> INodeType {
        self.inode
    }

    #[inline]
    pub const fn file_type(&self) -> FileType {
        self.file_type
    }

    #[inline]
    pub const fn len(&self) -> OffsetType {
        self.len
    }
}

pub struct FsRawFileControlBlock {
    access_token: Arc<dyn FsAccessToken>,
    options: OpenOptions,
    is_device: bool,
    file_pos: OffsetType,
}

impl FsRawFileControlBlock {
    #[inline]
    fn new(access_token: Arc<dyn FsAccessToken>, options: &OpenOptions, is_device: bool) -> Self {
        Self {
            access_token,
            options: *options,
            is_device,
            file_pos: 0,
        }
    }

    pub fn lseek(&mut self, offset: OffsetType, whence: Whence) -> Result<OffsetType> {
        if self.is_device {
            self.access_token.lseek(offset, whence)
        } else {
            if let Some(new_pos) = match whence {
                Whence::SeekSet => Some(offset),
                Whence::SeekCur => self.file_pos.checked_add(offset),
                Whence::SeekEnd => match self.access_token.stat() {
                    Some(stat) => stat.len().checked_add(offset),
                    None => Some(0),
                },
            } {
                self.file_pos = new_pos;
                Ok(new_pos)
            } else {
                Err(ErrorKind::InvalidInput.into())
            }
        }
    }

    pub fn truncate(&mut self, length: OffsetType) -> Result<()> {
        self.access_token.truncate(length)
    }

    pub fn fstat(&self) -> Option<FsRawMetaData> {
        self.access_token.stat()
    }
}

impl Read for FsRawFileControlBlock {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.options.contains(OpenOptions::READ) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access_token.read_data(self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    fn read_to_end(&mut self, vec: &mut Vec<u8>) -> Result<usize> {
        const BUFFER_SIZE: usize = 0x10000;
        let mut buffer = Vec::new();
        buffer
            .try_reserve(BUFFER_SIZE)
            .map_err(|_| megstd::io::Error::from(ErrorKind::OutOfMemory))?;
        buffer.resize(BUFFER_SIZE, 0);

        let mut count_read = 0;
        loop {
            match self.read(buffer.as_mut_slice()) {
                Ok(new_len) => {
                    if new_len == 0 {
                        return Ok(count_read);
                    }
                    if vec.try_reserve(new_len).is_err() {
                        return Err(ErrorKind::OutOfMemory.into());
                    }
                    if new_len < buffer.len() {
                        vec.extend_from_slice(&buffer[..new_len]);
                    } else {
                        vec.extend_from_slice(buffer.as_slice());
                    }
                    count_read += new_len;
                }
                Err(err) => match err.kind() {
                    ErrorKind::Interrupted => (),
                    _ => return Err(err),
                },
            }
        }
    }
}

impl Write for FsRawFileControlBlock {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if !self.options.contains(OpenOptions::WRITE) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access_token.write_data(self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    fn flush(&mut self) -> Result<()> {
        self.access_token.flush()
    }
}

impl fmt::Write for FsRawFileControlBlock {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes())
            .map(|_| ())
            .map_err(|_| core::fmt::Error)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Whence {
    SeekSet = 0,
    SeekCur,
    SeekEnd,
}

impl Default for Whence {
    #[inline]
    fn default() -> Self {
        Self::SeekSet
    }
}

impl TryFrom<usize> for Whence {
    type Error = ();

    fn try_from(value: usize) -> core::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::SeekSet),
            1 => Ok(Self::SeekCur),
            2 => Ok(Self::SeekEnd),
            _ => Err(()),
        }
    }
}
