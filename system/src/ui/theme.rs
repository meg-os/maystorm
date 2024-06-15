//! Theme Manager

use megstd::drawing::Color;

static THEME: Theme = Theme::new();

/// Theme Manager
pub struct Theme;

impl Theme {
    #[inline]
    const fn new() -> Self {
        Self {}
    }

    #[inline]
    pub fn shared<'a>() -> &'a Self {
        &THEME
    }

    #[inline]
    pub const fn default_desktop_color(&self) -> Color {
        Color::from_rgb(0x2196F3)
    }

    #[inline]
    pub const fn status_bar_background(&self) -> Color {
        Color::from_argb(0x80263238)
    }

    #[inline]
    pub const fn status_bar_foreground(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_default_background(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_default_foreground(&self) -> Color {
        Color::BLACK
    }

    #[inline]
    pub const fn window_default_accent(&self) -> Color {
        Color::LIGHT_BLUE
    }

    #[inline]
    pub const fn window_default_border_light(&self) -> Color {
        Color::LIGHT_GRAY
    }

    #[inline]
    pub const fn window_default_border_dark(&self) -> Color {
        // Color::from_rgb(0x546e7a)
        Color::DARK_GRAY
    }

    #[inline]
    pub const fn window_title_close_foreground(&self) -> Color {
        Color::BLACK
    }

    #[inline]
    pub const fn window_title_close_foreground_dark(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_title_close_active_foreground(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_title_close_active_background(&self) -> Color {
        Color::LIGHT_RED
    }

    #[inline]
    pub const fn window_title_active_background(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_title_active_foreground(&self) -> Color {
        Color::BLACK
    }

    #[inline]
    pub const fn window_title_active_shadow(&self) -> Color {
        Color::from_argb(0x80babdbe)
    }

    #[inline]
    pub const fn window_title_inactive_background(&self) -> Color {
        Color::from_rgb(0xE5E5E5)
    }

    #[inline]
    pub const fn window_title_inactive_foreground(&self) -> Color {
        Color::LIGHT_GRAY
    }

    #[inline]
    pub const fn window_title_active_background_dark(&self) -> Color {
        Color::from_rgb(0x29434e)
    }

    #[inline]
    pub const fn window_title_inactive_background_dark(&self) -> Color {
        Color::from_rgb(0x546e7a)
    }

    #[inline]
    pub const fn window_title_active_foreground_dark(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn window_title_active_shadow_dark(&self) -> Color {
        Color::from_argb(0x80546e7a)
    }

    #[inline]
    pub const fn window_title_inactive_foreground_dark(&self) -> Color {
        Color::from_rgb(0x819ca9)
    }

    #[inline]
    pub const fn button_default_background(&self) -> Color {
        Color::LIGHT_BLUE
    }

    #[inline]
    pub const fn button_default_foreground(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn button_default_border(&self) -> Color {
        Color::BLUE
    }

    #[inline]
    pub const fn button_destructive_background(&self) -> Color {
        Color::LIGHT_RED
    }

    #[inline]
    pub const fn button_destructive_foreground(&self) -> Color {
        Color::WHITE
    }

    #[inline]
    pub const fn button_destructive_border(&self) -> Color {
        Color::RED
    }
}
