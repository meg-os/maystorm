#[macro_use]
pub mod prelude;

pub mod fs_imp;
mod os_alloc;

#[macro_use]
pub mod syscall;

#[cfg(feature = "window")]
pub mod window;

pub mod path {
    pub const MAIN_SEPARATOR: &'static str = "/";
}

pub mod fcntl {
    pub const O_ACCMODE: usize = 3;
    pub const O_RDONLY: usize = 0;
    pub const O_WRONLY: usize = 1;
    pub const O_RDWR: usize = 2;
    pub const O_CREAT: usize = 0o00000100;
    pub const O_EXCL: usize = 0o00000200;
    pub const O_NOCTTY: usize = 0o00000400;
    pub const O_TRUNC: usize = 0o00001000;
    pub const O_APPEND: usize = 0o00002000;
    pub const O_NONBLOCK: usize = 0o00004000;
}
