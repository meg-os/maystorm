use core::{
    arch::asm,
    fmt,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign},
};

use crate::prot::IOPL;

#[cfg(target_arch = "x86")]
pub type Eflags = Flags;
#[cfg(target_arch = "x86_64")]
pub type Rflags = Flags;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Flags(usize);

impl Flags {
    /// Carry flag
    pub const CF: Self = Self(0x0000_0001);
    // Reserved Always 1
    pub const _VF: Self = Self(0x0000_0002);
    /// Parity flag
    pub const PF: Self = Self(0x0000_0004);
    /// Adjust flag
    pub const AF: Self = Self(0x0000_0010);
    /// Zero flag
    pub const ZF: Self = Self(0x0000_0040);
    /// Sign flag
    pub const SF: Self = Self(0x0000_0080);
    /// Trap flag
    pub const TF: Self = Self(0x0000_0100);
    /// Interrupt enable flag
    pub const IF: Self = Self(0x0000_0200);
    /// Direction flag
    pub const DF: Self = Self(0x0000_0400);
    /// Overflow flag
    pub const OF: Self = Self(0x0000_0800);
    /// I/O privilege level
    pub const IOPL3: Self = Self(0x0000_3000);
    /// Nested task flag
    pub const NT: Self = Self(0x0000_4000);
    /// Mode flag (NEC V30)
    pub const MD: Self = Self(0x0000_8000);
    /// Resume flag
    pub const RF: Self = Self(0x0001_0000);
    /// Virtual 8086 mode flag
    pub const VM: Self = Self(0x0002_0000);
    /// Alignment check
    pub const AC: Self = Self(0x0004_0000);
    /// Virtual interrupt flag
    pub const VIF: Self = Self(0x0008_0000);
    /// Virtual interrupt pending
    pub const VIP: Self = Self(0x0010_0000);
    /// Able to use CPUID instruction
    pub const ID: Self = Self(0x0020_0000);

    #[inline]
    pub const fn from_bits_retain(bits: usize) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn bits(&self) -> usize {
        self.0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    pub fn set(&mut self, bit: Self, value: bool) {
        if value {
            self.insert(bit);
        } else {
            self.remove(bit);
        }
    }

    #[cfg(target_arch = "x86")]
    #[inline]
    pub unsafe fn read() -> Self {
        let flags: usize;
        asm!("
            pushfd
            pop {}
            ", out(reg)flags);
        Self::from_bits_retain(flags)
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub unsafe fn read() -> Self {
        let flags: usize;
        asm!("
            pushfq
            pop {}
            ", out(reg)flags);
        Self::from_bits_retain(flags)
    }

    #[inline]
    pub fn iopl(&self) -> IOPL {
        IOPL::from_flags(self.bits())
    }

    #[inline]
    pub fn set_iopl(&mut self, iopl: IOPL) {
        *self = Self::from_bits_retain((self.bits() & !Self::IOPL3.bits()) | (iopl.into_flags()))
    }
}

impl BitOr<Self> for Flags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<Self> for Flags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

impl BitAnd<Self> for Flags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Flags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor<Self> for Flags {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign<Self> for Flags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl fmt::Debug for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Flags").field(&self.0).finish()
    }
}
