// use crate::*;
use super::*;
use crate::sync::Mutex;
use crate::*;
use core::ops::DerefMut;
use core::sync::atomic::{AtomicUsize, Ordering};
use megstd::fs::FileType;
use megstd::io::{ErrorKind, Result};

type ThisFs = RamFs;

// const BLOCK_SIZE: usize = 4096;
const FILE_SIZE_MAX: usize = i32::MAX as usize;
const INODE_MAX: usize = i16::MAX as usize;

pub struct RamFs {
    inodes: Mutex<BTreeMap<INodeType, Weak<ThisFsInodeEntity>>>,
    next_inode: AtomicUsize,

    root: Arc<ThisFsInodeEntity>,
}

impl RamFs {
    pub fn new() -> Arc<dyn FsDriver> {
        let root_inode = unsafe { INodeType::new_unchecked(2) };

        let root = Arc::new(ThisFsInodeEntity {
            inode: root_inode,
            content: ThisFsContent::new_directory(),
        });
        let fs = Self {
            inodes: Mutex::new(BTreeMap::new()),
            root: root.clone(),
            next_inode: AtomicUsize::new(root_inode.get() as usize),
        };

        fs.inodes
            .lock()
            .unwrap()
            .insert(root_inode, Arc::downgrade(&root));

        Arc::new(fs) as Arc<dyn FsDriver>
    }

    fn make_node(self: Arc<Self>, dir: INodeType, name: &str, is_dir: bool) -> Result<INodeType> {
        let dir = self.get_entity(dir).ok_or(ErrorKind::NotFound)?;
        let mut dir = match dir.content {
            ThisFsContent::File(_) => return Err(ErrorKind::NotADirectory.into()),
            ThisFsContent::Directory(ref v) => v.lock(),
        };

        dir.append_new(name, || {
            let inode = self
                .next_inode()
                .ok_or(ErrorKind::FilesystemQuotaExceeded)?;
            let content = if is_dir {
                ThisFsContent::new_directory()
            } else {
                ThisFsContent::new_file()
            };
            let entity = Arc::new(ThisFsInodeEntity { inode, content });

            self.inodes
                .lock()
                .unwrap()
                .insert(inode, Arc::downgrade(&entity));

            Ok(entity)
        })
    }

    #[inline]
    fn next_inode(&self) -> Option<INodeType> {
        self.next_inode
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                (self.inodes.lock().unwrap().len() < INODE_MAX).then(|| v + 1)
            })
            .map(|v| unsafe { INodeType::new((1 + v) as u128).unwrap_unchecked() })
            .ok()
    }

    /// Returns the entity of the inode.
    /// Also performs cache cleanup if necessary.
    #[inline]
    fn get_entity(&self, inode: INodeType) -> Option<Arc<ThisFsInodeEntity>> {
        let mut inodes = self.inodes.lock().unwrap();
        let entity = inodes.get(&inode)?;
        match entity.upgrade() {
            Some(v) => Some(v),
            None => {
                // cleanup
                inodes.remove(&inode);
                None
            }
        }
    }

    #[inline]
    fn compare_name(lhs: &str, rhs: &str) -> bool {
        lhs == rhs
    }
}

impl FsDriver for RamFs {
    fn device_name(&self) -> String {
        "ramfs".to_owned()
    }

    fn description(&self) -> Option<String> {
        None
    }

    fn root_dir(&self) -> INodeType {
        self.root.inode
    }

    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry> {
        let Some(dir) = self.get_entity(dir) else {
            return None;
        };
        match dir.content {
            ThisFsContent::File(_) => None,
            ThisFsContent::Directory(ref dir) => dir.lock().nth_child(index),
        }
    }

    fn lookup(&self, dir: INodeType, lpc: &str) -> Result<INodeType> {
        let Some(dir) = self.get_entity(dir) else {
            return Err(ErrorKind::NotFound.into());
        };

        match dir.content {
            ThisFsContent::File(_) => Err(ErrorKind::NotADirectory.into()),
            ThisFsContent::Directory(ref dir) => dir.lock().find(lpc).map(|v| v.inode()),
        }
    }

