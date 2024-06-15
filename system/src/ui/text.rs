// Text Drawing

use super::font::*;
use crate::*;
use alloc::borrow::Cow;
use core::num::NonZeroUsize;
use megstd::drawing::*;

pub struct AttributedString<'a> {
    text: Cow<'a, str>,
    attributes: AttributeSet,
}

impl AttributedString<'_> {
    #[inline]
    pub fn new() -> AttributeSet {
        AttributeSet::new()
    }

    #[inline]
    pub fn text(&self) -> &Cow<str> {
        &self.text
    }

    #[inline]
    pub fn font(&self) -> &FontDescriptor {
        &self.attributes.font
    }

    #[inline]
    pub fn color(&self) -> Color {
        self.attributes.color
    }

    #[inline]
    pub fn line_break_mode(&self) -> LineBreakMode {
        self.attributes.line_break_mode
    }

    #[inline]
    pub fn align(&self) -> TextAlignment {
        self.attributes.align
    }

    #[inline]
    pub fn valign(&self) -> VerticalAlignment {
        self.attributes.valign
    }

    #[inline]
    pub fn shadow_color(&self) -> Color {
        self.attributes.shadow_color
    }

    #[inline]
    pub fn shadow_offset(&self) -> Point {
        self.attributes.shadow_offset
    }

    #[inline]
    pub fn bounding_size(&self, size: Size, max_lines: usize) -> Size {
        TextProcessing::bounding_size(
            self.font(),
            &self.text,
            size,
            max_lines,
            self.line_break_mode(),
        )
    }

    #[inline]
    pub fn draw_text(&self, bitmap: &mut BitmapRefMut, rect: Rect, max_lines: usize) {
        TextProcessing::draw_text(
            bitmap,
            &self.text,
            self.font(),
            rect,
            self.color(),
            max_lines,
            self.line_break_mode(),
            self.align(),
            self.valign(),
            self.shadow_color(),
            self.shadow_offset(),
        );
    }
}

pub struct AttributeSet {
    font: FontDescriptor,
    color: Color,
    line_break_mode: LineBreakMode,
    align: TextAlignment,
    valign: VerticalAlignment,
    shadow_color: Color,
    shadow_offset: Point,
}

impl AttributeSet {
    #[inline]
    pub fn new() -> Self {
        Self {
            font: FontManager::ui_font(),
            color: Color::BLACK,
            line_break_mode: LineBreakMode::default(),
            align: TextAlignment::Leading,
            valign: VerticalAlignment::Center,
            shadow_color: Color::TRANSPARENT,
            shadow_offset: Point::default(),
        }
    }

    #[inline]
    pub fn text(self, text: &str) -> AttributedString {
        AttributedString {
            text: Cow::Borrowed(text),
            attributes: self,
        }
    }

    #[inline]
    pub fn font(mut self, font: &FontDescriptor) -> Self {
        self.font = font.clone();
        self
    }

    #[inline]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[inline]
    pub fn line_break_mode(mut self, line_break_mode: LineBreakMode) -> Self {
        self.line_break_mode = line_break_mode;
        self
    }

    #[inline]
    pub fn align(mut self, align: TextAlignment) -> Self {
        self.align = align;
        self
    }

    #[inline]
    pub fn center(mut self) -> Self {
        self.align = TextAlignment::Center;
        self
    }

    #[inline]
    pub fn trailing(mut self) -> Self {
        self.align = TextAlignment::Trailing;
        self
    }

    #[inline]
    pub fn valign(mut self, valign: VerticalAlignment) -> Self {
        self.valign = valign;
        self
    }

    #[inline]
    pub fn top_left(mut self) -> Self {
        self.valign = VerticalAlignment::Top;
        self.align = TextAlignment::Left;
        self
    }

    #[inline]
    pub fn top_center(mut self) -> Self {
        self.valign = VerticalAlignment::Top;
        self.align = TextAlignment::Center;
        self
    }

