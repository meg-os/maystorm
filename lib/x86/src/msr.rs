use core::{arch::asm, mem::transmute};

#[allow(unused_imports)]
use alloc::vec::Vec;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MSR(u32);

impl MSR {
    pub const IA32_TSC: Self = Self(0x0000_0010);
    pub const IA32_PLATFORM_ID: Self = Self(0x0000_0017);
    pub const IA32_APIC_BASE: Self = Self(0x0000_001B);
    pub const IA32_FEATURE_CONTROL: Self = Self(0x0000_003A);
    pub const IA32_TSC_ADJUST: Self = Self(0x0000_0003B);
    pub const IA32_XAPIC_DISABLE_STATUS: Self = Self(0x0000_00BD);
    pub const IA32_MTRRCAP: Self = Self(0x0000_00FE);
    pub const IA32_MISC_ENABLE: Self = Self(0x0000_01A0);
    pub const IA32_TSC_DEADLINE: Self = Self(0x0000_06E0);
    pub const IA32_SYSENTER_CS: Self = Self(0x0000_0174);
    pub const IA32_SYSENTER_ESP: Self = Self(0x0000_0175);
    pub const IA32_SYSENTER_EIP: Self = Self(0x0000_0176);
    pub const IA32_PAT: Self = Self(0x0000_0277);
    pub const IA32_MTRR_DEF_TYPE: Self = Self(0x0000_02FF);
    pub const IA32_X2APIC_APICID: Self = Self(0x0000_0802);
    pub const IA32_X2APIC_VERSION: Self = Self(0x0000_0803);
    pub const IA32_X2APIC_TPR: Self = Self(0x0000_0808);
    pub const IA32_X2APIC_PPR: Self = Self(0x0000_080A);
    pub const IA32_X2APIC_EOI: Self = Self(0x0000_080B);
    pub const IA32_X2APIC_LDR: Self = Self(0x0000_080D);
    pub const IA32_X2APIC_SIVR: Self = Self(0x0000_080F);
    pub const IA32_X2APIC_ISR0: Self = Self(0x0000_0810);
    pub const IA32_X2APIC_ISR1: Self = Self(0x0000_0811);
    pub const IA32_X2APIC_ISR2: Self = Self(0x0000_0812);
    pub const IA32_X2APIC_ISR3: Self = Self(0x0000_0813);
    pub const IA32_X2APIC_ISR4: Self = Self(0x0000_0814);
    pub const IA32_X2APIC_ISR5: Self = Self(0x0000_0815);
    pub const IA32_X2APIC_ISR6: Self = Self(0x0000_0816);
    pub const IA32_X2APIC_ISR7: Self = Self(0x0000_0817);
    pub const IA32_X2APIC_TMR0: Self = Self(0x0000_0818);
    pub const IA32_X2APIC_TMR1: Self = Self(0x0000_0819);
    pub const IA32_X2APIC_TMR2: Self = Self(0x0000_081A);
    pub const IA32_X2APIC_TMR3: Self = Self(0x0000_081B);
    pub const IA32_X2APIC_TMR4: Self = Self(0x0000_081C);
    pub const IA32_X2APIC_TMR5: Self = Self(0x0000_081D);
    pub const IA32_X2APIC_TMR6: Self = Self(0x0000_081E);
    pub const IA32_X2APIC_TMR7: Self = Self(0x0000_081F);
    pub const IA32_X2APIC_IRR0: Self = Self(0x0000_0820);
    pub const IA32_X2APIC_IRR1: Self = Self(0x0000_0821);
    pub const IA32_X2APIC_IRR2: Self = Self(0x0000_0822);
    pub const IA32_X2APIC_IRR3: Self = Self(0x0000_0823);
    pub const IA32_X2APIC_IRR4: Self = Self(0x0000_0824);
    pub const IA32_X2APIC_IRR5: Self = Self(0x0000_0825);
    pub const IA32_X2APIC_IRR6: Self = Self(0x0000_0826);
    pub const IA32_X2APIC_IRR7: Self = Self(0x0000_0827);
    pub const IA32_X2APIC_ESR: Self = Self(0x0000_0828);
    pub const IA32_X2APIC_LVT_CMCI: Self = Self(0x0000_082F);
    pub const IA32_X2APIC_ICR: Self = Self(0x0000_0830);
    pub const IA32_X2APIC_LVT_TIMER: Self = Self(0x0000_0832);
    pub const IA32_X2APIC_LVT_THERMAL: Self = Self(0x0000_0833);
    pub const IA32_X2APIC_LVT_PMI: Self = Self(0x0000_0834);
    pub const IA32_X2APIC_LVT_LINT0: Self = Self(0x0000_0835);
    pub const IA32_X2APIC_LVT_LINT1: Self = Self(0x0000_0836);
    pub const IA32_X2APIC_LVT_ERROR: Self = Self(0x0000_0837);
    pub const IA32_X2APIC_INIT_COUNT: Self = Self(0x0000_0838);
    pub const IA32_X2APIC_CUR_COUNT: Self = Self(0x0000_0839);
    pub const IA32_X2APIC_DIV_CONF: Self = Self(0x0000_083E);
    pub const IA32_X2APIC_SELF_IPI: Self = Self(0x0000_083F);