    fn open(self: Arc<Self>, inode: INodeType) -> Result<Arc<dyn FsAccessToken>> {
        self.get_entity(inode)
            .ok_or(ErrorKind::NotFound.into())
            .map(|v| Arc::new(ThisFsAccessToken { entity: v.clone() }) as Arc<dyn FsAccessToken>)
    }

    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData> {
        self.get_entity(inode).map(|v| (&*v).into())
    }

    fn mkdir(self: Arc<Self>, dir: INodeType, name: &str) -> Result<()> {
        self.make_node(dir, name, true).map(|_| ())
    }

    fn creat(self: Arc<Self>, dir: INodeType, name: &str) -> Result<Arc<dyn FsAccessToken>> {
        self.clone()
            .make_node(dir, name, false)
            .and_then(|inode| self.open(inode))
    }

    fn unlink(&self, dir: INodeType, name: &str) -> Result<()> {
        let dir = self.get_entity(dir).ok_or(ErrorKind::NotFound)?;
        let mut dir = match dir.content {
            ThisFsContent::File(_) => return Err(ErrorKind::NotADirectory.into()),
            ThisFsContent::Directory(ref dir) => dir.lock(),
        };

        let inode = dir.find(name)?.inode();

        dir.remove(name, false)?;

        // cleanup if needed
        self.get_entity(inode);

        Ok(())
    }

    fn rename(
        &self,
        old_dir: INodeType,
        old_name: &str,
        new_dir: INodeType,
        new_name: &str,
        replace: bool,
    ) -> Result<()> {
        if new_dir == old_dir {
            let dir = self.get_entity(old_dir).ok_or(ErrorKind::NotFound)?;
            let mut dir = match dir.content {
                ThisFsContent::File(_) => return Err(ErrorKind::NotADirectory.into()),
                ThisFsContent::Directory(ref dir) => dir.lock(),
            };

            let old_ = dir.link(old_name)?;

            let new_ = if replace {
                match dir.confirm_to_remove(new_name) {
                    Ok(_) => dir.link(new_name).ok(),
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => None,
                        _ => return Err(err),
                    },
                }
            } else {
                if dir.find(new_name).is_ok() {
                    return Err(ErrorKind::AlreadyExists.into());
                }
                None
            };

            if let Some(ref new_) = new_ {
                if old_.inode == new_.inode {
                    return Ok(());
                }
            }

            dir.force_rename(old_name, new_name).unwrap();

            drop(old_);
            drop(new_);
            Ok(())
        } else {
            let old_dir = self.get_entity(old_dir).ok_or(ErrorKind::NotFound)?;
            let mut old_dir = match old_dir.content {
                ThisFsContent::File(_) => return Err(ErrorKind::NotADirectory.into()),
                ThisFsContent::Directory(ref dir) => dir.lock(),
            };

            let old_ = old_dir.link(old_name)?;

            let new_dir = self.get_entity(new_dir).ok_or(ErrorKind::NotFound)?;
            let mut new_dir = match new_dir.content {
                ThisFsContent::File(_) => return Err(ErrorKind::NotADirectory.into()),
                ThisFsContent::Directory(ref dir) => dir.lock(),
            };

            let new_ = if replace {
                match new_dir.confirm_to_remove(new_name) {
                    Ok(_) => new_dir.link(new_name).ok(),
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => None,
                        _ => return Err(err),
                    },
                }
            } else {
                if new_dir.find(new_name).is_ok() {
                    return Err(ErrorKind::AlreadyExists.into());
                }
                None
            };

            if let Some(ref new_) = new_ {
                if old_.inode == new_.inode {
                    return Ok(());
                }
            }

            let dir_ent = old_dir.remove(old_name, true).unwrap();
            new_dir.append_or_replace(new_name, dir_ent.entity);

            drop(old_);
            drop(new_);
            Ok(())
        }
    }
}

struct ThisFsDirEntry {
    entity: Arc<ThisFsInodeEntity>,
    name: String,
}