    #[inline]
    pub fn top_right(mut self) -> Self {
        self.valign = VerticalAlignment::Top;
        self.align = TextAlignment::Right;
        self
    }

    #[inline]
    pub fn middle_left(mut self) -> Self {
        self.valign = VerticalAlignment::Center;
        self.align = TextAlignment::Left;
        self
    }

    #[inline]
    pub fn middle_center(mut self) -> Self {
        self.valign = VerticalAlignment::Center;
        self.align = TextAlignment::Center;
        self
    }

    #[inline]
    pub fn middle_right(mut self) -> Self {
        self.valign = VerticalAlignment::Center;
        self.align = TextAlignment::Right;
        self
    }

    #[inline]
    pub fn bottom_left(mut self) -> Self {
        self.valign = VerticalAlignment::Bottom;
        self.align = TextAlignment::Left;
        self
    }

    #[inline]
    pub fn bottom_center(mut self) -> Self {
        self.valign = VerticalAlignment::Bottom;
        self.align = TextAlignment::Center;
        self
    }

    #[inline]
    pub fn bottom_right(mut self) -> Self {
        self.valign = VerticalAlignment::Bottom;
        self.align = TextAlignment::Right;
        self
    }

    #[inline]
    pub fn shadow(mut self, color: Color, offset: Point) -> Self {
        self.shadow_color = color;
        self.shadow_offset = offset;
        self
    }
}

pub struct TextProcessing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBreakMode {
    NoWrap,
    CharWrapping,
    WordWrapping,
    TrancatingTail,
}

