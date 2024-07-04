// File I/O
// Most of them are clones of Rust's original definition.

use crate::io::{Read, Result, Write};
use crate::path::*;
use crate::prelude::*;
use crate::sys::fs_imp;

pub struct File {
    _phantom: (),
}

impl File {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
        OpenOptions::new().read(true).open(path.as_ref())
    }

    pub fn sync_all(&self) -> Result<()> {
        todo!()
    }

    pub fn sync_data(&self) -> Result<()> {
        todo!()
    }

    pub fn set_len(&self, _size: u64) -> Result<()> {
        todo!()
    }

    pub fn try_clone(&self) -> Result<File> {
        todo!()
    }

    pub fn set_permissions(&self, _perm: Permissions) -> Result<()> {
        todo!()
    }
}

impl Read for File {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    fn read_to_end(&mut self, _buf: &mut alloc::vec::Vec<u8>) -> Result<usize> {
        todo!()
    }
}

impl Write for File {
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
    }
}

// #[derive(Clone)]
pub struct OpenOptions(fs_imp::OpenOptions);

impl OpenOptions {
    #[inline]
    pub fn new() -> Self {
        Self(fs_imp::OpenOptions::new())
    }

    #[inline]
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.0.read(read);
        self
    }

    #[inline]
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.0.write(write);
        self
    }

    #[inline]
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.0.append(append);
        self
    }

    #[inline]
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.0.truncate(truncate);
        self
    }

    #[inline]
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.0.create(create);
        self
    }

    #[inline]
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.0.create_new(create_new);
        self
    }

    #[inline]
    pub fn open<P: AsRef<Path>>(&self, _path: P) -> Result<File> {
        todo!()
    }
}

pub fn read_dir<P: AsRef<Path>>(_path: P) -> Result<ReadDir> {
    todo!()
}

pub fn canonicalize<P: AsRef<Path>>(_path: P) -> Result<PathBuf> {
    todo!()
}

#[derive(Debug, Clone)]
pub struct Metadata(fs_imp::Metadata);

impl Metadata {
    pub fn file_type(&self) -> FileType {
        self.0.file_type()
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        self.file_type().is_dir()
    }

    #[inline]
    pub fn is_file(&self) -> bool {
        self.file_type().is_file()
    }

    pub fn len(&self) -> u64 {
        self.0.len()
    }

    pub fn permissions(&self) -> Permissions {
        Permissions(self.0.permissions())
    }

    // pub fn modified(&self) -> Result<SystemTime>
    // pub fn accessed(&self) -> Result<SystemTime>
    // pub fn created(&self) -> Result<SystemTime>
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FileType {
    Dir,
    File,
    Symlink,
    BlockDev,
    CharDev,
    Fifo,
    Socket,
}

impl FileType {
    #[inline]
    pub const fn is_dir(&self) -> bool {
        matches!(*self, Self::Dir)
    }

    #[inline]
    pub const fn is_file(&self) -> bool {
        matches!(*self, Self::File)
    }

    #[inline]
    pub const fn is_symlink(&self) -> bool {
        matches!(*self, Self::Symlink)
    }

    #[inline]
    pub const fn is_block_device(&self) -> bool {
        matches!(*self, Self::BlockDev)
    }

    #[inline]
    pub const fn is_char_device(&self) -> bool {
        matches!(*self, Self::CharDev)
    }

    #[inline]
    pub const fn is_fifo(&self) -> bool {
        matches!(*self, Self::Fifo)
    }

    #[inline]
    pub const fn is_socket(&self) -> bool {
        matches!(*self, Self::Socket)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(fs_imp::Permissions);

impl Permissions {
    #[inline]
    pub fn readonly(&self) -> bool {
        self.0.readonly()
    }

    #[inline]
    pub fn set_readonly(&mut self, readonly: bool) {
        self.0.set_readonly(readonly)
    }
}

pub struct ReadDir(fs_imp::ReadDir);

impl Iterator for ReadDir {
    type Item = Result<DirEntry>;

    #[inline]
    fn next(&mut self) -> Option<Result<DirEntry>> {
        self.0.next().map(|entry| entry.map(DirEntry))
    }
}

pub struct DirEntry(fs_imp::DirEntry);

impl DirEntry {
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.0.path()
    }

    #[inline]
    pub fn metadata(&self) -> Result<Metadata> {
        self.0.metadata().map(Metadata)
    }

    #[inline]
    pub fn file_type(&self) -> Result<FileType> {
        self.0.file_type()
    }

    #[inline]
    pub fn file_name(&self) -> OsString {
        self.0.file_name()
    }
}