impl ThisFsDirEntry {
    #[inline]
    pub fn inode(&self) -> INodeType {
        self.entity.inode
    }

    #[inline]
    pub fn name<'a>(&'a self) -> &'a str {
        self.name.as_str()
    }
}

struct ThisFsInodeEntity {
    inode: INodeType,
    content: ThisFsContent,
}

impl ThisFsInodeEntity {
    #[inline]
    pub fn file_type(&self) -> FileType {
        match self.content {
            ThisFsContent::File(_) => FileType::File,
            ThisFsContent::Directory(_) => FileType::Dir,
        }
    }

    #[inline]
    pub fn file_size(&self) -> usize {
        match self.content {
            ThisFsContent::File(ref v) => v.estimated_size(),
            ThisFsContent::Directory(ref v) => v.lock().len(),
        }
    }
}

impl From<&ThisFsInodeEntity> for FsRawMetaData {
    fn from(src: &ThisFsInodeEntity) -> Self {
        Self::new(src.inode, src.file_type(), src.file_size() as i64)
    }
}

enum ThisFsContent {
    File(ThisFsFile),
    Directory(ThisFsDirectory),
}

impl ThisFsContent {
    #[inline]
    pub fn new_file() -> Self {
        Self::File(ThisFsFile::new())
    }

    #[inline]
    pub fn new_directory() -> Self {
        Self::Directory(ThisFsDirectory::new())
    }
}

struct ThisFsFile {
    estimated_size: AtomicUsize,
    content: Mutex<Vec<u8>>,
}

impl ThisFsFile {
    #[inline]
    pub fn new() -> Self {
        Self {
            estimated_size: AtomicUsize::new(0),
            content: Mutex::new(Vec::new()),
        }
    }

    #[inline]
    pub fn estimated_size(&self) -> usize {
        self.estimated_size.load(Ordering::Relaxed)
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let content = self.content.lock().unwrap();
        if offset >= content.len() {
            return Ok(0);
        }
        let count = usize::min(buf.len(), content.len() - offset);
        unsafe {
            buf.as_mut_ptr()
                .copy_from_nonoverlapping(content.as_ptr().add(offset), count);
        }

        Ok(count)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut content = self.content.lock().unwrap();
        let Some(new_size) = offset.checked_add(buf.len()) else {
            return Err(ErrorKind::FilesystemQuotaExceeded.into());
        };
        if new_size > FILE_SIZE_MAX {
            return Err(ErrorKind::FilesystemQuotaExceeded.into());
        }
        if content.capacity() < new_size {
            let additional = new_size - content.len();
            match content.try_reserve(additional) {
                Ok(_) => (),
                Err(_) => return Err(ErrorKind::StorageFull.into()),
            }
        }
        if offset < content.len() {
            content.resize(offset, 0);
        }
        let count = buf.len();
        if count > 0 {
            unsafe {
                content
                    .as_mut_ptr()
                    .add(offset)
                    .copy_from_nonoverlapping(buf.as_ptr(), count);
                if content.len() < new_size {
                    content.set_len(new_size);
                    self.estimated_size.store(content.len(), Ordering::SeqCst);
                }
            }
        }

        Ok(count)
    }

    pub fn truncate(&self, length: OffsetType) -> Result<()> {
        let length = if length >= 0 {
            length as usize
        } else {
            return Err(ErrorKind::InvalidInput.into());
        };
        let mut content = self.content.lock().unwrap();
        if content.len() <= length {
            content.resize(length, 0);
            Ok(())
        } else {
            Err(ErrorKind::InvalidInput.into())
        }
    }
}

struct ThisFsDirectory {
    content: Mutex<ThisFsDirectoryContent>,
}

impl ThisFsDirectory {
    #[inline]
    pub fn new() -> Self {
        Self {
            content: Mutex::new(ThisFsDirectoryContent::new()),
        }
    }

    #[inline]
    pub fn lock<'a>(&'a self) -> impl DerefMut<Target = ThisFsDirectoryContent> + 'a {
        self.content.lock().unwrap()
    }
}

