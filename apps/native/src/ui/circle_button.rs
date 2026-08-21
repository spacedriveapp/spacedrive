//! `CircleButton` from `@spacedrive/primitives` — Spacedrive's signature
//! round icon button: a top-lit vertical gradient, 1px line border, and a
//! soft drop shadow. `accent(true)` makes it the blue primary; otherwise it
//! reads as a neutral app-box control.

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, linear_color_stop, linear_gradient, px, App, BoxShadow, ClickEvent, ElementId, FontWeight,
	InteractiveElement as _, IntoElement, ParentElement as _, Pixels, RenderOnce, SharedString,
	StatefulInteractiveElement as _, Styled as _, Window,
};

use super::lighten;
use crate::theme::ActiveTheme as _;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct CircleButton {
	id: ElementId,
	/// Glyph or short label centered in the circle. An icon system replaces
	/// this once one exists.
	glyph: SharedString,
	accent: bool,
	diameter: Pixels,
	on_click: Option<ClickHandler>,
}

impl CircleButton {
	pub fn new(id: impl Into<ElementId>, glyph: impl Into<SharedString>) -> Self {
		CircleButton {
			id: id.into(),
			glyph: glyph.into(),
			accent: false,
			// Web spec size `md`: h-8 w-8.
			diameter: px(32.0),
			on_click: None,
		}
	}

	pub fn accent(mut self, accent: bool) -> Self {
		self.accent = accent;
		self
	}

	// Extraction-ready surface; not every builder has an in-app caller yet.
	#[allow(dead_code)]
	pub fn diameter(mut self, diameter: Pixels) -> Self {
		self.diameter = diameter;
		self
	}

	#[allow(dead_code)]
	pub fn on_click(
		mut self,
		handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
	) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

/// The signature gradient at a given lift: top stop slightly brighter than
/// the bottom for a subtle glossy sheen, brightening on hover and flattening
/// when pressed.
fn sheen(base: gpui::Hsla, lift: f32) -> gpui::Background {
	let top = lighten(base, 0.12 + lift);
	let bottom = lighten(base, lift.max(0.0) * 0.5);
	linear_gradient(
		180.0,
		linear_color_stop(top, 0.0),
		linear_color_stop(bottom, 1.0),
	)
}

impl RenderOnce for CircleButton {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		let base = if self.accent {
			theme.accent
		} else {
			theme.app_box
		};
		let border = if self.accent {
			lighten(theme.accent, 0.15)
		} else {
			theme.app_line
		};
		let text = if self.accent { theme.white } else { theme.ink };

		div()
			.id(self.id)
			.flex()
			.items_center()
			.justify_center()
			.size(self.diameter)
			.rounded_full()
			.border_1()
			.border_color(border)
			.bg(sheen(base, 0.0))
			.hover(move |style| style.bg(sheen(base, 0.10)))
			.active(move |style| style.bg(sheen(base, -0.04)))
			.shadow(vec![BoxShadow::new(
				px(0.0),
				px(1.0),
				theme.black.opacity(0.45),
			)
			.blur_radius(px(5.0))])
			.text_size(theme.text_xs)
			.font_weight(FontWeight::MEDIUM)
			.text_color(text)
			.when_some(self.on_click, |element, on_click| {
				element.on_click(on_click)
			})
			.child(self.glyph)
	}
}
