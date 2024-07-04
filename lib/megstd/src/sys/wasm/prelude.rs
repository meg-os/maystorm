use crate::sys::syscall::*;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(OsPrint(), $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = writeln!(OsPrint(), $($arg)*);
    }};
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);
    os_exit();
}

pub struct OsPrint();

impl core::fmt::Write for OsPrint {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        os_print(s);
        Ok(())
    }
}
