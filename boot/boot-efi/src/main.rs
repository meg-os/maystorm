//! MEG-OS Boot loader for UEFI
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(cfg_match)]

pub mod invocation;
pub mod loader;
pub mod page;

use bootprot::*;
use core::fmt::Write;
use core::mem::*;
use invocation::*;
use lib_efi::*;
use loader::*;
use page::*;
use uefi::data_types::Guid;
use uefi::proto::console::gop;
use uefi::table::{
    boot::{MemoryType, OpenProtocolAttributes, OpenProtocolParams, SearchType},
    cfg::{ACPI2_GUID, SMBIOS_GUID},
};
use uefi::Identify;
use uefi::{guid, prelude::*};

//#define EFI_DTB_TABLE_GUID  {0xb1b621d5, 0xf19c, 0x41a5, {0x83, 0x0b, 0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0}}
const DTB_GUID: Guid = guid!("b1b621d5-f19c-41a5-830b-d9152c69aae0");

static KERNEL_PATH: &str = "/EFI/MEGOS/kernel.bin";
static INITRD_PATH: &str = "/EFI/MEGOS/initrd.img";

#[entry]
fn efi_main(handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut st).unwrap();

    let mut info = BootInfo {
        platform: PlatformType::UefiNative,
        color_mode: ColorMode::Argb32,
        ..Default::default()
    };
    let bs = st.boot_services();

    // Find the ACPI Table
    info.acpi_rsdptr = match st.find_config_table(ACPI2_GUID) {
        Some(val) => val,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    // Find DeviceTree
    info.dtb = st.find_config_table(DTB_GUID).unwrap_or_default();

    // Find the SMBIOS Table
    info.smbios = st.find_config_table(SMBIOS_GUID).unwrap_or_default();

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
    if let Ok(handle_buffer) =
        bs.locate_handle_buffer(SearchType::ByProtocol(&gop::GraphicsOutput::GUID))
    {
        if let Some(handle_gop) = handle_buffer.first() {
            if let Ok(mut gop) = unsafe {
                bs.open_protocol::<gop::GraphicsOutput>(
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

    // println!("ACPI:   {:012x}", info.acpi_rsdptr);
    // println!("SMBIOS: {:012x}", info.smbios);
    // println!("DTB:    {:012x}", info.dtb);
    // todo!();

    // Load the KERNEL
    let kernel = match get_file(handle, &bs, KERNEL_PATH) {
        Ok(v) => v,
        Err(status) => {
            writeln!(st.stdout(), "Error: Load failed {}", KERNEL_PATH).unwrap();
            return status;
        }
    };
    let kernel = match ElfLoader::parse(&kernel) {
        Some(v) => v,
        None => {
            writeln!(st.stdout(), "Error: BAD KERNEL SIGNATURE FOUND").unwrap();
            return Status::LOAD_ERROR;
        }
    };
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

    let (_st, mm) = st.exit_boot_services(MemoryType::LOADER_DATA);

    // ------------------------------------------------------------------------

    unsafe {
        PageManager::init_late(&mut info, mm);
        let entry = kernel.locate(VirtualAddress(info.kernel_base));

        let stack_size: usize = 0x4000;
        let new_sp = VirtualAddress(info.kernel_base | 0x3FFFF000);
        PageManager::valloc(new_sp - stack_size, stack_size);

        // println!("Starting kernel...");
        invocation.invoke_kernel(info, entry, new_sp);
    }
}

pub trait MyUefiLib {
    fn find_config_table(&self, _: ::uefi::Guid) -> Option<u64>;
}

impl MyUefiLib for SystemTable<::uefi::table::Boot> {
    fn find_config_table(&self, guid: ::uefi::Guid) -> Option<u64> {
        for entry in self.config_table() {
            if entry.guid == guid {
                return Some(entry.address as u64);
            }
        }
        None
    }
}
