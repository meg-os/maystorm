// MEG-OS Terminal

use crate::{io::tty::*, ui::font::*, ui::window::*, *};
use alloc::boxed::Box;
use core::{
    fmt::Write,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};
use megstd::drawing::*;

const DEFAULT_INSETS: EdgeInsets = EdgeInsets::new(0, 0, 0, 0);
const DEFAULT_ATTRIBUTE: u8 = 0x07;
const BG_ALPHA: u8 = 0xE0;
// const DEFAULT_ATTRIBUTE: u8 = 0xF8;
// const BG_ALPHA: u8 = 0xFF;

static TA: TerminalAgent = TerminalAgent::new();

struct TerminalAgent {
    n_instances: AtomicUsize,
}

impl TerminalAgent {
    #[inline]
    const fn new() -> Self {
        Self {
            n_instances: AtomicUsize::new(0),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        &TA
    }

    #[inline]
    fn next_instance() -> usize {
        let shared = Self::shared();
        shared.n_instances.fetch_add(1, Ordering::SeqCst)
    }

    // fn console_thread(_: usize) {
    //     let shared = Self::shared();
    //     Scheduler::spawn_async(Task::new(Self::console_manager()));
    //     Scheduler::perform_tasks();
    // }

    // async fn console_manager() {
    //     loop {
    //         //
    //     }
    // }
}

pub struct Terminal {
    window: WindowHandle,
    alpha: u8,
    font: FontDescriptor,
    cols: usize,
    rows: usize,
    insets: EdgeInsets,
    x: usize,
    y: usize,
    attribute: u8,
    fg_color: Color,
    bg_color: Color,
    is_cursor_enabled: bool,
    font_cache: Option<OwnedBitmap32>,
    palette: [TrueColor; 16],
}

impl Terminal {
    pub const PRIMARY_PALETTE: [TrueColor; 16] = [
        TrueColor::PRIMARY_BLACK,
        TrueColor::from_rgb(0x00_00_80),
        TrueColor::from_rgb(0x00_80_00),
        TrueColor::from_rgb(0x00_80_80),
        TrueColor::from_rgb(0x80_00_00),
        TrueColor::from_rgb(0x80_00_80),
        TrueColor::from_rgb(0x80_80_00),
        TrueColor::from_rgb(0xC0_C0_C0),
        TrueColor::from_rgb(0x80_80_80),
        TrueColor::PRIMARY_BLUE,
        TrueColor::PRIMARY_GREEN,
        TrueColor::PRIMARY_CYAN,
        TrueColor::PRIMARY_RED,
        TrueColor::PRIMARY_MAGENTA,
        TrueColor::PRIMARY_YELLOW,
        TrueColor::PRIMARY_WHITE,
    ];

    pub const DEFAULT_PALETTE: [TrueColor; 16] = [
        IndexedColor::BLACK.as_true_color(),
        IndexedColor::BLUE.as_true_color(),
        IndexedColor::GREEN.as_true_color(),
        IndexedColor::CYAN.as_true_color(),
        IndexedColor::RED.as_true_color(),
        IndexedColor::MAGENTA.as_true_color(),
        IndexedColor::BROWN.as_true_color(),
        IndexedColor::LIGHT_GRAY.as_true_color(),
        IndexedColor::DARK_GRAY.as_true_color(),
        IndexedColor::LIGHT_BLUE.as_true_color(),
        IndexedColor::LIGHT_GREEN.as_true_color(),
        IndexedColor::LIGHT_CYAN.as_true_color(),
        IndexedColor::LIGHT_RED.as_true_color(),
        IndexedColor::LIGHT_MAGENTA.as_true_color(),
        IndexedColor::YELLOW.as_true_color(),
        IndexedColor::WHITE.as_true_color(),
    ];

