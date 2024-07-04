// impl Path for MEG-OS
// Most of them are clones of Rust's original definition.

use crate::prelude::*;
use core::borrow::Borrow;
use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::iter::FusedIterator;
use core::mem::transmute;
use core::ops::Deref;
use core::str;

pub use crate::sys::path::MAIN_SEPARATOR;

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    inner: OsStr,
}

impl Path {
    #[inline]
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        &self.inner
    }

    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str()
    }

    #[inline]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.inner.to_os_string())
    }

    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.has_root()
    }

    #[inline]
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    #[inline]
    pub fn has_root(&self) -> bool {
        self.components()
            .next()
            .filter(|v| matches!(v, Component::RootDir))
            .is_some()
    }

    #[inline]
    pub fn parent(&self) -> Option<&Path> {
        let mut components = self.components();
        components.next_back().and_then(|v| match v {
            Component::RootDir => None,
            _ => Some(components.as_path()),
        })
    }

    #[inline]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { next: Some(&self) }
    }

    #[inline]
    #[track_caller]
    pub fn file_name(&self) -> Option<&OsStr> {
        for item in self.components().rev() {
            match item {
                Component::Normal(v) => return Some(v),
                Component::CurDir => (),
                _ => return None,
            }
        }
        None
    }

    pub fn strip_prefix<P: AsRef<Path>>(&self, base: P) -> Result<&Path, StripPrefixError> {
        let base = base.as_ref();

        let mut iter = self.iter();
        for (a, b) in base.iter().zip(&mut iter) {
            if a != b {
                return Err(StripPrefixError(()));
            }
        }

        Ok(iter.as_path())
    }

    pub fn starts_with<P: AsRef<Path>>(&self, base: P) -> bool {
        let base = base.as_ref();
        self.iter()
            .zip(base.iter())
            .filter(|(a, b)| a != b)
            .next()
            .is_none()
    }

    pub fn ends_with<P: AsRef<Path>>(&self, child: P) -> bool {
        let child = child.as_ref();
        self.iter()
            .rev()
            .zip(child.iter().rev())
            .filter(|(a, b)| a != b)
            .next()
            .is_none()
    }

    pub fn file_stem(&self) -> Option<&OsStr> {
        let file_name = self.file_name()?;
        let s = file_name.to_str()?;

        let Some(first_dot_index) = s.find(".") else {
            return Some(file_name);
        };
        let Some(last_dot_index) = s.rfind(".") else {
            return Some(file_name);
        };
        if first_dot_index == 0 && last_dot_index == 0 {
            Some(file_name)
        } else {
            s.get(..last_dot_index).map(|v| OsStr::new(v))
        }
    }

    // #[deprecated = "Currently unavailable"]
    // #[track_caller]
    // pub fn file_prefix(&self) -> Option<&OsStr> {
    //     todo!()
    // }

    pub fn extension(&self) -> Option<&OsStr> {
        let file_name = self.file_name()?;
        let file_name = file_name.to_str()?;
        let first_dot_index = file_name.find(".")?;
        let last_dot_index = file_name.rfind(".")?;
        if first_dot_index == 0 && last_dot_index == 0 {
            None
        } else {
            file_name.get(last_dot_index + 1..).map(|v| OsStr::new(v))
        }
    }

    #[must_use]
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        let mut buf = self.to_path_buf();
        buf.push(path);
        buf
    }

    pub fn with_file_name<S: AsRef<OsStr>>(&self, file_name: S) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_file_name(file_name.as_ref());
        buf
    }

    pub fn with_extension<S: AsRef<OsStr>>(&self, extension: S) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_extension(extension.as_ref());
        buf
    }

    #[inline]
    pub fn components(&self) -> Components<'_> {
        Components::new(self)
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.components(),
        }
    }

    // #[inline]
    // pub fn canonicalize(&self) -> io::Result<PathBuf> {
    //     // fs::canonicalize(self)
    // }
}

impl fmt::Debug for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

impl AsRef<OsStr> for Path {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner
    }
}

impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for str {
    #[inline]
    fn as_ref(&self) -> &Path {
        unsafe { transmute(self) }
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;

    fn to_owned(&self) -> Self::Owned {
        self.to_path_buf()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StripPrefixError(());

#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathBuf {
    inner: OsString,
}

impl PathBuf {
    #[inline]
    pub fn new() -> PathBuf {
        Self {
            inner: OsString::new(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> PathBuf {
        Self {
            inner: OsString::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn as_path(&self) -> &Path {
        self
    }

    #[inline]
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if path.is_absolute() {
            self.inner.clear();
            self.inner.push(path);
        } else {
            if self.inner.len() > 0 && !self.inner.to_str().unwrap().ends_with(MAIN_SEPARATOR) {
                self.inner.push(MAIN_SEPARATOR);
            }
            self.inner.push(path);
        }
    }

    #[inline]
    pub fn pop(&mut self) -> bool {
        match self.parent().map(|v| v.inner.bytes().len()) {
            Some(len) => {
                self.inner.as_mut_vec().truncate(len);
                true
            }
            None => false,
        }
    }

    #[inline]
    pub fn set_file_name<S: AsRef<OsStr>>(&mut self, file_name: S) {
        let file_name = file_name.as_ref();
        if self.file_name().is_some() {
            self.pop();
        }
        self.push(file_name);
    }

    #[inline]
    pub fn set_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        let Some(file_name) = self.file_name() else {
            return false;
        };
        let extension = extension.as_ref();

        let last_index = if self.extension().is_some() {
            file_name
                .bytes()
                .iter()
                .enumerate()
                .rfind(|(_, v)| **v == b'.')
                .map(|(i, _)| i)
        } else {
            None
        }
        .unwrap_or(file_name.len());

        let mut file_name = file_name.to_owned();
        file_name.as_mut_vec().truncate(last_index);
        if !extension.is_empty() {
            file_name.push(".");
            file_name.push(extension);
        }

        self.set_file_name(file_name);
        true
    }

    #[inline]
    pub fn into_os_string(self) -> OsString {
        self.inner
    }

    #[inline]
    pub fn into_boxed_path(self) -> Box<Path> {
        let rw = Box::into_raw(self.inner.into_boxed_os_str()) as *mut Path;
        unsafe { Box::from_raw(rw) }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    // pub fn shrink_to(&mut self, min_capacity: usize)
}

impl AsRef<OsStr> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner[..]
    }
}

impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl Deref for PathBuf {
    type Target = Path;
    #[inline]
    fn deref(&self) -> &Path {
        Path::new(&self.inner)
    }
}

impl From<&str> for PathBuf {
    #[inline]
    fn from(value: &str) -> Self {
        OsStr::new(value).to_os_string().into()
    }
}

impl From<OsString> for PathBuf {
    #[inline]
    fn from(s: OsString) -> PathBuf {
        PathBuf { inner: s }
    }
}

impl From<PathBuf> for OsString {
    #[inline]
    fn from(path_buf: PathBuf) -> OsString {
        path_buf.inner
    }
}

impl Borrow<Path> for PathBuf {
    #[inline]
    fn borrow(&self) -> &Path {
        self.as_path()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Ancestors<'a> {
    next: Option<&'a Path>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        self.next = next.and_then(Path::parent);
        next
    }
}

impl FusedIterator for Ancestors<'_> {}

#[allow(dead_code)]
pub struct Components<'a> {
    path: &'a [u8],
    front: usize,
    back: usize,
}

impl<'a> Components<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path: path.inner.bytes(),
            front: 0,
            back: path.inner.len(),
        }
    }

    pub fn as_path(&self) -> &'a Path {
        Path::new(unsafe {
            core::str::from_utf8_unchecked(self.path.get_unchecked(self.front..self.back))
        })
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let c = self.path.get(self.front)?;
            if *c == b'/' {
                self.front += 1;
                if self.front == 1 {
                    return Some(Component::RootDir);
                }
            } else {
                break;
            }
        }

        let begin = self.front;
        let end = loop {
            self.front += 1;
            let end = self.front;
            match self.path.get(end) {
                Some(c) => {
                    if *c == b'/' {
                        break end;
                    }
                }
                None => break end,
            }
        };

        loop {
            match self.path.get(self.front) {
                Some(c) => {
                    if *c == b'/' {
                        self.front += 1;
                        continue;
                    }
                }
                None => (),
            }
            break;
        }

        self.path.get(begin..end).map(|raw| {
            let raw_str = unsafe { core::str::from_utf8_unchecked(raw) };
            match raw_str {
                "." => Component::CurDir,
                ".." => Component::ParentDir,
                _ => Component::Normal(OsStr::new(raw_str)),
            }
        })
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.back == 0 {
            return None;
        }
        while self.back > 0 {
            let c = self.path[self.back - 1];
            if c != b'/' {
                break;
            }
            self.back -= 1;
        }

        let end = self.back;
        while self.back > 0 {
            let c = self.path[self.back - 1];
            if c == b'/' {
                break;
            }
            self.back -= 1;
        }
        let begin = self.back;

        while self.back > 1 {
            let c = self.path[self.back - 1];
            if c == b'/' {
                self.back -= 1;
                continue;
            }
            break;
        }

        self.path.get(begin..end).map(|raw| {
            let raw_str = unsafe { core::str::from_utf8_unchecked(raw) };
            match raw_str {
                "" => {
                    #[cfg(test)]
                    if begin != 0 {
                        unreachable!();
                    }

                    Component::RootDir
                }
                "." => Component::CurDir,
                ".." => Component::ParentDir,
                _ => Component::Normal(OsStr::new(raw_str)),
            }
        })
    }
}

