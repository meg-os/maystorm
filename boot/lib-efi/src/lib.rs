#![no_std]
#![feature(alloc_error_handler)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use boot::get_image_file_system;
use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::media::file::*;
use uefi::CStr16;

extern crate alloc;

pub mod debug;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let _ = write!(debug::Console::shared(), $($arg)*);
    };
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        let _ = writeln!(debug::Console::shared(), $($arg)*);
    };
}

pub fn get_file(handle: Handle, path: &str) -> Result<Box<[u8]>, Status> {
    let Ok(mut fs) = get_image_file_system(handle) else {
        return Err(Status::LOAD_ERROR);
    };
    let mut root = match fs.open_volume() {
        Ok(val) => val,
        Err(err) => return Err(err.status()),
    };

    let mut path = path
        .chars()
        .map(|c| match c {
            '/' => '\\',
            _ => c,
        })
        .map(|c| c as u16)
        .collect::<Vec<u16>>();
    path.push(0);
    let path = CStr16::from_u16_with_nul(&path).unwrap();

    let handle = match root
        .handle()
        .open(path, FileMode::Read, FileAttribute::empty())
    {
        Ok(handle) => handle,
        Err(err) => {
            return Err(err.status());
        }
    };

    let mut file = match handle.into_type().unwrap() {
        FileType::Regular(file) => file,
        FileType::Dir(_) => return Err(Status::UNSUPPORTED),
    };

    match file.set_position(RegularFile::END_OF_FILE) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };
    let file_size = match file.get_position() {
        Ok(val) => val,
        Err(err) => return Err(err.status()),
    } as usize;
    match file.set_position(0) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };

    let mut buffer = Vec::new();
    if buffer.try_reserve(file_size).is_err() {
        return Err(Status::OUT_OF_RESOURCES);
    }
    unsafe {
        buffer.set_len(file_size);
    }

    file.read(buffer.as_mut_slice())
        .map(|size| {
            buffer.resize(size, 0);
            buffer.into_boxed_slice()
        })
        .map_err(|v| v.status())
}