    pub fn from_window(
        window: WindowHandle,
        insets: Option<EdgeInsets>,
        font: FontDescriptor,
        alpha: u8,
        attribute: u8,
        palette: Option<[TrueColor; 16]>,
    ) -> Self {
        let insets = insets.unwrap_or(DEFAULT_INSETS);
        let attribute = if attribute > 0 {
            attribute
        } else {
            DEFAULT_ATTRIBUTE
        };
        let alpha = if alpha > 0 { alpha } else { BG_ALPHA };
        let palette = palette.unwrap_or(Self::PRIMARY_PALETTE);
        let (fg_color, bg_color) = Self::_split_attr(&palette, attribute, alpha);

        let rect = window.content_size().bounds().insets_by(insets);
        let cols = (rect.width() / font.em_width()) as usize;
        let rows = (rect.height() / font.line_height()) as usize;

        Self {
            window,
            alpha,
            font,
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
            font_cache: Self::_fill_cache(&font),
            palette,
        }
    }

    pub fn new(
        cols: usize,
        rows: usize,
        font: FontDescriptor,
        palette: Option<[TrueColor; 16]>,
    ) -> Self {
        let insets = DEFAULT_INSETS;
        let attribute = DEFAULT_ATTRIBUTE;
        let alpha = BG_ALPHA;
        let palette = palette.unwrap_or(Self::DEFAULT_PALETTE);
        let (fg_color, bg_color) = Self::_split_attr(&palette, attribute, alpha);

        let n_instances = TerminalAgent::next_instance();
        let screen_insets = WindowManager::screen_insets();
        let window_size = Size::new(
            font.em_width() * cols as isize,
            font.line_height() * rows as isize,
        ) + insets;

        let window = WindowBuilder::new()
            .frame(Rect::new(
                screen_insets.left + 16 + 24 * n_instances as isize,
                screen_insets.top + 16 + 24 * n_instances as isize,
                window_size.width,
                window_size.height,
            ))
            .bg_color(bg_color)
            .build("Terminal");

        Self {
            window,
            alpha,
            font,
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
            font_cache: Self::_fill_cache(&font),
            palette,
        }
    }

    fn _fill_cache(_font: &FontDescriptor) -> Option<OwnedBitmap32> {
        return None;
        // if font.is_scalable() {
        //     let font_size = Size::new(font.em_width(), font.line_height());
        //     let mut bitmap =
        //         OwnedBitmap32::new(font_size * Size::new(256, 1), TrueColor::TRANSPARENT);
        //     {
        //         let mut bitmap = Bitmap::from(bitmap.as_mut());
        //         for i in 32..128 {
        //             let origin = Point::new(font_size.width * i, 0);
        //             font.draw_char(i as u8 as char, &mut bitmap, origin, Color::LIGHT_BLUE);
        //         }
        //     }
        //     Some(bitmap)
        // } else {
        //     None
        // }
    }

    fn split_attr(&self, val: u8, alpha: u8) -> (Color, Color) {
        Self::_split_attr(&self.palette, val, alpha)
    }

    fn _split_attr(palette: &[TrueColor; 16], val: u8, alpha: u8) -> (Color, Color) {
        (
            Color::from(palette[(val & 0x0F) as usize]),
            Color::from(palette[(val >> 4) as usize].with_opacity(alpha)),
        )
    }

    fn scroll_up(&mut self) {
        let h = self.font.line_height();

        let frame = Rect::from(self.window.content_size()).insets_by(self.insets);
        let rect = Rect::new(0, h, frame.width(), frame.height() - h);
        let rect2 = Rect::new(0, frame.height() - h, frame.width(), h);
        self.window
            .draw_in_rect(frame, |bitmap| {
                bitmap.blt_itself(Point::default(), rect);
                bitmap.fill_rect(rect2, self.bg_color);
            })
            .unwrap();
        self.window.set_needs_display();
    }

    fn put_char(&mut self, c: char) -> Option<Rect> {
        match c {
            '\x08' => {
                if self.x > 0 {
                    self.x -= 1;
                }
                None
            }
            '\t' => {
                for _ in 0..8 - (self.x & 7) {
                    let _ = self.put_char(' ');
                }
                None
            }
            '\r' => {
                self.x = 0;
                None
            }
            '\n' => {
                self.x = 0;
                self.y += 1;
                while self.y >= self.rows {
                    self.scroll_up();
                    self.y -= 1;
                }
                None
            }
            _ => {
                let w = self.font.em_width();
                let h = self.font.line_height();

                if self.x >= self.cols {
                    self.x = 0;
                    self.y += 1;
                }
                if self.y >= self.rows {
                    self.scroll_up();
                    self.y = self.rows - 1;
                }

                let rect = Rect::new(
                    self.insets.left + self.x as isize * w,
                    self.insets.top + self.y as isize * h,
                    w,
                    h,
                );
                self.window
                    .draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), self.bg_color);

                        if let Some(font_cache) = self.font_cache.as_ref() {
                            let font_cache = ConstBitmap::from(font_cache.as_ref());
                            let rect = Rect::new(w * c as isize, 0, w, h);
                            bitmap.blt_transparent(
                                &font_cache,
                                Point::default(),
                                rect,
                                IndexedColor::DEFAULT_KEY,
                            );
                        } else {
                            self.font
                                .draw_char(c, bitmap, Point::default(), self.fg_color);
                        }
                    })
                    .unwrap();