    pub const IA32_PASID: Self = Self(0x0000_0D93);
    pub const IA32_XSS: Self = Self(0x0000_0DA0);

    pub const IA32_HW_FEEDBACK_PTR: Self = Self(0x0000_17D0);
    pub const IA32_HW_FEEDBACK_CONFIG: Self = Self(0x0000_17D1);
    pub const IA32_THREAD_FEEDBACK_CHAR: Self = Self(0x0000_17D2);
    pub const IA32_HW_FEEDBACK_THREAD_CONFIG: Self = Self(0x0000_17D4);

    pub const IA32_EFER: Self = Self(0xC000_0080);
    pub const IA32_STAR: Self = Self(0xC000_0081);
    pub const IA32_LSTAR: Self = Self(0xC000_0082);
    pub const IA32_CSTAR: Self = Self(0xC000_0083);
    pub const IA32_FMASK: Self = Self(0xC000_0084);
    pub const IA32_FS_BASE: Self = Self(0xC000_0100);
    pub const IA32_GS_BASE: Self = Self(0xC000_0101);
    pub const IA32_KERNEL_GS_BASE: Self = Self(0xC000_0102);
    pub const IA32_TSC_AUX: Self = Self(0xC000_0103);
    pub const CPU_WATCHDOG_TIMER: Self = Self(0xC001_0074);

    #[inline]
    #[allow(non_snake_case)]
    pub fn IA32_MTRRphysBase(n: &MtrrIndex) -> Self {
        Self(0x0000_0200 + n.0 as u32 * 2)
    }

    #[inline]
    #[allow(non_snake_case)]
    pub fn IA32_MTRRphysMask(n: &MtrrIndex) -> Self {
        Self(0x0000_0201 + n.0 as u32 * 2)
    }

    #[inline]
    pub unsafe fn write(&self, value: u64) {
        let value = MsrResult { qword: value };
        unsafe {
            asm!(
                "wrmsr",
                in("eax") value.pair.eax,
                in("edx") value.pair.edx,
                in("ecx") self.0,
            );
        }
    }

    #[inline]
    pub unsafe fn read(&self) -> u64 {
        let eax: u32;
        let edx: u32;
        asm!(
            "rdmsr",
            lateout("eax") eax,
            lateout("edx") edx,
            in("ecx") self.0,
        );

        MsrResult {
            pair: EaxAndEdx { eax, edx },
        }
        .qword
    }

    #[inline]
    pub unsafe fn bit_set(&self, value: u64) -> u64 {
        let mut temp = self.read();
        temp |= value;
        self.write(temp);
        temp
    }

    #[inline]
    pub unsafe fn bit_clear(&self, value: u64) -> u64 {
        let mut temp = self.read();
        temp &= !value;
        self.write(temp);
        temp
    }

