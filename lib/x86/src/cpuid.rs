#[cfg(target_arch = "x86")]
pub use core::arch::x86::{__cpuid as cpuid, __cpuid_count as cpuid_count};
#[cfg(target_arch = "x86_64")]
pub use core::arch::x86_64::{__cpuid as cpuid, __cpuid_count as cpuid_count};

pub fn is_intel_processor() -> bool {
    let cpuid = unsafe { cpuid(0) };
    // GenuineIntel
    cpuid.ebx == 0x756e6547 && cpuid.edx == 0x49656e69 && cpuid.ecx == 0x6c65746e
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    F01D(F01D),
    F01C(F01C),
    F07B(F070B),
    F07C(F070C),
    F07D(F070D),
    F81D(F81D),
    F81C(F81C),
}

macro_rules! short_feature_impl {
    { $vis:vis enum $node:ident ( $sub_class:ident ) { $( $mnemonic:ident, )* } $($next:tt)*} => {
        impl Feature {
            $(
                $vis const $mnemonic: Self = Self::$node($sub_class::$mnemonic);
            )*
        }
        short_feature_impl! { $($next)* }
    };
    {} => {};
}

short_feature_impl! {
    pub enum F01D(F01D) {
        FPU,
        VME,
        DE,
        PSE,
        TSC,
        MSR,
        PAE,
        MCE,
        CX8,
        APIC,
        SEP,
        MTRR,
        MGE,
        MCA,
        CMOV,
        PAT,
        PSE36,
        PSN,
        CLFSH,
        DS,
        ACPI,
        MMX,
        FXSR,
        SSE,
        SSE2,
        SS,
        HTT,
        TM,
        IA64,
        PBE,
    }

    pub enum F01C(F01C) {
        SSE3,
        PCLMULQDQ,
        DTES64,
        MONITOR,
        DS_CPL,
        VMX,
        SMX,
        EST,
        TM2,
        SSSE3,
        CNXT_ID,
        SDBG,
        FMA,
        CX16,
        XTPR,
        PDCM,
        PCID,
        DCA,
        SSE4_1,
        SSE4_2,
        X2APIC,
        MOVBE,
        POPCNT,
        TSC_DEADLINE,
        AES,
        XSAVE,
        OSXSAVE,
        AVX,
        F16C,
        RDRND,
        HYPERVISOR,
    }

    pub enum F07B(F070B) {
        FSGSBASE,
        IA32_TSC_ADJUST,
        SGX,
        BMI1,
        HLE,
        AVX2,
        FDP_EXCPTN_ONLY,
        SMEP,
        BMI2,
        ERMS,
        INVPCID,
        RTM,
        PQM,
        MPX,
        PQE,
        AVX512_F,
        AVX512_DQ,
        RDSEED,
        ADX,
        SMAP,
        AVX512_IFMA,
        PCOMMIT,
        CLFLUSHIPT,
        CLWB,
        INTEL_PT,
        AVX512_PF,
        AVX512_ER,
        AVX512_CD,
        SHA,
        AVX512_BW,
        AVX512_VL,
    }

    pub enum F07C(F070C) {
        PREFETCHWT1,
        AVX512_VBMI,
        UMIP,
        PKU,
        OSPKE,
        WAITPKG,
        AVX512_VBMI2,
        CET_SS,
        GFNI,
        VAES,
        VPCLMULQDQ,
        AVX512_VNNI,
        AVX512_BITALG,
        AVX512_VPOPCNTDQ,
        LA57,
        RDPID,
        CLDEMOTE,
        MOVDIRI,
        MOVDIR64B,
        ENQCMD,
        SGX_LC,
        PKS,
    }

    pub enum F07D(F070D) {
        AVX512_4VNNIW,
        AVX512_4FMAPS,
        FSRM,
        UINTR,
        AVX512_VP2INTERSECT,
        SRBDS_CTRL,
        MD_CLEAR,
        TSX_FORCE_ABORT,
        SERIALIZE,
        HYBRID,
        TSXLDTRK,
        PCONFIG,
        LBR,
        CET_IBT,
        AMX_BF16,
        AVX512_FP16,
        AMX_TILE,
        AMX_INT8,
        IBRS_IBPB,
        STIBP,
        L1D_FLUSH,
        IA32_ARCH_CAPABILITIES,
        IA32_CORE_CAPABILITIES,
        SSBD,
    }

    pub enum F81D(F81D) {
        SYSCALL,
        NX,
        PDPE1GB,
        RDTSCP,
        LM,
    }

    pub enum F81C(F81C) {
        LAHF_LM,
        CMP_LEGACY,
        SVM,
        EXTAPIC,
        CR8_LEGACY,
        ABM,
        SSE4A,
        MISALIGNSSE,
        _3DNOWPREFETCH,
        OSVW,
        IBS,
        XOP,
        SKINIT,
        WDT,
        LWP,
        FMA4,
        TCE,
        NODEID_MSR,
        TBM,
        TOPOEXT,
        PERFCTR_CORE,
        PERFCTR_NB,
        DBX,
        PERFTSC,
        PCX_L2I,
    }
}

