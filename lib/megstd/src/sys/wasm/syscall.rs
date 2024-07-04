use crate::sys::megos::svc::Function;
use crate::time::SystemTime;
use core::arch::asm;
use core::mem::MaybeUninit;
use core::time::Duration;

#[allow(dead_code)]
#[link(wasm_import_module = "megos-canary")]
extern "C" {
    fn svc0(_: Function) -> usize;
    fn svc1(_: Function, _: usize) -> usize;
    fn svc2(_: Function, _: usize, _: usize) -> usize;
    fn svc3(_: Function, _: usize, _: usize, _: usize) -> usize;
    fn svc4(_: Function, _: usize, _: usize, _: usize, _: usize) -> usize;
    fn svc5(_: Function, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    fn svc6(_: Function, _: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
}

#[macro_export]
macro_rules! syscall {
    ($nr:ident) => {
        svc0(Function::$nr)
    };
    ($nr:ident, $a1:expr) => {
        svc1(Function::$nr, $a1 as usize)
    };
    ($nr:ident, $a1:expr, $a2:expr) => {
        svc2(Function::$nr, $a1 as usize, $a2 as usize)
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr) => {
        svc3(Function::$nr, $a1 as usize, $a2 as usize, $a3 as usize)
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        svc4(
            Function::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
        )
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        svc5(
            Function::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
            $a5 as usize,
        )
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        svc6(
            Function::$nr,
            $a1 as usize,
            $a2 as usize,
            $a3 as usize,
            $a4 as usize,
            $a5 as usize,
            $a6 as usize,
        )
    };
}

#[inline]
pub fn os_exit() -> ! {
    unsafe {
        syscall!(Exit);
        asm!("", options(noreturn, nostack));
    }
}

/// Display a string.
#[inline]
pub fn os_print(s: &str) {
    unsafe {
        let _ = syscall!(PrintString, s.as_ptr(), s.len());
    }
}

/// Get the value of the monotonic timer in microseconds.
#[inline]
pub fn os_monotonic() -> u32 {
    unsafe { syscall!(Monotonic) as u32 }
}

#[inline]
pub fn os_time_now() -> SystemTime {
    let mut result = MaybeUninit::<SystemTime>::zeroed();
    unsafe {
        syscall!(Time, 0, result.as_mut_ptr());
        result.assume_init()
    }
}

#[inline]
pub fn os_time_monotonic() -> Duration {
    let mut result = MaybeUninit::<Duration>::zeroed();
    unsafe {
        syscall!(Time, 1, result.as_mut_ptr());
        result.assume_init()
    }
}

/// Blocks a thread for the specified microseconds.
#[inline]
pub fn os_usleep(us: u32) {
    unsafe {
        let _ = syscall!(Usleep, us);
    }
}

/// Get the system version information.
#[inline]
pub fn os_version() -> u32 {
    unsafe { syscall!(GetSystemInfo, 0) as u32 }
}

/// Create a new window.
#[inline]
#[must_use]
pub fn os_new_window1(title: &str, width: u32, height: u32) -> usize {
    unsafe { syscall!(NewWindow, title.as_ptr(), title.len(), width, height) }
}

/// Create a new window.
#[inline]
#[must_use]
pub fn os_new_window2(
    title: &str,
    width: u32,
    height: u32,
    bg_color: u32,
    options: usize,
) -> usize {
    unsafe {
        syscall!(
            NewWindow,
            title.as_ptr(),
            title.len(),
            width,
            height,
            bg_color,
            options
        )
    }
}

/// Close a window.
#[inline]
pub fn os_close_window(window: usize) {
    unsafe {
        let _ = syscall!(CloseWindow, window);
    }
}

/// Create a drawing context
#[inline]
pub fn os_begin_draw(window: usize) -> usize {
    unsafe { syscall!(BeginDraw, window) }
}

/// Discard the drawing context and reflect it to the screen
#[inline]
pub fn os_end_draw(ctx: usize) {
    unsafe {
        let _ = syscall!(EndDraw, ctx);
    }
}

/// Draw a string in a window.
#[inline]
pub fn os_win_draw_string(ctx: usize, x: i32, y: i32, s: &str, color: u32) {
    unsafe {
        let _ = syscall!(DrawString, ctx, x, y, s.as_ptr(), s.len(), color);
    }
}

#[inline]
pub fn os_draw_shape(ctx: usize, x: i32, y: i32, width: u32, height: u32, params: &OsDrawShape) {
    unsafe {
        let _ = syscall!(DrawShape, ctx, x, y, width, height, params as *const _);
    }
}

#[inline]
pub fn os_window_max_fps(window: usize, fps: usize) {
    unsafe {
        let _ = syscall!(WindowFpsThrottle, window, fps);
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct OsDrawShape {
    pub radius: u32,
    pub bg_color: u32,
    pub border_color: u32,
}

/// Fill a rectangle in a window.
#[inline]
pub fn os_win_fill_rect(ctx: usize, x: i32, y: i32, width: u32, height: u32, color: u32) {
    unsafe {
        let _ = syscall!(FillRect, ctx, x, y, width, height, color);
    }
}

#[inline]
pub fn os_win_draw_line(ctx: usize, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
    unsafe {
        let _ = syscall!(DrawLine, ctx, x1, y1, x2, y2, color);
    }
}

/// Wait for key event
#[inline]
pub fn os_wait_char(window: usize) -> u32 {
    unsafe { syscall!(WaitChar, window) as u32 }
}

/// Read a key event
#[inline]
pub fn os_read_char(window: usize) -> u32 {
    unsafe { syscall!(ReadChar, window) as u32 }
}

/// Draw a bitmap in a window
#[inline]
pub fn os_blt8(ctx: usize, x: i32, y: i32, bitmap: usize) {
    unsafe {
        let _ = syscall!(Blt8, ctx, x, y, bitmap);
    }
}

#[inline]
pub fn os_blt8_sub(ctx: usize, x: i32, y: i32, bitmap: usize, w: u32, h: u32) {
    unsafe {
        let _ = syscall!(Blt8, ctx, x, y, bitmap, w, h);
    }
}

#[inline]
pub fn os_blt32(ctx: usize, x: i32, y: i32, bitmap: usize) {
    unsafe {
        let _ = syscall!(Blt32, ctx, x, y, bitmap);
    }
}

#[inline]
pub fn os_blt32_sub(ctx: usize, x: i32, y: i32, bitmap: usize, w: u32, h: u32) {
    unsafe {
        let _ = syscall!(Blt32, ctx, x, y, bitmap, w, h);
    }
}

/// Draw a bitmap in a window
#[inline]
pub fn os_blt1(ctx: usize, x: i32, y: i32, bitmap: usize, color: u32, mode: usize) {
    unsafe {
        let _ = syscall!(Blt1, ctx, x, y, bitmap, color, mode);
    }
}

/// TEST
#[inline]
pub fn os_blend_rect(bitmap: usize, x: i32, y: i32, width: u32, height: u32, color: u32) {
    unsafe {
        let _ = syscall!(BlendRect, bitmap, x, y, width, height, color);
    }
}

/// Returns a simple pseudo-random number
///
/// # Safety
///
/// Since this system call returns a simple pseudo-random number,
/// it should not be used in situations where random number safety is required.
#[inline]
pub fn os_rand() -> u32 {
    unsafe { syscall!(Rand) as u32 }
}

/// Set the seed of the random number.
#[inline]
pub fn os_srand(srand: u32) -> u32 {
    unsafe { syscall!(Srand, srand) as u32 }
}

/// Allocates memory blocks with a simple allocator
#[inline]
#[must_use]
pub unsafe fn os_alloc(size: usize, align: usize) -> *mut u8 {
    syscall!(Alloc, size, align) as *mut u8
}

/// Frees an allocated memory block
#[inline]
pub unsafe fn os_dealloc(ptr: *mut u8, size: usize, align: usize) {
    let _ = syscall!(Dealloc, ptr, size, align);
}

#[inline]
#[must_use]
pub fn os_open(name: &str, options: usize) -> isize {
    unsafe { syscall!(Open, name.as_ptr(), name.len(), options) as isize }
}

#[inline]
pub fn os_close(handle: usize) -> isize {
    unsafe { syscall!(Close, handle) as isize }
}

#[inline]
pub fn os_read(handle: usize, buf: &mut [u8]) -> isize {
    unsafe { syscall!(Read, handle, buf.as_mut_ptr(), buf.len()) as isize }
}

#[inline]
pub fn os_write(handle: usize, buf: &[u8]) -> isize {
    unsafe { syscall!(Write, handle, buf.as_ptr(), buf.len()) as isize }
}

#[inline]
pub fn os_lseek(handle: usize, offset: i32, whence: usize) -> isize {
    unsafe { syscall!(LSeek, handle, offset, whence) as isize }
}