    #[inline]
    pub unsafe fn set_pat(values: [PAT; 8]) {
        let data = u64::from_le_bytes(values.map(|v| v as u8));
        MSR::IA32_PAT.write(data);
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
union MsrResult {
    qword: u64,
    pair: EaxAndEdx,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct EaxAndEdx {
    eax: u32,
    edx: u32,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MtrrIndex(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mtrr {
    /// UC Uncacheable
    UC = 0,
    /// WC WriteCombining
    WC = 1,
    /// WT WriteThrough
    WT = 4,
    /// WP WriteProtect
    WP = 5,
    /// WB WriteBack
    WB = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PAT {
    /// UC Uncacheable
    UC = 0,
    /// WC WriteCombining
    WC = 1,
    /// WT WriteThrough
    WT = 4,
    /// WP WriteProtect
    WP = 5,
    /// WB WriteBack
    WB = 6,
    /// UC- Uncached
    UC_ = 7,
}

impl Mtrr {
    #[inline]
    pub fn count() -> usize {
        unsafe { MSR::IA32_MTRRCAP.read() as usize & 0xFF }
    }

    #[inline]
    pub fn indexes() -> impl Iterator<Item = MtrrIndex> {
        (0..Self::count() as u8).into_iter().map(|v| MtrrIndex(v))
    }

    #[inline]
    pub unsafe fn get(index: &MtrrIndex) -> MtrrItem {
        unsafe {
            let base = MSR::IA32_MTRRphysBase(index).read();
            let mask = MSR::IA32_MTRRphysMask(index).read();
            MtrrItem::from_raw(base, mask)
        }
    }

    #[inline]
    pub unsafe fn set(index: &MtrrIndex, item: MtrrItem) {
        unsafe {
            let (base, mask) = item.into_pair();
            MSR::IA32_MTRRphysBase(index).write(base);
            MSR::IA32_MTRRphysMask(index).write(mask);
        }
    }

    #[inline]
    pub unsafe fn items() -> impl Iterator<Item = MtrrItem> {
        Self::indexes().map(|n| unsafe { Self::get(&n) })
    }

    #[inline]
    pub unsafe fn set_items(items: &[MtrrItem]) {
        let mut items = items
            .iter()
            .filter(|v| v.is_enabled)
            .map(|v| *v)
            .collect::<Vec<_>>();
        items.sort_by_key(|v| v.base);
        items.resize(Self::count(), MtrrItem::empty());
        for (index, item) in Self::indexes().zip(items.into_iter()) {
            unsafe {
                Self::set(&index, item);
            }
        }
    }

    #[inline]
    pub const fn from_raw(value: u8) -> Self {
        unsafe { transmute(value) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtrrItem {
    pub base: u64,
    pub mask: u64,
    pub mem_type: Mtrr,
    pub is_enabled: bool,
}

impl MtrrItem {
    const ADDR_MASK: u64 = !0xFFF;

    #[inline]
    pub const fn empty() -> Self {
        Self {
            base: 0,
            mask: 0,
            mem_type: Mtrr::UC,
            is_enabled: false,
        }
    }

    #[inline]
    pub fn from_raw(base: u64, mask: u64) -> Self {
        let mem_type = Mtrr::from_raw(base as u8);
        let is_enabled = (mask & 0x800) != 0;
        Self {
            base: base & Self::ADDR_MASK,
            mask: mask & Self::ADDR_MASK,
            mem_type,
            is_enabled,
        }
    }

    #[inline]
    pub fn into_pair(self) -> (u64, u64) {
        let base = (self.base & Self::ADDR_MASK) | self.mem_type as u64;
        let mask = (self.mask & Self::ADDR_MASK) | if self.is_enabled { 0x800 } else { 0 };
        (base, mask)
    }

    #[inline]
    pub fn matches(&self, other: u64) -> bool {
        (self.base & self.mask) == (other & self.mask)
    }
}