impl Feature {
    pub fn exists(&self) -> bool {
        unsafe {
            match *self {
                Self::F01D(bit) => (cpuid(0x0000_0001).edx & (1 << bit as usize)) != 0,
                Self::F01C(bit) => (cpuid(0x0000_0001).ecx & (1 << bit as usize)) != 0,
                Self::F07B(bit) => (cpuid_count(0x0000_0007, 0).ebx & (1 << bit as usize)) != 0,
                Self::F07C(bit) => (cpuid_count(0x0000_0007, 0).ecx & (1 << bit as usize)) != 0,
                Self::F07D(bit) => (cpuid_count(0x0000_0007, 0).edx & (1 << bit as usize)) != 0,
                Self::F81D(bit) => (cpuid(0x8000_0001).edx & (1 << bit as usize)) != 0,
                Self::F81C(bit) => (cpuid(0x8000_0001).ecx & (1 << bit as usize)) != 0,
            }
        }
    }
}

/// CPUID Feature Function 0000_0001, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F01D {
    FPU = 0,
    VME = 1,
    DE = 2,
    PSE = 3,
    TSC = 4,
    MSR = 5,
    PAE = 6,
    MCE = 7,
    CX8 = 8,
    APIC = 9,
    SEP = 11,
    MTRR = 12,
    MGE = 13,
    MCA = 14,
    CMOV = 15,
    PAT = 16,
    PSE36 = 17,
    PSN = 18,
    CLFSH = 19,
    DS = 21,
    ACPI = 22,
    MMX = 23,
    FXSR = 24,
    SSE = 25,
    SSE2 = 26,
    SS = 27,
    HTT = 28,
    TM = 29,
    IA64 = 30,
    PBE = 31,
}

/// CPUID Feature Function 0000_0001, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F01C {
    SSE3 = 0,
    PCLMULQDQ = 1,
    DTES64 = 2,
    MONITOR = 3,
    DS_CPL = 4,
    VMX = 5,
    SMX = 6,
    EST = 7,
    TM2 = 8,
    SSSE3 = 9,
    CNXT_ID = 10,
    SDBG = 11,
    FMA = 12,
    CX16 = 13,
    XTPR = 14,
    PDCM = 15,
    PCID = 17,
    DCA = 18,
    SSE4_1 = 19,
    SSE4_2 = 20,
    X2APIC = 21,
    MOVBE = 22,
    POPCNT = 23,
    TSC_DEADLINE = 24,
    AES = 25,
    XSAVE = 26,
    OSXSAVE = 27,
    AVX = 28,
    F16C = 29,
    RDRND = 30,
    HYPERVISOR = 31,
}

/// CPUID Feature Function 0000_0007, 0, EBX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F070B {
    FSGSBASE = 0,
    IA32_TSC_ADJUST = 1,
    SGX = 2,
    BMI1 = 3,
    HLE = 4,
    AVX2 = 5,
    FDP_EXCPTN_ONLY = 6,
    SMEP = 7,
    BMI2 = 8,
    ERMS = 9,
    INVPCID = 10,
    RTM = 11,
    PQM = 12,
    // FPU CS and FPU DS deprecated = 13,
    MPX = 14,
    PQE = 15,
    AVX512_F = 16,
    AVX512_DQ = 17,
    RDSEED = 18,
    ADX = 19,
    SMAP = 20,
    AVX512_IFMA = 21,
    PCOMMIT = 22,
    CLFLUSHIPT = 23,
    CLWB = 24,
    INTEL_PT = 25,
    AVX512_PF = 26,
    AVX512_ER = 27,
    AVX512_CD = 28,
    SHA = 29,
    AVX512_BW = 30,
    AVX512_VL = 31,
}