                self.x += 1;
                Some(rect)
            }
        }
    }

    fn put_str(&mut self, s: &str) {
        let old_cursor = self.set_cursor_enabled(false);
        let mut coords: Option<Coordinates> = None;
        for c in s.chars() {
            self.put_char(c)
                .and_then(|v| Coordinates::from_rect(v).ok())
                .map(|c2| match &mut coords {
                    Some(v) => *v += c2,
                    None => coords = Some(c2),
                });
        }
        self.set_cursor_enabled(old_cursor);
        if let Some(v) = coords {
            self.window.invalidate_rect(v.into());
        }
    }

    fn set_needs_update_cursor(&mut self) {
        let w = self.font.em_width();
        let h = self.font.line_height();
        let dims = self.dims();
        if self.x >= dims.0 as usize || self.y >= dims.1 as usize {
            return;
        }

        let rect = Rect::new(
            self.insets.left + w * self.x as isize,
            self.insets.top + h * self.y as isize,
            w,
            h,
        );

        self.window
            .draw_in_rect(rect, |bitmap| {
                bitmap.fill_rect(
                    bitmap.bounds(),
                    if self.is_cursor_enabled {
                        self.fg_color
                    } else {
                        self.bg_color
                    },
                );
            })
            .unwrap();
        self.window.invalidate_rect(rect);
    }
}

impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.put_str(s);
        Ok(())
    }
}

impl TtyRead for Terminal {
    fn read_async(
        &self,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = TtyReadResult> + '_>> {
        Box::pin(ConsoleReader {
            window: self.window,
        })
    }
}

impl TtyWrite for Terminal {
    fn reset(&mut self) -> Result<(), TtyError> {
        let rect = self.window.content_size().into();
        self.window
            .draw_in_rect(rect, |bitmap| {
                bitmap.fill_rect(bitmap.bounds(), self.bg_color);
            })
            .unwrap();
        self.set_cursor_position(0, 0);
        self.window.set_needs_display();
        Ok(())
    }

    fn dims(&self) -> (isize, isize) {
        (self.cols as isize, self.rows as isize)
    }

    fn cursor_position(&self) -> (isize, isize) {
        (self.x as isize, self.y as isize)
    }

    fn set_cursor_position(&mut self, x: isize, y: isize) {
        let old_cursor = self.set_cursor_enabled(false);
        self.x = x as usize;
        self.y = y as usize;
        self.set_cursor_enabled(old_cursor);
    }

    fn is_cursor_enabled(&self) -> bool {
        self.is_cursor_enabled
    }

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let r = self.is_cursor_enabled;
        self.is_cursor_enabled = enabled;
        if enabled || r {
            self.set_needs_update_cursor();
        }
        r
    }

    fn set_attribute(&mut self, attribute: u8) {
        let attribute = if attribute > 0 {
            attribute
        } else {
            DEFAULT_ATTRIBUTE
        };
        self.attribute = attribute;
        let (fg_color, bg_color) = self.split_attr(attribute, self.alpha);
        self.fg_color = fg_color;
        self.bg_color = bg_color;
    }
}

impl Tty for Terminal {}

struct ConsoleReader {
    window: WindowHandle,
}

impl Future for ConsoleReader {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.window.poll_message(cx) {
                Poll::Ready(v) => {
                    if let Some(message) = v {
                        match message {
                            WindowMessage::Char(c) => return Poll::Ready(Ok(c)),
                            _ => self.window.handle_default_message(message),
                        }
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
