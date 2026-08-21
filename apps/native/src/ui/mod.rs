//! SpaceUI-shaped components over gpui, named after their
//! `@spacedrive/primitives` counterparts. The web rendering is the spec.
//!
//! This module depends only on gpui crates and `theme` — never on app, data,
//! or view code — so it extracts to the spaceui repo later.

mod button;
mod circle_button;
mod sidebar;

pub use button::{Button, ButtonVariant};
pub use circle_button::CircleButton;
pub use sidebar::{SidebarItem, SidebarSectionLabel};

use gpui::Hsla;

/// Blend a color toward white by `t` (0..1), in lightness.
pub(crate) fn lighten(color: Hsla, t: f32) -> Hsla {
	Hsla {
		l: color.l + (1.0 - color.l) * t,
		..color
	}
}