/// CPUID Feature Function 0000_0007, 0, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F070C {
    PREFETCHWT1 = 0,
    AVX512_VBMI = 1,
    UMIP = 2,
    PKU = 3,
    OSPKE = 4,
    WAITPKG = 5,
    AVX512_VBMI2 = 6,
    CET_SS = 7,
    GFNI = 8,
    VAES = 9,
    VPCLMULQDQ = 10,
    AVX512_VNNI = 11,
    AVX512_BITALG = 12,
    AVX512_VPOPCNTDQ = 14,
    LA57 = 16,
    RDPID = 22,
    CLDEMOTE = 25,
    MOVDIRI = 27,
    MOVDIR64B = 28,
    ENQCMD = 29,
    SGX_LC = 30,
    PKS = 31,
}

/// CPUID Feature Function 0000_0007, 0, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F070D {
    AVX512_4VNNIW = 2,
    AVX512_4FMAPS = 3,
    FSRM = 4,
    UINTR = 5,
    AVX512_VP2INTERSECT = 8,
    SRBDS_CTRL = 9,
    MD_CLEAR = 10,
    TSX_FORCE_ABORT = 13,
    SERIALIZE = 14,
    HYBRID = 15,
    TSXLDTRK = 16,
    PCONFIG = 18,
    LBR = 19,
    CET_IBT = 20,
    AMX_BF16 = 22,
    AVX512_FP16 = 23,
    AMX_TILE = 24,
    AMX_INT8 = 25,
    IBRS_IBPB = 26,
    STIBP = 27,
    L1D_FLUSH = 28,
    IA32_ARCH_CAPABILITIES = 29,
    IA32_CORE_CAPABILITIES = 30,
    SSBD = 31,
}

/// CPUID Feature Function 0000_0007, 1, EAX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F071A {
    AVX_VNNI = 4,
    AVX512_BF16 = 5,
    FAST_ZERO_LENGTH_REP_MOVSB = 10,
    FAST_SHORT_REP_STOSB = 11,
    FAST_SHORT_REP_CMPSB = 12,
    HRESET = 22,
    INVD_DISABLE_POST_BIOS_DONE = 30,
}

/// CPUID Feature Function 0000_0007, 1, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F071D {
    CET_SSS = 17,
}

/// CPUID Feature Function 0000_0007, 2, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F072D {
    PSFD = 0,
    IPRED_CTRL = 1,
    RRSBA_CTRL = 2,
    DDPD_U = 3,
    BHI_CTRL = 4,
    MCDT_NO = 5,
}

/// CPUID Feature Function 8000_0001, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F81D {
    SYSCALL = 11,
    NX = 20,
    PDPE1GB = 26,
    RDTSCP = 27,
    LM = 29,
}

/// CPUID Feature Function 8000_0001, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum F81C {
    LAHF_LM = 0,
    CMP_LEGACY = 1,
    SVM = 2,
    EXTAPIC = 3,
    CR8_LEGACY = 4,
    ABM = 5,
    SSE4A = 6,
    MISALIGNSSE = 7,
    _3DNOWPREFETCH = 8,
    OSVW = 9,
    IBS = 10,
    XOP = 11,
    SKINIT = 12,
    WDT = 13,
    LWP = 15,
    FMA4 = 16,
    TCE = 17,
    NODEID_MSR = 19,
    TBM = 21,
    TOPOEXT = 22,
    PERFCTR_CORE = 23,
    PERFCTR_NB = 24,
    DBX = 26,
    PERFTSC = 27,
    PCX_L2I = 28,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum NativeModelCoreType {
    /// `0x40` P-core (Core)
    Performance,
    /// `0x20` E-core (Atom)
    Efficient,
}

impl NativeModelCoreType {
    const CORE_TYPE_ATOM: u8 = 0x20;

    const CORE_TYPE_CORE: u8 = 0x40;

    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            Self::CORE_TYPE_ATOM => Some(Self::Efficient),
            Self::CORE_TYPE_CORE => Some(Self::Performance),
            _ => None,
        }
    }
}