impl FusedIterator for Components<'_> {}

impl AsRef<OsStr> for Components<'_> {
    fn as_ref(&self) -> &OsStr {
        unsafe { transmute(self.as_path()) }
    }
}

impl AsRef<Path> for Components<'_> {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

pub struct Iter<'a> {
    inner: Components<'a>,
}

impl<'a> Iter<'a> {
    #[inline]
    pub fn as_path(&self) -> &'a Path {
        self.inner.as_path()
    }
}

impl<'a> AsMut<Iter<'a>> for Iter<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut Iter<'a> {
        self
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a OsStr;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| v.as_os_str())
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|v| v.as_os_str())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Component<'a> {
    Prefix(PrefixComponent<'a>),
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a OsStr),
}

impl<'a> Component<'a> {
    pub fn as_os_str(self) -> &'a OsStr {
        match self {
            Component::Prefix(p) => p.as_os_str(),
            Component::RootDir => OsStr::new(MAIN_SEPARATOR),
            Component::CurDir => OsStr::new("."),
            Component::ParentDir => OsStr::new(".."),
            Component::Normal(path) => path,
        }
    }
}

/// Unusable in meg-os
#[derive(Copy, Clone, Eq, Debug)]
pub struct PrefixComponent<'a> {
    /// The prefix as an unparsed `OsStr` slice.
    raw: &'a OsStr,

    /// The parsed prefix data.
    parsed: Prefix<'a>,
}

impl<'a> PrefixComponent<'a> {
    /// Returns the parsed prefix data.
    ///
    /// See [`Prefix`]'s documentation for more information on the different
    /// kinds of prefixes.
    #[inline]
    pub fn kind(&self) -> Prefix<'a> {
        self.parsed
    }

    /// Returns the raw [`OsStr`] slice for this prefix.
    #[inline]
    pub fn as_os_str(&self) -> &'a OsStr {
        self.raw
    }
}

impl<'a> cmp::PartialEq for PrefixComponent<'a> {
    #[inline]
    fn eq(&self, other: &PrefixComponent<'a>) -> bool {
        cmp::PartialEq::eq(&self.parsed, &other.parsed)
    }
}

impl<'a> cmp::PartialOrd for PrefixComponent<'a> {
    #[inline]
    fn partial_cmp(&self, other: &PrefixComponent<'a>) -> Option<cmp::Ordering> {
        cmp::PartialOrd::partial_cmp(&self.parsed, &other.parsed)
    }
}

impl cmp::Ord for PrefixComponent<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        cmp::Ord::cmp(&self.parsed, &other.parsed)
    }
}

impl Hash for PrefixComponent<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.parsed.hash(h);
    }
}