impl Default for LineBreakMode {
    fn default() -> Self {
        Self::CharWrapping
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Leading,
    Trailing,
}

impl Default for TextAlignment {
    fn default() -> Self {
        Self::Leading
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Bottom,
    Center,
}

impl Default for VerticalAlignment {
    fn default() -> Self {
        Self::Top
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LineStatus {
    pub start_position: usize,
    pub end_position: usize,
    pub width: u32,
    pub height: u32,
}

impl LineStatus {
    #[inline]
    const fn empty() -> Self {
        Self {
            start_position: 0,
            end_position: 0,
            width: 0,
            height: 0,
        }
    }

    #[inline]
    fn new_line(&mut self, start_position: usize, width: u32, height: u32) {
        self.start_position = start_position;
        self.end_position = start_position;
        self.width = width;
        self.height = height;
    }
}

impl TextProcessing {
    pub fn line_statuses(
        font: &FontDescriptor,
        text: &str,
        size: Size,
        max_lines: usize,
        line_break: LineBreakMode,
    ) -> Vec<LineStatus> {
        let max_lines = NonZeroUsize::new(max_lines)
            .map(|v| v.get())
            .unwrap_or(usize::MAX);
        let limit_max_lines = 64;
        let mut vec = Vec::with_capacity(usize::min(max_lines, limit_max_lines));

        // TODO: Line Breaking
        let no_wrap = max_lines == 1 && line_break == LineBreakMode::NoWrap;

        let mut current_line = LineStatus::empty();
        current_line.height = font.line_height();
        let mut current_height = current_line.height;
        let mut prev_char = ' ';
        for (index, c) in text.chars().enumerate() {
            if c == '\n' {
                current_line.end_position = index;
                current_height += current_line.height;
                vec.push(current_line);
                current_line = LineStatus::empty();
                if vec.len() >= max_lines || current_height > size.height() {
                    break;
                }
                current_line.new_line(index + 1, 0, font.line_height());
                prev_char = ' ';
            } else {
                current_line.end_position = index;
                let current_width = font.width_of(c);
                let new_line_width =
                    (current_line.width as i32 + font.kern(prev_char, c) + current_width as i32)
                        .max(0) as u32;
                let line_is_over = if no_wrap {
                    current_line.width > size.width
                } else {
                    current_line.width > 0 && new_line_width > size.width
                };
                if line_is_over {
                    current_height += current_line.height;
                    vec.push(current_line);
                    current_line = LineStatus::empty();
                    if vec.len() >= max_lines || current_height > size.height() {
                        break;
                    }
                    current_line.new_line(index, current_width, font.line_height());
                    prev_char = ' ';
                } else {
                    current_line.width = new_line_width;
                    prev_char = c;
                }
            }
        }
        if vec.len() < max_lines && current_line.width > 0 {
            current_line.end_position += 1;
            vec.push(current_line);
        }

        vec
    }

    pub fn bounding_size(
        font: &FontDescriptor,
        text: &str,
        size: Size,
        max_lines: usize,
        line_break: LineBreakMode,
    ) -> Size {
        let lines = Self::line_statuses(font, text, size, max_lines, line_break);
        Size::new(
            lines.iter().fold(0, |v, i| v.max(i.width)),
            lines.iter().fold(0, |v, i| v + i.height),
        )
    }

    /// Write text to bitmap
    pub fn draw_text(
        bitmap: &mut BitmapRefMut,
        text: &str,
        font: &FontDescriptor,
        rect: Rect,
        color: Color,
        max_lines: usize,
        line_break: LineBreakMode,
        align: TextAlignment,
        valign: VerticalAlignment,
        shadow_color: Color,
        shadow_offset: Point,
    ) {
        let Ok(coords) = Coordinates::from_rect(rect) else {
            return;
        };

        // bitmap.draw_rect(rect, Color::YELLOW);

        let lines = Self::line_statuses(font, text, rect.size(), max_lines, line_break);
        let mut chars = text.chars();
        let mut cursor = Point::default();
        let mut prev_position = 0;

        let perferred_height = lines.iter().fold(0, |v, i| v + i.height);
        // let preferred_width = lines.iter().fold(0, |v, i| isize::max(v, i.width));
        cursor.y = match valign {
            VerticalAlignment::Top => coords.top,
            VerticalAlignment::Center => {
                coords.top + ((rect.height() - perferred_height) / 2) as i32
            }
            VerticalAlignment::Bottom => coords.bottom - perferred_height as i32,
        };

        for line in lines {
            for _ in prev_position..line.start_position {
                let _ = chars.next();
            }

            if line.start_position < line.end_position {
                cursor.x = match align {
                    TextAlignment::Leading | TextAlignment::Left => coords.left,
                    TextAlignment::Trailing | TextAlignment::Right => {
                        coords.right - line.width as i32
                    }
                    TextAlignment::Center => coords.left + ((rect.width() - line.width) / 2) as i32,
                };
                let mut prev_char = ' ';

                for index in line.start_position..line.end_position {
                    let c = chars.next().unwrap();

                    if cursor.x >= rect.max_x() {
                        panic!(
                            "OUT OF BOUNDS {} > {}, [{}, {}, {}] {:02x}, TEXT {:#}",
                            cursor.x,
                            rect.width(),
                            line.start_position,
                            line.end_position,
                            index,
                            c as u32,
                            text,
                        );
                    }

                    cursor.x += font.kern(prev_char, c);
                    let font_width = font.width_of(c);

                    // bitmap.draw_rect(
                    //     Rect::new(cursor.x, cursor.y, font_width, line.height),
                    //     Color::LIGHT_BLUE,
                    // );
                    // bitmap.draw_vline(cursor, line.height, Color::LIGHT_RED);

                    if !shadow_color.is_transparent() && !shadow_offset.is_zero() {
                        font.draw_char(c, bitmap, cursor + shadow_offset, shadow_color);
                    }

                    font.draw_char(c, bitmap, cursor, color);
                    cursor.x += font_width as i32;
                    prev_char = c;
                }
            }

            prev_position = line.end_position;
            cursor.y += line.height as i32;
        }
    }
}
