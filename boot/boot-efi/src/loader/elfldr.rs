//! Minimal ELF File Loader

use super::*;
use crate::page::*;
use core::intrinsics::transmute;
use core::marker::PhantomData;
use core::ptr::copy_nonoverlapping;
use myelf::*;

pub struct ElfLoader<'a> {
    elf_hdr: &'a elf64::Header,
    blob: &'a [u8],
    image_base: VirtualAddress,
    image_size: usize,
}

impl<'a> ElfLoader<'a> {
    #[inline]
    pub fn parse(blob: &'a [u8]) -> Option<Self> {
        let mut result = Self {
            elf_hdr: unsafe { transmute(blob.as_ptr()) },
            blob,
            image_base: VirtualAddress(0),
            image_size: 0,
        };
        result._recognize().then_some(result)
    }

    fn _recognize(&mut self) -> bool {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        let preferred_machine = EM_X86_64;
        #[cfg(target_arch = "aarch64")]
        let preferred_machine = EM_AARCH64;

        let elf_hdr = self.elf_hdr;
        if elf_hdr.is_valid(ET_EXEC, preferred_machine) {
            let page_mask = UEFI_PAGE_SIZE - 1;
            let image_base = VirtualAddress(
                self.program_header()
                    .filter(|v| v.p_type == PT_LOAD)
                    .fold(u64::MAX, |a, v| u64::min(a, v.p_vaddr))
                    & !page_mask,
            );
            let image_size = ((self
                .program_header()
                .filter(|v| v.p_type == PT_LOAD)
                .fold(0, |a, v| u64::max(a, v.p_vaddr + v.p_memsz))
                - image_base.as_u64()
                + page_mask)
                & !page_mask) as usize;

            self.image_base = image_base;
            self.image_size = image_size;

            true
        } else {
            false
        }
    }

    #[inline]
    fn program_header(&'a self) -> impl Iterator<Item = &'a elf64::ProgramHeader> {
        unsafe {
            ElfProgramHeaderIter::new(
                self.blob.as_ptr().add(self.elf_hdr.e_phoff as usize),
                self.elf_hdr.e_phentsize as usize,
                self.elf_hdr.e_phnum as usize,
            )
        }
    }
}

impl ImageLoader for ElfLoader<'_> {
    #[inline]
    fn image_bounds(&self) -> (VirtualAddress, usize) {
        (self.image_base, self.image_size)
    }

    unsafe fn locate(&self, _base: VirtualAddress) -> VirtualAddress {
        unsafe {
            let elf_hdr = self.elf_hdr;
            let image_base = self.image_base;
            let image_size = self.image_size;

            // Step 1 - allocate memory
            let page_mask = UEFI_PAGE_SIZE - 1;
            let vmem = PageManager::valloc(image_base, image_size);
            vmem.write_bytes(0, image_size);

            // Step 2 - locate segments
            for item in self.program_header() {
                if item.p_type == PT_LOAD {
                    let rva = (item.p_vaddr - image_base.as_u64()) as usize;
                    let p = vmem.add(rva);
                    let q: *const u8 = self.blob.as_ptr().add(item.p_offset as usize);
                    let z = item.p_filesz as usize;
                    copy_nonoverlapping(q, p, z);
                }
            }

            // Step 3 - relocation
            // nothing to do

            // Step 4 - attributes
            for item in self.program_header() {
                if item.p_type == PT_LOAD {
                    let va = VirtualAddress(item.p_vaddr & !page_mask);
                    let size = ((item.p_memsz + item.p_vaddr - va.as_u64() + page_mask)
                        & !page_mask) as usize;
                    PageManager::vprotect(va, size, item.p_flags);
                }
            }

            VirtualAddress(elf_hdr.e_entry)
        }
    }
}

#[derive(Clone)]
struct ElfProgramHeaderIter<'a> {
    base: *const u8,
    entry_size: usize,
    n_entries: usize,
    index: usize,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> ElfProgramHeaderIter<'a> {
    #[inline]
    const unsafe fn new(base: *const u8, entry_size: usize, n_entries: usize) -> Self {
        Self {
            base,
            entry_size,
            n_entries,
            index: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a> Iterator for ElfProgramHeaderIter<'a> {
    type Item = &'a elf64::ProgramHeader;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.n_entries {
            let item = unsafe {
                let p = self.base.add(self.index * self.entry_size) as *const elf64::ProgramHeader;
                &*p
            };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
