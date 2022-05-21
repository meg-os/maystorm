// MEG-OS Boot loader for UEFI

#![no_std]
#![no_main]
#![feature(abi_efiapi)]

use boot_efi::{invocation::*, loader::*, page::*};
use bootprot::*;
use core::{ffi::c_void, fmt::Write, mem::*};
use uefi::prelude::*;

extern crate lib_efi;
use lib_efi::*;

static KERNEL_PATH: &str = "/EFI/MEGOS/kernel.bin";
static INITRD_PATH: &str = "/EFI/MEGOS/initrd.img";

#[entry]
fn efi_main(handle: Handle, mut st: SystemTable<Boot>) -> Status {
    unsafe {
        uefi::alloc::init(st.boot_services());
    }

    let mut info = BootInfo::default();
    let bs = st.boot_services();
    info.platform = Platform::UEFI;
    info.color_mode = ColorMode::Argb32;

    // Find the ACPI Table
    info.acpi_rsdptr = match st.find_config_table(::uefi::table::cfg::ACPI2_GUID) {
        Some(val) => val as u64,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    // Find the SMBIOS Table
    info.smbios = match st.find_config_table(::uefi::table::cfg::SMBIOS_GUID) {
        Some(val) => val as u64,
        None => 0,
    };

    // Check the CPU
    let invocation = Invocation::new();
    if !invocation.is_compatible() {
        writeln!(
            st.stdout(),
            "Attempts to boot the operating system, but it is not compatible with this processor."
        )
        .unwrap();
        return Status::LOAD_ERROR;
    }

    // Init graphics
    if let Ok(gop) = bs.locate_protocol::<::uefi::proto::console::gop::GraphicsOutput>() {
        let gop = unsafe { &mut *gop.get() };

        let gop_info = gop.current_mode_info();
        let mut fb = gop.frame_buffer();
        info.vram_base = fb.as_mut_ptr() as usize as u64;

        let stride = gop_info.stride();
        let (mut width, mut height) = gop_info.resolution();

        if width > stride {
            // GPD micro PC fake landscape mode
            swap(&mut width, &mut height);
        }

        info.vram_stride = stride as u16;
        info.screen_width = width as u16;
        info.screen_height = height as u16;

        debug::Console::init(info.vram_base as usize, width, height, stride);
    } else if !info.flags.contains(BootFlags::HEADLESS) {
        writeln!(st.stdout(), "Error: GOP Not Found").unwrap();
        return Status::LOAD_ERROR;
    }

    // Load the KERNEL
    let blob = match get_file(handle, &bs, KERNEL_PATH) {
        Ok(blob) => (blob),
        Err(status) => {
            writeln!(st.stdout(), "Error: Load failed {}", KERNEL_PATH).unwrap();
            return status;
        }
    };
    let mut kernel = ElfLoader::new(&blob);
    if kernel.recognize().is_err() {
        writeln!(st.stdout(), "Error: BAD KERNEL SIGNATURE FOUND").unwrap();
        return Status::LOAD_ERROR;
    }
    let bounds = kernel.image_bounds();
    info.kernel_base = bounds.0.as_u64();

    // Load the initrd
    match get_file(handle, &bs, INITRD_PATH) {
        Ok(blob) => {
            info.initrd_base = blob.as_ptr() as u32;
            info.initrd_size = blob.len() as u32;
            forget(blob);
        }
        Err(status) => {
            writeln!(st.stdout(), "Error: Load failed {}", INITRD_PATH).unwrap();
            return status;
        }
    };

    unsafe {
        match PageManager::init_first(&bs) {
            Ok(_) => (),
            Err(err) => {
                writeln!(st.stdout(), "Error: {:?}", err).unwrap();
                return err;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exit Boot Services
    //

    // because some UEFI implementations require an additional buffer during exit_boot_services
    let mmap_size = st.boot_services().memory_map_size();
    let buf_size = mmap_size.map_size * 2;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(::uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, buf_size) };
    let (_st, mm) = st.exit_boot_services(handle, buf).unwrap();
    uefi::alloc::exit_boot_services();

    // ------------------------------------------------------------------------

    unsafe {
        PageManager::init_late(&mut info, mm);
    }

    let entry = kernel.locate(VirtualAddress(info.kernel_base));

    let stack_size: usize = 0x4000;
    let new_sp = VirtualAddress(info.kernel_base + 0x3FFFF000);
    PageManager::valloc(new_sp - stack_size, stack_size);

    // println!("hello, world");
    unsafe {
        invocation.invoke_kernel(&info, entry, new_sp);
    }
}

pub trait MyUefiLib {
    fn find_config_table(&self, _: ::uefi::Guid) -> Option<*const c_void>;
}

impl MyUefiLib for SystemTable<::uefi::table::Boot> {
    fn find_config_table(&self, guid: ::uefi::Guid) -> Option<*const c_void> {
        for entry in self.config_table() {
            if entry.guid == guid {
                return Some(entry.address);
            }
        }
        None
    }
}
