//! SpaceUI design tokens, hand-derived from
//! `spaceui/packages/tokens/src/css/theme.css`.
//!
//! Field names transliterate the CSS variable names one-to-one
//! (`--color-app-box` → `app_box`, `--radius-lg` → `radius_lg`) so drift
//! between the two sources is visible in review. Dark theme only for now; the
//! struct-of-values shape means later themes are data, not code.
//!
//! This module depends only on gpui — it extracts to the spaceui repo later.

use gpui::{px, App, Global, Hsla, Pixels};

/// A CSS `hsl(h, s%, l%)` triple as gpui's normalized [`Hsla`].
const fn hsl(h: f32, s: f32, l: f32) -> Hsla {
	Hsla {
		h: h / 360.0,
		s: s / 100.0,
		l: l / 100.0,
		a: 1.0,
	}
}

/// `rem` at the CSS root font size of 16px.
const fn rem(value: f32) -> Pixels {
	px(value * 16.0)
}

/// The full SpaceUI token surface: colors, radii, and the type scale.
///
/// The whole surface is carried even though the scaffold reads only part of
/// it — completeness against theme.css is the contract that keeps drift
/// visible.
#[allow(dead_code)]
pub struct Theme {
	// Accent
	pub accent: Hsla,
	pub accent_faint: Hsla,
	pub accent_deep: Hsla,

	// Text
	pub ink: Hsla,
	pub ink_dull: Hsla,
	pub ink_faint: Hsla,

	// App surfaces
	pub app: Hsla,
	pub app_box: Hsla,
	pub app_dark_box: Hsla,
	pub app_darker_box: Hsla,
	pub app_light_box: Hsla,
	pub app_overlay: Hsla,
	pub app_input: Hsla,
	pub app_focus: Hsla,
	pub app_line: Hsla,
	pub app_divider: Hsla,
	pub app_button: Hsla,
	pub app_selected: Hsla,
	pub app_selected_item: Hsla,
	pub app_hover: Hsla,
	pub app_active: Hsla,
	pub app_shade: Hsla,
	pub app_frame: Hsla,
	pub app_slider: Hsla,
	pub app_explorer_scrollbar: Hsla,

	// Sidebar
	pub sidebar: Hsla,
	pub sidebar_box: Hsla,
	pub sidebar_line: Hsla,
	pub sidebar_ink: Hsla,
	pub sidebar_ink_dull: Hsla,
	pub sidebar_ink_faint: Hsla,
	pub sidebar_divider: Hsla,
	pub sidebar_button: Hsla,
	pub sidebar_selected: Hsla,
	pub sidebar_shade: Hsla,

	// Menu
	pub menu: Hsla,
	pub menu_line: Hsla,
	pub menu_ink: Hsla,
	pub menu_faint: Hsla,
	pub menu_hover: Hsla,
	pub menu_selected: Hsla,
	pub menu_shade: Hsla,

	// Status
	pub status_success: Hsla,
	pub status_warning: Hsla,
	pub status_error: Hsla,
	pub status_info: Hsla,

	// Black/White
	pub black: Hsla,
	pub white: Hsla,

	// Border radius
	pub radius_window: Pixels,
	pub radius_lg: Pixels,
	pub radius_md: Pixels,

	// Type scale (Spacedrive's tighter scale; rem at a 16px root)
	pub text_tiny: Pixels,
	pub text_xs: Pixels,
	pub text_sm: Pixels,
	pub text_base: Pixels,
	pub text_lg: Pixels,
	pub text_xl: Pixels,
	pub text_2xl: Pixels,
	pub text_3xl: Pixels,
	pub text_4xl: Pixels,
	pub text_5xl: Pixels,
	pub text_6xl: Pixels,
	pub text_7xl: Pixels,
}

