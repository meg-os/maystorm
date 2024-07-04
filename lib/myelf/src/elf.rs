//! Executable and Linking Format

use core::ops::BitOr;

pub const EI_NIDENT: usize = 16;
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const EI_VERSION: usize = 6;
pub const EI_OSABI: usize = 7;
pub const EI_ABIVERSION: usize = 8;
pub const EI_PAD: usize = 9;

pub const ELFMAG: [u8; 4] = *b"\x7FELF";

// e_ident[EI_CLASS],
pub const ELFCLASSNONE: u8 = 0;
pub const ELFCLASS32: u8 = 1;
pub const ELFCLASS64: u8 = 2;

// e_ident[EI_DATA]
pub const ELFDATANONE: u8 = 0;
pub const ELFDATA2LSB: u8 = 1;
pub const ELFDATA2MSB: u8 = 2;

// e_ident[EI_VERSION]
pub const EV_NONE: u8 = 0;
pub const EV_CURRENT: u8 = 1;

pub type ElfHalf = u16;
pub type ElfWord = u32;
pub type ElfXWord = u64;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ElfType(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Machine(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentType(pub u32);

//
// These constants define the various ELF target machines
//
pub const EM_NONE: Machine = Machine(0);
/// AT&T WE 32100
pub const EM_M32: Machine = Machine(1);
/// SPARC
pub const EM_SPARC: Machine = Machine(2);
/// Intel 80386
pub const EM_386: Machine = Machine(3);
/// Motorola 68000
pub const EM_68K: Machine = Machine(4);
/// Motorola 88000
pub const EM_88K: Machine = Machine(5);
/// Intel 80860
pub const EM_860: Machine = Machine(7);
/// MIPS R3000 (officially, big-endian only)
pub const EM_MIPS: Machine = Machine(8);
/// HPPA
pub const EM_PARISC: Machine = Machine(15);
/// Sun's "v8plus"
pub const EM_SPARC32PLUS: Machine = Machine(18);
/// PowerPC
pub const EM_PPC: Machine = Machine(20);
/// PowerPC64
pub const EM_PPC64: Machine = Machine(21);
/// Cell BE SPU
pub const EM_SPU: Machine = Machine(23);
/// ARM 32 bit
pub const EM_ARM: Machine = Machine(40);
/// SuperH
pub const EM_SH: Machine = Machine(42);
/// SPARC v9 64-bit
pub const EM_SPARCV9: Machine = Machine(43);
/// Renesas H8/300
pub const EM_H8_300: Machine = Machine(46);
/// HP/Intel IA-64
pub const EM_IA_64: Machine = Machine(50);
/// AMD x86-64
pub const EM_X86_64: Machine = Machine(62);
/// IBM S/390
pub const EM_S390: Machine = Machine(22);
/// Axis Communications 32-bit embedded processor
pub const EM_CRIS: Machine = Machine(76);
/// Renesas M32R
pub const EM_M32R: Machine = Machine(88);
/// Panasonic/MEI MN10300, AM33
pub const EM_MN10300: Machine = Machine(89);
/// OpenRISC 32-bit embedded processor
pub const EM_OPENRISC: Machine = Machine(92);
/// ARCompact processor
pub const EM_ARCOMPACT: Machine = Machine(93);
/// Tensilica Xtensa Architecture
pub const EM_XTENSA: Machine = Machine(94);
/// ADI Blackfin Processor
pub const EM_BLACKFIN: Machine = Machine(106);
/// UniCore-32
pub const EM_UNICORE: Machine = Machine(110);
/// Altera Nios II soft-core processor
pub const EM_ALTERA_NIOS2: Machine = Machine(113);
/// TI C6X DSPs
pub const EM_TI_C6000: Machine = Machine(140);
/// QUALCOMM Hexagon
pub const EM_HEXAGON: Machine = Machine(164);
/// Andes Technology compact code size embedded RISC processor family
pub const EM_NDS32: Machine = Machine(167);
/// ARM 64 bit
pub const EM_AARCH64: Machine = Machine(183);
/// Tilera TILEPro
pub const EM_TILEPRO: Machine = Machine(188);
/// Xilinx MicroBlaze
pub const EM_MICROBLAZE: Machine = Machine(189);
/// Tilera TILE-Gx
pub const EM_TILEGX: Machine = Machine(191);
/// ARCv2 Cores
pub const EM_ARCV2: Machine = Machine(195);
/// RISC-V
pub const EM_RISCV: Machine = Machine(243);
/// Linux BPF - in-kernel virtual machine
pub const EM_BPF: Machine = Machine(247);
/// C-SKY
pub const EM_CSKY: Machine = Machine(252);
/// LoongArch
pub const EM_LOONGARCH: Machine = Machine(258);
/// Fujitsu FR-V
pub const EM_FRV: Machine = Machine(0x5441);

/// This is an interim value that we will use until the committee comes up with a final number.
pub const EM_ALPHA: Machine = Machine(0x9026);

/// Bogus old m32r magic number, used by old tools.
pub const EM_CYGNUS_M32R: Machine = Machine(0x9041);
/// This is the old interim value for S/390 architecture
pub const EM_S390_OLD: Machine = Machine(0xA390);
/// Also Panasonic/MEI MN10300, AM33
pub const EM_CYGNUS_MN10300: Machine = Machine(0xbeef);

//
// These constants are for the segment types stored in the image headers
//
pub const PT_NULL: SegmentType = SegmentType(0);
pub const PT_LOAD: SegmentType = SegmentType(1);
pub const PT_DYNAMIC: SegmentType = SegmentType(2);
pub const PT_INTERP: SegmentType = SegmentType(3);
pub const PT_NOTE: SegmentType = SegmentType(4);
pub const PT_SHLIB: SegmentType = SegmentType(5);
pub const PT_PHDR: SegmentType = SegmentType(6);
pub const PT_TLS: SegmentType = SegmentType(7);

pub const PT_LOOS: SegmentType = SegmentType(0x6000_0000);
pub const PT_HIOS: SegmentType = SegmentType(0x6FFF_FFFF);

pub const PT_SUNW_UNWIND: SegmentType = SegmentType(0x6464_E550);
pub const PT_GNU_EH_FRAME: SegmentType = SegmentType(0x6474_E550);
pub const PT_GNU_STACK: SegmentType = SegmentType(0x6474_E551);
pub const PT_GNU_RELRO: SegmentType = SegmentType(0x6474_E552);
pub const PT_GNU_PROPERTY: SegmentType = SegmentType(0x6474_E553);

pub const PT_LOPROC: SegmentType = SegmentType(0x7000_0000);
pub const PT_HIPROC: SegmentType = SegmentType(0x7FFF_FFFF);

pub const PT_AARCH64_MEMTAG_MTE: SegmentType = SegmentType(0x7000_0002);

//
// These constants define the different elf file types
//
pub const ET_NONE: ElfType = ElfType(0);
pub const ET_REL: ElfType = ElfType(1);
pub const ET_EXEC: ElfType = ElfType(2);
pub const ET_DYN: ElfType = ElfType(3);
pub const ET_CORE: ElfType = ElfType(4);
pub const ET_LOPROC: ElfType = ElfType(0xFF00);
pub const ET_HIPROC: ElfType = ElfType(0xFFFF);

pub const PF_X: SegmentFlags = SegmentFlags(1);
pub const PF_W: SegmentFlags = SegmentFlags(2);
pub const PF_R: SegmentFlags = SegmentFlags(4);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentFlags(pub u32);

impl SegmentFlags {
    pub const READ: Self = Self(4);
    pub const WRITE: Self = Self(2);
    pub const EXEC: Self = Self(1);
    pub const RWX: Self = Self(Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits());

    #[inline]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr<Self> for SegmentFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub mod elf32 {
    use super::*;

    pub type ElfAddr = u32;
    pub type ElfOff = u32;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct Header {
        pub e_ident: [u8; EI_NIDENT],
        pub e_type: ElfType,
        pub e_machine: Machine,
        pub e_version: ElfWord,
        pub e_entry: ElfAddr,
        pub e_phoff: ElfOff,
        pub e_shoff: ElfOff,
        pub e_flags: ElfWord,
        pub e_ehsize: ElfHalf,
        pub e_phentsize: ElfHalf,
        pub e_phnum: ElfHalf,
        pub e_shentsize: ElfHalf,
        pub e_shnum: ElfHalf,
        pub e_shstrndx: ElfHalf,
    }

    impl Header {
        #[inline]
        pub fn is_valid(&self, elf_type: ElfType, machine: Machine) -> bool {
            (self.e_ident[..4] == ELFMAG)
                && (self.e_ident[EI_CLASS] == ELFCLASS32)
                && (self.e_ident[EI_DATA] == ELFDATA2LSB)
                && (self.e_ident[EI_VERSION] == EV_CURRENT)
                && self.e_type == elf_type
                && self.e_machine == machine
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct ProgramHeader {
        pub p_type: SegmentType,
        pub p_offset: ElfOff,
        pub p_vaddr: ElfAddr,
        pub p_paddr: ElfAddr,
        pub p_filesz: ElfWord,
        pub p_memsz: ElfWord,
        pub p_flags: SegmentFlags,
        pub p_align: ElfWord,
    }
}

pub mod elf64 {
    use super::*;

    pub type ElfAddr = u64;
    pub type ElfOff = u64;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct Header {
        pub e_ident: [u8; EI_NIDENT],
        pub e_type: ElfType,
        pub e_machine: Machine,
        pub e_version: ElfWord,
        pub e_entry: ElfAddr,
        pub e_phoff: ElfOff,
        pub e_shoff: ElfOff,
        pub e_flags: ElfWord,
        pub e_ehsize: ElfHalf,
        pub e_phentsize: ElfHalf,
        pub e_phnum: ElfHalf,
        pub e_shentsize: ElfHalf,
        pub e_shnum: ElfHalf,
        pub e_shstrndx: ElfHalf,
    }

    impl Header {
        #[inline]
        pub fn is_valid(&self, elf_type: ElfType, machine: Machine) -> bool {
            (self.e_ident[..4] == ELFMAG)
                && (self.e_ident[EI_CLASS] == ELFCLASS64)
                && (self.e_ident[EI_DATA] == ELFDATA2LSB)
                && (self.e_ident[EI_VERSION] == EV_CURRENT)
                && self.e_type == elf_type
                && self.e_machine == machine
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct ProgramHeader {
        pub p_type: SegmentType,
        pub p_flags: SegmentFlags,
        pub p_offset: ElfOff,
        pub p_vaddr: ElfAddr,
        pub p_paddr: ElfAddr,
        pub p_filesz: ElfXWord,
        pub p_memsz: ElfXWord,
        pub p_align: ElfXWord,
    }
}
