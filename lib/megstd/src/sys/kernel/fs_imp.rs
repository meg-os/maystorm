// FileSystem Implementation

use crate::fs::*;
use crate::io::Result;
use crate::path::*;
use crate::prelude::*;
use crate::sys::fcntl::*;

pub struct File {
    _phantom: (),
}

impl File {
    pub fn open<P: AsRef<Path>>(_path: P, _options: OpenOptions) -> Result<File> {
        todo!()
    }

    pub fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    pub fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        todo!()
    }

    pub fn flush(&mut self) -> Result<()> {
        todo!()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct OpenOptions(u32);

impl OpenOptions {
    pub const READ: Self = Self(0b0000_0001);
    pub const WRITE: Self = Self(0b0000_0010);
    pub const APPEND: Self = Self(0b0001_0000);
    pub const TRUNC: Self = Self(0b0010_0000);
    pub const CREAT: Self = Self(0b0100_0000);
    pub const EXCL: Self = Self(0b1000_0000);

    #[inline]
    pub fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub fn set(&mut self, bit: Self, value: bool) {
        if value {
            self.0 |= bit.0;
        } else {
            self.0 &= !bit.0;
        }
    }

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn contains(&self, bit: Self) -> bool {
        (self.0 & bit.0) == bit.0
    }

    #[inline]
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.set(Self::READ, read);
        self
    }

    #[inline]
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.set(Self::WRITE, write);
        self
    }

    #[inline]
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.set(Self::APPEND, append);
        self
    }

    #[inline]
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.set(Self::TRUNC, truncate);
        self
    }

    #[inline]
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.set(Self::CREAT, create);
        self
    }

    #[inline]
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.set(Self::EXCL, create_new);
        self
    }

    pub fn build(&self) -> usize {
        let mut f = if self.contains(Self::WRITE) {
            if self.contains(Self::READ) {
                O_RDWR
            } else {
                O_WRONLY
            }
        } else {
            O_RDONLY
        };
        if self.contains(Self::APPEND) {
            f |= O_APPEND;
        }
        if self.contains(Self::TRUNC) {
            f |= O_TRUNC;
        }
        if self.contains(Self::CREAT) {
            f |= O_CREAT;
        }
        if self.contains(Self::EXCL) {
            f |= O_CREAT | O_EXCL;
        }
        f
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    _phantom: (),
}

impl Metadata {
    pub fn file_type(&self) -> FileType {
        todo!()
    }

    #[inline]
    pub fn len(&self) -> u64 {
        todo!()
    }

    #[inline]
    pub fn permissions(&self) -> Permissions {
        todo!()
    }

    // pub fn modified(&self) -> Result<SystemTime>
    // pub fn accessed(&self) -> Result<SystemTime>
    // pub fn created(&self) -> Result<SystemTime>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(usize);

impl Permissions {
    pub fn readonly(&self) -> bool {
        todo!()
    }

    pub fn set_readonly(&mut self, _readonly: bool) {
        todo!()
    }
}

pub struct ReadDir {
    _phantom: (),
}

impl Iterator for ReadDir {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Result<DirEntry>> {
        todo!()
    }
}

pub struct DirEntry {
    _phantom: (),
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        todo!()
    }

    pub fn metadata(&self) -> Result<Metadata> {
        todo!()
    }

    pub fn file_type(&self) -> Result<FileType> {
        todo!()
    }

    pub fn file_name(&self) -> OsString {
        todo!()
    }
}