struct ThisFsDirectoryContent {
    content: Vec<ThisFsDirEntry>,
}

impl ThisFsDirectoryContent {
    #[inline]
    fn new() -> Self {
        Self {
            content: Vec::new(),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.content.len()
    }

    #[inline]
    fn find(&self, name: &str) -> Result<&ThisFsDirEntry> {
        self.content
            .iter()
            .find(|v| ThisFs::compare_name(v.name(), name))
            .ok_or(ErrorKind::NotFound.into())
    }

    #[inline]
    fn find_index(&self, name: &str) -> Result<usize> {
        self.content
            .iter()
            .position(|v| ThisFs::compare_name(v.name(), name))
            .ok_or(ErrorKind::NotFound.into())
    }

    #[inline]
    fn nth_child(&self, index: usize) -> Option<FsRawDirEntry> {
        let dir_ent = self.content.get(index)?;
        Some(FsRawDirEntry::new(
            dir_ent.inode(),
            dir_ent.name(),
            (&*dir_ent.entity).into(),
        ))
    }

    #[inline]
    fn link(&self, name: &str) -> Result<Arc<ThisFsInodeEntity>> {
        self.find(name).map(|v| v.entity.clone())
    }

    fn append_new<F>(&mut self, name: &str, entity: F) -> Result<INodeType>
    where
        F: FnOnce() -> Result<Arc<ThisFsInodeEntity>>,
    {
        if self.find(name).is_ok() {
            return Err(ErrorKind::AlreadyExists.into());
        }

        let entity = entity()?;
        let inode = entity.inode;

        self.content.push(ThisFsDirEntry {
            entity,
            name: name.to_owned(),
        });

        Ok(inode)
    }

    /// # Returns
    ///
    /// * Ok
    ///   * File exists and ready to be removed
    /// * Err(NotFound)
    ///   * File does not exist
    /// * Err(DirectoryNotEmpty)
    ///   * Files exist as directories, but directory contents are not empty
    fn confirm_to_remove(&self, name: &str) -> Result<()> {
        let dir_ent = self.find(name)?;
        match dir_ent.entity.content {
            ThisFsContent::File(_) => Ok(()),
            ThisFsContent::Directory(ref children) => {
                if children.lock().len() > 0 {
                    Err(ErrorKind::DirectoryNotEmpty.into())
                } else {
                    Ok(())
                }
            }
        }
    }

    fn remove(&mut self, name: &str, force: bool) -> Result<ThisFsDirEntry> {
        if !force {
            self.confirm_to_remove(name)?;
        }

        let index = self.find_index(name)?;
        Ok(self.content.remove(index))
    }

    fn force_rename(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        let new_index = self.find_index(new_name).ok();
        let old_index = self.find_index(old_name)?;

        self.content[old_index].name = new_name.to_owned();
        if let Some(new_index) = new_index {
            self.content.remove(new_index);
        }

        Ok(())
    }

    fn append_or_replace(&mut self, name: &str, entity: Arc<ThisFsInodeEntity>) {
        match self
            .content
            .iter_mut()
            .find(|dir_ent| ThisFs::compare_name(dir_ent.name(), name))
        {
            Some(dir_ent) => dir_ent.entity = entity,
            None => self.content.push(ThisFsDirEntry {
                name: name.to_owned(),
                entity,
            }),
        }
    }
}

struct ThisFsAccessToken {
    entity: Arc<ThisFsInodeEntity>,
}

impl FsAccessToken for ThisFsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData> {
        Some((&*self.entity).into())
    }

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        match self.entity.content {
            ThisFsContent::File(ref content) => content.read(offset as usize, buf),
            ThisFsContent::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }

    fn write_data(&self, offset: OffsetType, buf: &[u8]) -> Result<usize> {
        match self.entity.content {
            ThisFsContent::File(ref content) => content.write(offset as usize, buf),
            ThisFsContent::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }

    fn truncate(&self, length: OffsetType) -> Result<()> {
        match self.entity.content {
            ThisFsContent::File(ref content) => content.truncate(length),
            ThisFsContent::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }
}
