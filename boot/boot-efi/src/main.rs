//! MEG-OS Boot loader for UEFI
#![no_std]
#![no_main]
#![feature(cfg_match)]

pub mod invocation;
pub mod loader;
pub mod page;

use bootprot::*;
use core::mem::*;
use invocation::*;
use lib_efi::{debug, get_file};
use loader::*;
use page::*;
use uefi::{
    boot::{
        exit_boot_services, image_handle, locate_handle_buffer, open_protocol, MemoryType,
        OpenProtocolAttributes, OpenProtocolParams, SearchType,
    },
    guid,
    prelude::*,
    proto::console::gop,
    table::cfg::{ACPI2_GUID, SMBIOS_GUID},
    Guid, Identify,
};

#[allow(unused_imports)]
use core::fmt::Write;

//#define EFI_DTB_TABLE_GUID  {0xb1b621d5, 0xf19c, 0x41a5, {0x83, 0x0b, 0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0}}
const DTB_GUID: Guid = guid!("b1b621d5-f19c-41a5-830b-d9152c69aae0");

static KERNEL_PATH: &str = "/EFI/MEGOS/kernel.bin";
static INITRD_PATH: &str = "/EFI/MEGOS/initrd.img";

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    let handle = image_handle();

    let mut info = BootInfo {
        platform: PlatformType::UefiNative,
        color_mode: ColorMode::Argb32,
        ..Default::default()
    };

    // Find the ACPI Table
    info.acpi_rsdptr = match find_config_table(ACPI2_GUID) {
        Some(val) => val,
        None => {
            uefi::println!("Error: ACPI Table Not Found");
            return Status::LOAD_ERROR;
        }
    };

    // Find DeviceTree
    info.dtb = find_config_table(DTB_GUID).unwrap_or_default();

    // Find the SMBIOS Table
    info.smbios = find_config_table(SMBIOS_GUID).unwrap_or_default();

    // Check the CPU
    let invocation = Invocation::new();
    if !invocation.is_compatible() {
        uefi::println!("{}", Invocation::INCOMPATIBILITY_MESSAGE);
        return Status::LOAD_ERROR;
    }

    // Init graphics
    if let Ok(handle_buffer) =
        locate_handle_buffer(SearchType::ByProtocol(&gop::GraphicsOutput::GUID))
    {
        if let Some(handle_gop) = handle_buffer.first() {
            if let Ok(mut gop) = unsafe {
                open_protocol::<gop::GraphicsOutput>(
                    OpenProtocolParams {
                        handle: *handle_gop,
                        agent: handle,
                        controller: None,
                    },
                    OpenProtocolAttributes::GetProtocol,
                )
            } {
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

                unsafe {
                    debug::Console::init(info.vram_base as usize, width, height, stride);
                }
            }
        }
    }

    // uefi::println!("ACPI:   {:012x}", info.acpi_rsdptr);
    // uefi::println!("SMBIOS: {:012x}", info.smbios);
    // uefi::println!("DTB:    {:012x}", info.dtb);
    // todo!();

    // Load the KERNEL
    let kernel = match get_file(handle, KERNEL_PATH) {
        Ok(v) => v,
        Err(status) => {
            uefi::println!("Error: Load failed {}", KERNEL_PATH);
            return status;
        }
    };
    let kernel = match ElfLoader::parse(&kernel) {
        Some(v) => v,
        None => {
            uefi::println!("Error: BAD KERNEL SIGNATURE FOUND");
            return Status::LOAD_ERROR;
        }
    };
    let bounds = kernel.image_bounds();
    info.kernel_base = bounds.0.as_u64();

    // Load the initrd
    match get_file(handle, INITRD_PATH) {
        Ok(blob) => {
            info.initrd_base = blob.as_ptr() as u32;
            info.initrd_size = blob.len() as u32;
            forget(blob);
        }
        Err(status) => {
            uefi::println!("Error: Load failed {}", INITRD_PATH);
            return status;
        }
    };

    unsafe {
        match PageManager::init_first() {
            Ok(_) => (),
            Err(err) => {
                uefi::println!("Error: {:?}", err);
                return err;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exit Boot Services
    //

    let mm = unsafe { exit_boot_services(MemoryType::LOADER_DATA) };

    // ------------------------------------------------------------------------

    unsafe {
        PageManager::init_late(&mut info, mm);
        let entry = kernel.locate(VirtualAddress(info.kernel_base));

        let stack_size: usize = 0x4000;
        let new_sp = VirtualAddress(info.kernel_base | 0x3FFFF000);
        PageManager::valloc(new_sp - stack_size, stack_size);

        // lib_efi::println!("Starting kernel...");
        invocation.invoke_kernel(info, entry, new_sp);
    }
}

fn find_config_table(guid: ::uefi::Guid) -> Option<u64> {
    uefi::system::with_config_table(|items| {
        for entry in items {
            if entry.guid == guid {
                return Some(entry.address as u64);
            }
        }
        None
    })
}
