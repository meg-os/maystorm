use core::{
    intrinsics::transmute,
    sync::atomic::{AtomicU64, Ordering},
};

pub type FloatType = f64;
type InnerType = u64;
type InnerAtomicType = AtomicU64;

#[repr(transparent)]
#[derive(Default)]
pub struct AtomicFloat {
    inner: InnerAtomicType,
}

impl AtomicFloat {
    #[inline]
    const fn _to_inner(val: FloatType) -> InnerType {
        unsafe { transmute(val) }
    }

    #[inline]
    const fn _from_inner(val: InnerType) -> FloatType {
        unsafe { transmute(val) }
    }

    #[inline]
    pub const fn new(val: FloatType) -> Self {
        Self {
            inner: InnerAtomicType::new(Self::_to_inner(val)),
        }
    }

    #[inline]
    pub fn into_inner(self) -> FloatType {
        Self::_from_inner(self.inner.into_inner())
    }

    #[inline]
    pub fn load(&self, order: Ordering) -> FloatType {
        Self::_from_inner(self.inner.load(order))
    }

    #[inline]
    pub fn store(&self, val: FloatType, order: Ordering) {
        self.inner.store(Self::_to_inner(val), order)
    }

    #[inline]
    pub fn swap(&self, val: FloatType, order: Ordering) -> FloatType {
        Self::_from_inner(self.inner.swap(Self::_to_inner(val), order))
    }

    #[inline]
    pub fn compare_exchange(
        &self,
        current: FloatType,
        new: FloatType,
        success: Ordering,
        failure: Ordering,
    ) -> Result<FloatType, FloatType> {
        match self.inner.compare_exchange(
            Self::_to_inner(current),
            Self::_to_inner(new),
            success,
            failure,
        ) {
            Ok(v) => Ok(Self::_from_inner(v)),
            Err(v) => Err(Self::_from_inner(v)),
        }
    }

    #[inline]
    pub fn compare_exchange_weak(
        &self,
        current: FloatType,
        new: FloatType,
        success: Ordering,
        failure: Ordering,
    ) -> Result<FloatType, FloatType> {
        match self.inner.compare_exchange_weak(
            Self::_to_inner(current),
            Self::_to_inner(new),
            success,
            failure,
        ) {
            Ok(v) => Ok(Self::_from_inner(v)),
            Err(v) => Err(Self::_from_inner(v)),
        }
    }

    #[inline]
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<FloatType, FloatType>
    where
        F: FnMut(FloatType) -> Option<FloatType>,
    {
        match self.inner.fetch_update(set_order, fetch_order, |v| {
            f(Self::_from_inner(v)).map(|v| Self::_to_inner(v))
        }) {
            Ok(v) => Ok(Self::_from_inner(v)),
            Err(v) => Err(Self::_from_inner(v)),
        }
    }

    #[inline]
    fn _fetch_opr<F>(&self, order: Ordering, mut f: F) -> FloatType
    where
        F: FnMut(FloatType) -> FloatType,
    {
        let fetch_order = match order {
            Ordering::Acquire => Ordering::Release,
            _ => order,
        };
        match self.fetch_update(order, fetch_order, |v| Some(f(v))) {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    #[inline]
    pub fn fetch_add(&self, val: FloatType, order: Ordering) -> FloatType {
        self._fetch_opr(order, |v| v + val)
    }

    #[inline]
    pub fn fetch_sub(&self, val: FloatType, order: Ordering) -> FloatType {
        self._fetch_opr(order, |v| v - val)
    }
}

impl From<FloatType> for AtomicFloat {
    fn from(val: FloatType) -> Self {
        Self::new(val)
    }
}