impl Theme {
	pub const fn dark() -> Self {
		Theme {
			accent: hsl(208.0, 100.0, 57.0),
			accent_faint: hsl(208.0, 100.0, 64.0),
			accent_deep: hsl(208.0, 100.0, 47.0),

			ink: hsl(235.0, 35.0, 92.0),
			ink_dull: hsl(235.0, 10.0, 70.0),
			ink_faint: hsl(235.0, 10.0, 55.0),

			app: hsl(235.0, 15.0, 13.0),
			app_box: hsl(235.0, 15.0, 18.0),
			app_dark_box: hsl(235.0, 15.0, 15.0),
			app_darker_box: hsl(235.0, 16.0, 11.0),
			app_light_box: hsl(235.0, 15.0, 34.0),
			app_overlay: hsl(235.0, 15.0, 17.0),
			app_input: hsl(235.0, 15.0, 20.0),
			app_focus: hsl(235.0, 15.0, 10.0),
			app_line: hsl(235.0, 15.0, 23.0),
			app_divider: hsl(235.0, 15.0, 5.0),
			app_button: hsl(235.0, 15.0, 17.0),
			app_selected: hsl(235.0, 15.0, 24.0),
			app_selected_item: hsl(235.0, 15.0, 18.0),
			app_hover: hsl(235.0, 15.0, 19.0),
			app_active: hsl(235.0, 15.0, 30.0),
			app_shade: hsl(235.0, 15.0, 0.0),
			app_frame: hsl(235.0, 15.0, 25.0),
			app_slider: hsl(235.0, 15.0, 20.0),
			app_explorer_scrollbar: hsl(235.0, 20.0, 25.0),

			sidebar: hsl(235.0, 15.0, 7.0),
			sidebar_box: hsl(235.0, 15.0, 16.0),
			sidebar_line: hsl(235.0, 15.0, 23.0),
			sidebar_ink: hsl(235.0, 15.0, 92.0),
			sidebar_ink_dull: hsl(235.0, 10.0, 70.0),
			sidebar_ink_faint: hsl(235.0, 10.0, 55.0),
			sidebar_divider: hsl(235.0, 15.0, 17.0),
			sidebar_button: hsl(235.0, 15.0, 18.0),
			sidebar_selected: hsl(235.0, 15.0, 24.0),
			sidebar_shade: hsl(235.0, 15.0, 23.0),

			menu: hsl(235.0, 15.0, 10.0),
			menu_line: hsl(235.0, 15.0, 14.0),
			menu_ink: hsl(235.0, 25.0, 92.0),
			menu_faint: hsl(235.0, 5.0, 80.0),
			menu_hover: hsl(235.0, 15.0, 30.0),
			menu_selected: hsl(235.0, 5.0, 30.0),
			menu_shade: hsl(235.0, 5.0, 0.0),

			status_success: hsl(142.0, 76.0, 36.0),
			status_warning: hsl(38.0, 92.0, 50.0),
			status_error: hsl(0.0, 84.0, 60.0),
			status_info: hsl(208.0, 100.0, 57.0),

			black: hsl(0.0, 0.0, 0.0),
			white: hsl(0.0, 0.0, 100.0),

			radius_window: px(10.0),
			radius_lg: px(8.0),
			radius_md: px(6.0),

			text_tiny: rem(0.7),
			text_xs: rem(0.75),
			text_sm: rem(0.8),
			text_base: rem(1.0),
			text_lg: rem(1.125),
			text_xl: rem(1.25),
			text_2xl: rem(1.5),
			text_3xl: rem(1.875),
			text_4xl: rem(2.25),
			text_5xl: rem(3.0),
			text_6xl: rem(4.0),
			text_7xl: rem(5.0),
		}
	}

	/// Install the theme as the app-wide global.
	pub fn init(cx: &mut App) {
		cx.set_global(Theme::dark());
	}
}

impl Global for Theme {}

/// Access the SpaceUI theme from any context that derefs to [`App`].
///
/// gpui-component ships its own `ActiveTheme` returning its theme type; the
/// two must not be imported into the same module.
pub trait ActiveTheme {
	fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
	fn theme(&self) -> &Theme {
		self.global::<Theme>()
	}
}
