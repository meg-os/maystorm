// impl OsString for MEG-OS
// Most of them are clones of Rust's original definition.

use crate::prelude::*;
use alloc::borrow::Cow;
use alloc::collections::TryReserveError;
use core::borrow::Borrow;
use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::mem;
use core::ops::{self, Deref, DerefMut};
use core::str;

#[repr(transparent)]
#[derive(Eq)]
pub struct OsStr {
    inner: Slice,
}

impl OsStr {
    #[inline]
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &OsStr {
        s.as_ref()
    }

    #[inline]
    fn from_inner(inner: &Slice) -> &OsStr {
        unsafe { &*(inner as *const Slice as *const OsStr) }
    }

    #[inline]
    fn from_inner_mut(inner: &mut Slice) -> &mut OsStr {
        unsafe { &mut *(inner as *mut Slice as *mut OsStr) }
    }

    #[inline]
    pub fn to_os_string(&self) -> OsString {
        OsString {
            inner: self.inner.to_owned(),
        }
    }

    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.inner.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.inner.len()
    }

    #[inline]
    pub fn into_os_string(&self) -> OsString {
        OsString {
            inner: self.inner.to_owned(),
        }
    }

    #[inline]
    pub(crate) fn bytes(&self) -> &[u8] {
        unsafe { &*(&self.inner as *const _ as *const [u8]) }
    }
}

impl fmt::Debug for OsStr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_str() {
            Some(v) => fmt::Debug::fmt(v, formatter),
            None => Ok(()),
        }
    }
}

impl Default for &OsStr {
    #[inline]
    fn default() -> Self {
        OsStr::new("")
    }
}

impl PartialEq for OsStr {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.bytes().eq(other.bytes())
    }
}

impl PartialEq<str> for OsStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        *self == *OsStr::new(other)
    }
}

impl PartialEq<OsStr> for str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        *other == *OsStr::new(self)
    }
}

impl PartialOrd for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<cmp::Ordering> {
        self.bytes().partial_cmp(other.bytes())
    }
    #[inline]
    fn lt(&self, other: &OsStr) -> bool {
        self.bytes().lt(other.bytes())
    }
    #[inline]
    fn le(&self, other: &OsStr) -> bool {
        self.bytes().le(other.bytes())
    }
    #[inline]
    fn gt(&self, other: &OsStr) -> bool {
        self.bytes().gt(other.bytes())
    }
    #[inline]
    fn ge(&self, other: &OsStr) -> bool {
        self.bytes().ge(other.bytes())
    }
}

impl PartialOrd<str> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.partial_cmp(OsStr::new(other))
    }
}

impl Ord for OsStr {
    #[inline]
    fn cmp(&self, other: &OsStr) -> cmp::Ordering {
        self.bytes().cmp(other.bytes())
    }
}

impl Hash for OsStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes().hash(state)
    }
}

impl ToOwned for OsStr {
    type Owned = OsString;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        self.to_os_string()
    }
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OsString {
    inner: Buf,
}

impl OsString {
    #[inline]
    pub fn new() -> OsString {
        OsString {
            inner: Buf::from_string(String::new()),
        }
    }

    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        self
    }

    #[inline]
    pub fn push<T: AsRef<OsStr>>(&mut self, s: T) {
        self.inner.push_slice(&s.as_ref().inner)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> OsString {
        Self {
            inner: Buf::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
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

    #[inline]
    pub fn into_boxed_os_str(self) -> Box<OsStr> {
        let rw = Box::into_raw(self.inner.into_box()) as *mut OsStr;
        unsafe { Box::from_raw(rw) }
    }

    #[inline]
    pub(crate) fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.inner.inner
    }

    pub fn into_string(self) -> Result<String, OsString> {
        String::from_utf8(self.inner.inner).map_err(|e| Self {
            inner: Buf {
                inner: e.into_bytes(),
            },
        })
    }
}

impl Deref for OsString {
    type Target = OsStr;

    #[inline]
    fn deref(&self) -> &OsStr {
        &self[..]
    }
}

impl DerefMut for OsString {
    #[inline]
    fn deref_mut(&mut self) -> &mut OsStr {
        &mut self[..]
    }
}

impl Default for OsString {
    #[inline]
    fn default() -> OsString {
        OsString::new()
    }
}

impl ops::Index<ops::RangeFull> for OsString {
    type Output = OsStr;

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &OsStr {
        OsStr::from_inner(self.inner.as_slice())
    }
}

impl ops::IndexMut<ops::RangeFull> for OsString {
    #[inline]
    fn index_mut(&mut self, _index: ops::RangeFull) -> &mut OsStr {
        OsStr::from_inner_mut(self.inner.as_mut_slice())
    }
}

impl AsRef<OsStr> for OsStr {
    fn as_ref(&self) -> &OsStr {
        self
    }
}

impl AsRef<OsStr> for str {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        OsStr::from_inner(Slice::from_str(self))
    }
}

