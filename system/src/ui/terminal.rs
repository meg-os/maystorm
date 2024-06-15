use crate::io::tty::*;
use crate::ui::font::*;
use crate::ui::window::*;
use crate::*;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::{Context, Poll};
use megstd::drawing::*;

const DEFAULT_INSETS: EdgeInsets = EdgeInsets::new(0, 0, 0, 0);
// const DEFAULT_ATTRIBUTE: u8 = 0x07;
// const BG_ALPHA: Alpha8 = Alpha8::new(0xE0);
const DEFAULT_ATTRIBUTE: u8 = 0xF8;
// const BG_ALPHA: Alpha8 = Alpha8::OPAQUE;
const BG_ALPHA: Alpha8 = Alpha8::new(0xC0);

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
    alpha: Alpha8,
    font: FontDescriptor,
    cols: u32,
    rows: u32,
    insets: EdgeInsets,
    x: u32,
    y: u32,
    default_attribute: u8,
    attribute: u8,
    fg_color: Color,
    bg_color: Color,
    is_cursor_enabled: bool,
    palette: [TrueColor; 16],
}

impl Terminal {
    pub const DEFAULT_PALETTE: [TrueColor; 16] = [
        TrueColor::BLACK,
        TrueColor::BLUE,
        TrueColor::GREEN,
        TrueColor::CYAN,
        TrueColor::RED,
        TrueColor::MAGENTA,
        TrueColor::BROWN,
        TrueColor::LIGHT_GRAY,
        TrueColor::DARK_GRAY,
        TrueColor::LIGHT_BLUE,
        TrueColor::LIGHT_GREEN,
        TrueColor::LIGHT_CYAN,
        TrueColor::LIGHT_RED,
        TrueColor::LIGHT_MAGENTA,
        TrueColor::YELLOW,
        TrueColor::WHITE,
    ];

    pub fn from_window(
        window: WindowHandle,
        insets: Option<EdgeInsets>,
        font: FontDescriptor,
        alpha: Alpha8,
        attribute: u8,
        palette: Option<&[TrueColor; 16]>,
    ) -> Self {
        let insets = insets.unwrap_or(DEFAULT_INSETS);
        let attribute = if attribute > 0 {
            attribute
        } else {
            DEFAULT_ATTRIBUTE
        };
        let palette = *palette.unwrap_or(&Self::DEFAULT_PALETTE);
        let (fg_color, bg_color) = Self::_split_attr(&palette, attribute, alpha);

        let rect = window.content_size().bounds().insets_by(insets);
        let cols = rect.width() / font.em_width();
        let rows = rect.height() / font.line_height();

        Self {
            window,
            alpha,
            font: font.clone(),
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            default_attribute: attribute,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
            palette,
        }
    }

    pub fn new(
        cols: u32,
        rows: u32,
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
        let window_size = Size::new(font.em_width() * cols, font.line_height() * rows) + insets;

        let window = RawWindowBuilder::new()
            .frame(Rect::new(
                screen_insets.left + 16 + 24 * n_instances as i32,
                screen_insets.top + 16 + 24 * n_instances as i32,
                window_size.width,
                window_size.height,
            ))
            .bg_color(bg_color)
            // .style_add(WindowStyle::DARK_MODE)
            .build("Terminal");

        Self {
            window,
            alpha,
            font: font.clone(),
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            default_attribute: attribute,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
            palette,
        }
    }

    fn split_attr(&self, val: u8, alpha: Alpha8) -> (Color, Color) {
        Self::_split_attr(&self.palette, val, alpha)
    }

    fn _split_attr(palette: &[TrueColor; 16], val: u8, alpha: Alpha8) -> (Color, Color) {
        (
            Color::from(palette[(val & 0x0F) as usize]),
            Color::from(palette[(val >> 4) as usize].with_opacity(alpha)),
        )
    }

    fn scroll_up(&mut self) {
        let h = self.font.line_height();

        let frame = Rect::from(self.window.content_size()).insets_by(self.insets);
        let rect = Rect::new(0, h as i32, frame.width(), frame.height() - h);
        let rect2 = Rect::new(0, frame.height() as i32 - h as i32, frame.width(), h);
        self.window
            .draw_in_rect(frame, |bitmap| {
                bitmap.copy(Point::default(), rect);
                bitmap.fill_rect(rect2, self.bg_color);
            })
            .unwrap();
        self.window.set_needs_display();
    }

    fn put_char(&mut self, c: char) -> Option<Rect> {
        if self.window.validate().is_none() {
            return None;
        }

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
                    self.insets.left + (self.x * w) as i32,
                    self.insets.top + (self.y * h) as i32,
                    w,
                    h,
                );
                self.window
                    .draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), self.bg_color);
                        self.font
                            .draw_char(c, bitmap, Point::default(), self.fg_color);
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
        if self.window.validate().is_none() {
            return;
        }

        let w = self.font.em_width();
        let h = self.font.line_height();
        let dims = self.dims();
        if self.x >= dims.0 || self.y >= dims.1 {
            return;
        }

        let rect = Rect::new(
            self.insets.left + (w * self.x) as i32,
            self.insets.top + (h * self.y) as i32,
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
            window: self.window.clone(),
        })
    }
}

impl TtyWrite for Terminal {
    fn reset(&mut self) -> Result<(), TtyError> {
        if self.window.validate().is_none() {
            return Ok(());
        }
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

    fn dims(&self) -> (u32, u32) {
        (self.cols, self.rows)
    }

    fn cursor_position(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    fn set_cursor_position(&mut self, x: u32, y: u32) {
        let old_cursor = self.set_cursor_enabled(false);
        self.x = x;
        self.y = y;
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
            self.default_attribute
        };
        self.attribute = attribute;
        let (fg_color, bg_color) = self.split_attr(attribute, self.alpha);
        self.fg_color = fg_color;
        self.bg_color = bg_color;
    }

    fn attributes(&self) -> u8 {
        self.attribute
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
