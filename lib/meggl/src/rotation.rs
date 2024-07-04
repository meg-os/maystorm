use core::mem::transmute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rotation {
    /// 0 degree
    Default = 0,
    /// 90 degree
    ClockWise = 1,
    /// 180 degree
    UpsideDown = 2,
    /// 270 degree
    CounterClockWise = 3,
}

impl Rotation {
    #[inline]
    pub const fn succ(self) -> Self {
        match self {
            Self::Default => Self::ClockWise,
            Self::ClockWise => Self::UpsideDown,
            Self::UpsideDown => Self::CounterClockWise,
            Self::CounterClockWise => Self::Default,
        }
    }
}

impl Default for Rotation {
    #[inline]
    fn default() -> Self {
        Self::Default
    }
}

impl From<usize> for Rotation {
    #[inline]
    fn from(value: usize) -> Self {
        unsafe { transmute(value as u8) }
    }
}

impl From<Rotation> for usize {
    #[inline]
    fn from(value: Rotation) -> Self {
        value as usize
    }
}