impl AsRef<OsStr> for String {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        (&**self).as_ref()
    }
}

impl AsRef<OsStr> for OsString {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}

impl fmt::Debug for OsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

impl Borrow<OsStr> for OsString {
    #[inline]
    fn borrow(&self) -> &OsStr {
        self.as_os_str()
    }
}

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Slice {
    pub inner: [u8],
}

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Buf {
    pub inner: Vec<u8>,
}

impl Buf {
    pub fn from_string(s: String) -> Buf {
        Buf {
            inner: s.into_bytes(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Buf {
        Buf {
            inner: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }

    #[inline]
    pub fn as_slice(&self) -> &Slice {
        // SAFETY: Slice just wraps [u8],
        // and &*self.inner is &[u8], therefore
        // transmuting &[u8] to &Slice is safe.
        unsafe { mem::transmute(&*self.inner) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut Slice {
        // SAFETY: Slice just wraps [u8],
        // and &mut *self.inner is &mut [u8], therefore
        // transmuting &mut [u8] to &mut Slice is safe.
        unsafe { mem::transmute(&mut *self.inner) }
    }

    pub fn into_string(self) -> Result<String, Buf> {
        String::from_utf8(self.inner).map_err(|p| Buf {
            inner: p.into_bytes(),
        })
    }

    pub fn push_slice(&mut self, s: &Slice) {
        self.inner.extend_from_slice(&s.inner)
    }

    #[inline]
    pub fn into_box(self) -> Box<Slice> {
        unsafe { mem::transmute(self.inner.into_boxed_slice()) }
    }

    #[inline]
    pub fn from_box(boxed: Box<Slice>) -> Buf {
        let inner: Box<[u8]> = unsafe { mem::transmute(boxed) };
        Buf {
            inner: inner.into_vec(),
        }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        self.as_slice().into_arc()
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        self.as_slice().into_rc()
    }
}

impl Slice {
    #[inline]
    fn from_u8_slice(s: &[u8]) -> &Slice {
        unsafe { mem::transmute(s) }
    }

    #[inline]
    pub fn from_str(s: &str) -> &Slice {
        Slice::from_u8_slice(s.as_bytes())
    }

    pub fn to_str(&self) -> Option<&str> {
        str::from_utf8(&self.inner).ok()
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }

    pub fn to_owned(&self) -> Buf {
        Buf {
            inner: self.inner.to_vec(),
        }
    }

    pub fn clone_into(&self, buf: &mut Buf) {
        self.inner.clone_into(&mut buf.inner)
    }

    #[inline]
    pub fn into_box(&self) -> Box<Slice> {
        let boxed: Box<[u8]> = self.inner.into();
        unsafe { mem::transmute(boxed) }
    }

    pub fn empty_box() -> Box<Slice> {
        let boxed: Box<[u8]> = Default::default();
        unsafe { mem::transmute(boxed) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        let arc: Arc<[u8]> = Arc::from(&self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Slice) }
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        let rc: Rc<[u8]> = Rc::from(&self.inner);
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Slice) }
    }

    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.inner.make_ascii_lowercase()
    }

    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.inner.make_ascii_uppercase()
    }

    #[inline]
    pub fn to_ascii_lowercase(&self) -> Buf {
        Buf {
            inner: self.inner.to_ascii_lowercase(),
        }
    }

    #[inline]
    pub fn to_ascii_uppercase(&self) -> Buf {
        Buf {
            inner: self.inner.to_ascii_uppercase(),
        }
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.inner.is_ascii()
    }

    #[inline]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        self.inner.eq_ignore_ascii_case(&other.inner)
    }
}
