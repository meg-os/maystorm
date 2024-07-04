// MEG-OS Window API

pub use crate::drawing::*;
use crate::sys::megos;
use crate::sys::syscall::{self, OsDrawShape};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowHandle(pub usize);

pub type WindowColor = PackedColor;

pub struct Window {
    handle: WindowHandle,
}

impl Window {
    #[inline]
    pub fn new(title: &str, size: Size) -> Self {
        let handle = WindowHandle(syscall::os_new_window1(title, size.width, size.height));
        Self { handle }
    }

    #[inline]
    pub fn close(self) {
        syscall::os_close_window(self.handle.0);
    }

    #[inline]
    pub const fn handle(&self) -> WindowHandle {
        self.handle
    }

    #[inline]
    #[must_use]
    pub fn begin_draw(&self) -> DrawingContext {
        unsafe { DrawingContext::from_raw(syscall::os_begin_draw(self.handle.0)) }
    }

    #[inline]
    pub fn draw<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DrawingContext) -> R,
    {
        let mut context = self.begin_draw();
        let result = f(&mut context);
        drop(context);
        result
    }

    #[inline]
    pub fn wait_char(&self) -> char {
        core::char::from_u32(syscall::os_wait_char(self.handle.0)).unwrap_or('\0')
    }

    #[inline]
    pub fn read_char(&self) -> Option<char> {
        match syscall::os_read_char(self.handle.0) {
            megos::OPTION_CHAR_NONE => None,
            c => Some(unsafe { core::char::from_u32_unchecked(c as u32) }),
        }
    }

    #[inline]
    pub fn set_max_fps(&self, fps: usize) {
        syscall::os_window_max_fps(self.handle.0, fps);
    }
}

pub struct DrawingContext {
    ctx: usize,
}

impl DrawingContext {
    #[inline]
    pub const unsafe fn from_raw(ctx: usize) -> Self {
        Self { ctx }
    }

    #[inline]
    pub const unsafe fn raw_context(&self) -> usize {
        self.ctx
    }

    #[inline]
    pub fn draw_string(&mut self, s: &str, origin: Point, color: WindowColor) {
        syscall::os_win_draw_string(self.ctx, origin.x, origin.y, s, color.into_raw());
    }

    #[inline]
    pub fn draw_line(&mut self, c1: Point, c2: Point, color: WindowColor) {
        syscall::os_win_draw_line(self.ctx, c1.x, c1.y, c2.x, c2.y, color.into_raw())
    }

    #[inline]
    pub fn fill_rect(&mut self, rect: Rect, color: WindowColor) {
        syscall::os_win_fill_rect(
            self.ctx,
            rect.min_x(),
            rect.min_y(),
            rect.width(),
            rect.height(),
            color.into_raw(),
        )
    }

    #[inline]
    pub fn draw_shape(
        &mut self,
        rect: Rect,
        radius: isize,
        bg_color: WindowColor,
        border_color: WindowColor,
    ) {
        let params = OsDrawShape {
            radius: radius as u32,
            bg_color: bg_color.into_raw(),
            border_color: border_color.into_raw(),
        };
        syscall::os_draw_shape(
            self.ctx,
            rect.min_x(),
            rect.min_y(),
            rect.width(),
            rect.height(),
            &params,
        );
    }

    #[inline]
    pub fn blt1<'a, T: AsRef<BitmapRef1<'a>>>(
        &mut self,
        bitmap: &T,
        origin: Point,
        color: WindowColor,
        mode: usize,
    ) {
        syscall::os_blt1(
            self.ctx,
            origin.x,
            origin.y,
            bitmap as *const _ as usize,
            color.into_raw(),
            mode,
        );
    }

    #[inline]
    pub fn blt8<'a, T: AsRef<BitmapRef8<'a>>>(&mut self, bitmap: &T, origin: Point) {
        syscall::os_blt8(self.ctx, origin.x, origin.y, bitmap as *const _ as usize)
    }

    #[inline]
    pub fn blt8_sub<'a, T: AsRef<BitmapRef8<'a>>>(&mut self, bitmap: &T, rect: Rect) {
        syscall::os_blt8_sub(
            self.ctx,
            rect.min_x(),
            rect.min_y(),
            bitmap as *const _ as usize,
            rect.width(),
            rect.height(),
        )
    }

    #[inline]
    pub fn blt32<'a, T: AsRef<BitmapRef32<'a>>>(&mut self, bitmap: &T, origin: Point) {
        syscall::os_blt32(self.ctx, origin.x, origin.y, bitmap as *const _ as usize)
    }

    #[inline]
    pub fn blt32_sub<'a, T: AsRef<BitmapRef32<'a>>>(&mut self, bitmap: &T, rect: Rect) {
        syscall::os_blt32_sub(
            self.ctx,
            rect.min_x(),
            rect.min_y(),
            bitmap as *const _ as usize,
            rect.width(),
            rect.height(),
        )
    }
}

impl Drop for DrawingContext {
    #[inline]
    fn drop(&mut self) {
        syscall::os_end_draw(self.ctx);
    }
}

pub struct WindowBuilder {
    size: Size,
    bg_color: WindowColor,
    options: u32,
    max_fps: usize,
}

impl WindowBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self {
            size: Size::new(300, 400),
            bg_color: WindowColor::WHITE,
            options: 0,
            max_fps: 0,
        }
    }

    /// Create a window from the specified options.
    #[inline]
    pub fn build(self, title: &str) -> Window {
        let handle = WindowHandle(syscall::os_new_window2(
            title,
            self.size.width(),
            self.size.height(),
            self.bg_color.into_raw(),
            self.options as usize,
        ));
        if self.max_fps > 0 {
            syscall::os_window_max_fps(handle.0, self.max_fps);
        }
        Window { handle }
    }

    /// Set window size
    #[inline]
    pub const fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Set background color
    #[inline]
    pub const fn bg_color(mut self, bg_color: WindowColor) -> Self {
        self.bg_color = bg_color;
        self
    }

    /// Sets the window's content bitmap to ARGB32 format.
    #[inline]
    pub const fn bitmap_argb32(mut self) -> Self {
        self.options |= megos::window::USE_BITMAP32;
        self
    }

    /// Makes the border of the window a thin border.
    #[inline]
    pub const fn thin_frame(mut self) -> Self {
        self.options |= megos::window::THIN_FRAME;
        self
    }

    /// Content is opaque
    #[inline]
    pub const fn opaque(mut self) -> Self {
        self.options |= megos::window::OPAQUE_CONTENT;
        self
    }

    #[inline]
    pub const fn fullscreen(mut self) -> Self {
        self.options |= megos::window::FULLSCREEN;
        self
    }

    /// Set window options
    #[inline]
    pub const fn with_options(mut self, options: u32) -> Self {
        self.options = options;
        self
    }

    #[inline]
    pub const fn max_fps(mut self, fps: usize) -> Self {
        self.max_fps = fps;
        self
    }
}