/// Unusable in meg-os
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub enum Prefix<'a> {
    /// Verbatim prefix, e.g., `\\?\cat_pics`.
    Verbatim(&'a OsStr),
    /// Verbatim prefix using Windows' _**U**niform **N**aming **C**onvention_,
    /// e.g., `\\?\UNC\server\share`.
    VerbatimUNC(&'a OsStr, &'a OsStr),
    /// Verbatim disk prefix, e.g., `\\?\C:`.
    VerbatimDisk(u8),
    /// Device namespace prefix, e.g., `\\.\COM42`.
    DeviceNS(&'a OsStr),
    /// Prefix using Windows' _**U**niform **N**aming **C**onvention_, e.g.
    /// `\\server\share`.
    UNC(&'a OsStr, &'a OsStr),
    /// Prefix `C:` for the given disk drive.
    Disk(u8),
}

impl Prefix<'_> {
    pub fn is_verbatim(&self) -> bool {
        use self::Prefix::*;
        matches!(*self, Verbatim(_) | VerbatimDisk(_) | VerbatimUNC(..))
    }

    #[allow(dead_code)]
    #[inline]
    fn is_drive(&self) -> bool {
        matches!(*self, Prefix::Disk(_))
    }

    #[allow(dead_code)]
    #[inline]
    fn has_implicit_root(&self) -> bool {
        !self.is_drive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_file_name() {
        assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
        assert_eq!(
            Some(OsStr::new("foo.txt")),
            Path::new("tmp/foo.txt").file_name()
        );
        assert_eq!(
            Some(OsStr::new("foo.txt")),
            Path::new("foo.txt/.").file_name()
        );
        assert_eq!(
            Some(OsStr::new("foo.txt")),
            Path::new("foo.txt/.//").file_name()
        );
        assert_eq!(None, Path::new("foo.txt/..").file_name());
        assert_eq!(None, Path::new("/").file_name());
    }

    #[test]
    fn components() {
        let mut components = Path::new("/tmp/foo/bar.txt").components();
        components.next();
        components.next();

        assert_eq!(Path::new("foo/bar.txt"), components.as_path());

        let path = Path::new("//tmp/.//foo//../bar.txt/.//");
        let components = path.components().collect::<Vec<_>>();
        assert_eq!(
            &components,
            &[
                Component::RootDir,
                Component::Normal("tmp".as_ref()),
                Component::CurDir,
                Component::Normal("foo".as_ref()),
                Component::ParentDir,
                Component::Normal("bar.txt".as_ref()),
                Component::CurDir,
            ]
        );

        let components = path.components().rev().collect::<Vec<_>>();
        assert_eq!(
            &components,
            &[
                Component::CurDir,
                Component::Normal("bar.txt".as_ref()),
                Component::ParentDir,
                Component::Normal("foo".as_ref()),
                Component::CurDir,
                Component::Normal("tmp".as_ref()),
                Component::RootDir,
            ]
        );
    }

    #[test]
    fn iter() {
        let mut it = Path::new("/tmp/foo.txt").iter();
        assert_eq!(
            it.next(),
            Some(OsStr::new(&path::MAIN_SEPARATOR.to_string()))
        );
        assert_eq!(it.next(), Some(OsStr::new("tmp")));
        assert_eq!(it.next(), Some(OsStr::new("foo.txt")));
        assert_eq!(it.next(), None)
    }

    #[test]
    fn parent() {
        let path = Path::new("/foo/bar");
        let parent = path.parent().unwrap();
        assert_eq!(parent, Path::new("/foo"));

        let grand_parent = parent.parent().unwrap();
        assert_eq!(grand_parent, Path::new("/"));
        assert_eq!(grand_parent.parent(), None);

        let relative_path = Path::new("foo/bar");
        let parent = relative_path.parent();
        assert_eq!(parent, Some(Path::new("foo")));
        let grand_parent = parent.and_then(Path::parent);
        assert_eq!(grand_parent, Some(Path::new("")));
        let great_grand_parent = grand_parent.and_then(Path::parent);
        assert_eq!(great_grand_parent, None);
    }

    #[test]
    fn ancestors() {
        let mut ancestors = Path::new("/foo/bar").ancestors();
        assert_eq!(ancestors.next(), Some(Path::new("/foo/bar")));
        assert_eq!(ancestors.next(), Some(Path::new("/foo")));
        assert_eq!(ancestors.next(), Some(Path::new("/")));
        assert_eq!(ancestors.next(), None);

        let mut ancestors = Path::new("../foo/bar").ancestors();
        assert_eq!(ancestors.next(), Some(Path::new("../foo/bar")));
        assert_eq!(ancestors.next(), Some(Path::new("../foo")));
        assert_eq!(ancestors.next(), Some(Path::new("..")));
        assert_eq!(ancestors.next(), Some(Path::new("")));
        assert_eq!(ancestors.next(), None);
    }

    #[test]
    fn extension() {
        assert_eq!(Path::new(".gitignore").extension(), None);
        assert_eq!("d", Path::new("a.b/c.d").extension().unwrap());
        assert_eq!("rs", Path::new(".foo.rs").extension().unwrap());
        assert_eq!("rs", Path::new("foo.rs").extension().unwrap());
        assert_eq!("gz", Path::new("foo.tar.gz").extension().unwrap());
    }

    #[test]
    fn file_stem() {
        assert_eq!(".gitignore", Path::new(".gitignore").file_stem().unwrap());
        assert_eq!("c", Path::new("a.b/c.d").file_stem().unwrap());
        assert_eq!(".foo", Path::new(".foo.rs").file_stem().unwrap());
        assert_eq!("foo", Path::new("foo.rs").file_stem().unwrap());
        assert_eq!("foo.tar", Path::new("foo.tar.gz").file_stem().unwrap());
    }

    // #[test]
    // fn file_prefix() {
    //     assert_eq!("foo", Path::new("foo.rs").file_prefix().unwrap());
    //     assert_eq!("foo", Path::new("foo.tar.gz").file_prefix().unwrap());
    // }

    #[test]
    fn join() {
        assert_eq!(
            Path::new("/etc").join("passwd"),
            PathBuf::from("/etc/passwd")
        );
        assert_eq!(Path::new("/etc").join("/bin/sh"), PathBuf::from("/bin/sh"));
    }

    #[test]
    fn with_file_name() {
        let path = Path::new("/tmp/foo.png");
        assert_eq!(path.with_file_name("bar"), PathBuf::from("/tmp/bar"));
        assert_eq!(
            path.with_file_name("bar.txt"),
            PathBuf::from("/tmp/bar.txt")
        );

        let path = Path::new("/tmp");
        assert_eq!(path.with_file_name("var"), PathBuf::from("/var"));
    }

    #[test]
    fn starts_with() {
        let path = Path::new("/etc/passwd");

        assert!(path.starts_with("/etc"));
        assert!(path.starts_with("/etc/"));
        assert!(path.starts_with("/etc/passwd"));
        assert!(path.starts_with("/etc/passwd/")); // extra slash is okay
        assert!(path.starts_with("/etc/passwd///")); // multiple extra slashes are okay

        assert!(!path.starts_with("/e"));
        assert!(!path.starts_with("/etc/passwd.txt"));

        assert!(!Path::new("/etc/foo.rs").starts_with("/etc/foo"));
    }

    #[test]
    fn ends_with() {
        let path = Path::new("/etc/resolv.conf");

        assert!(path.ends_with("resolv.conf"));
        assert!(path.ends_with("etc/resolv.conf"));
        assert!(path.ends_with("/etc/resolv.conf"));

        assert!(!path.ends_with("/resolv.conf"));
        assert!(!path.ends_with("conf")); // use .extension() instead
    }

    #[test]
    fn set_extension() {
        let mut p = PathBuf::from("/feel/the");

        p.set_extension("force");
        assert_eq!(Path::new("/feel/the.force"), p.as_path());

        p.set_extension("dark.side");
        assert_eq!(Path::new("/feel/the.dark.side"), p.as_path());

        p.set_extension("cookie");
        assert_eq!(Path::new("/feel/the.dark.cookie"), p.as_path());

        p.set_extension("");
        assert_eq!(Path::new("/feel/the.dark"), p.as_path());

        p.set_extension("");
        assert_eq!(Path::new("/feel/the"), p.as_path());

        p.set_extension("");
        assert_eq!(Path::new("/feel/the"), p.as_path());
    }

    #[test]
    fn strip_prefix() {
        let path = Path::new("/test/haha/foo.txt");

        assert_eq!(path.strip_prefix("/"), Ok(Path::new("test/haha/foo.txt")));
        assert_eq!(path.strip_prefix("/test"), Ok(Path::new("haha/foo.txt")));
        assert_eq!(path.strip_prefix("/test/"), Ok(Path::new("haha/foo.txt")));
        assert_eq!(path.strip_prefix("/test/haha/foo.txt"), Ok(Path::new("")));
        assert_eq!(path.strip_prefix("/test/haha/foo.txt/"), Ok(Path::new("")));

        assert!(path.strip_prefix("test").is_err());
        assert!(path.strip_prefix("/haha").is_err());

        let prefix = PathBuf::from("/test/");
        assert_eq!(path.strip_prefix(prefix), Ok(Path::new("haha/foo.txt")));
    }
}
