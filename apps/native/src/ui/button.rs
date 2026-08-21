//! `Button` from `@spacedrive/primitives`: bordered, rounded, quiet until
//! hovered. The web component's `default`, `gray`, and `accent` variants are
//! carried; the rest arrive with the components that need them.

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, px, App, BoxShadow, ClickEvent, ElementId, FontWeight, InteractiveElement as _,
	IntoElement, ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
	Styled as _, Window,
};

use crate::theme::ActiveTheme as _;

/// Visual variants, named after the web component's `variant` prop.
///
/// Component API here is extraction-ready surface; not every variant or
/// builder has an in-app caller yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
	/// Transparent fill, hairline border; fills on hover.
	#[default]
	Default,
	/// Solid app-button fill.
	Gray,
	/// Accent fill, white label, soft shadow.
	Accent,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
	id: ElementId,
	label: SharedString,
	variant: ButtonVariant,
	on_click: Option<ClickHandler>,
}

impl Button {
	pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
		Button {
			id: id.into(),
			label: label.into(),
			variant: ButtonVariant::default(),
			on_click: None,
		}
	}

	pub fn variant(mut self, variant: ButtonVariant) -> Self {
		self.variant = variant;
		self
	}

	pub fn on_click(
		mut self,
		handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
	) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Button {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		// Web spec (size `sm`): px-2 py-0.5, text-sm, font-medium, rounded-xl.
		let base = div()
			.id(self.id)
			.flex()
			.items_center()
			.justify_center()
			.gap(px(6.0))
			.px(px(8.0))
			.py(px(2.0))
			.rounded(px(12.0))
			.border_1()
			.text_size(theme.text_sm)
			.font_weight(FontWeight::MEDIUM);

		let styled = match self.variant {
			ButtonVariant::Default => {
				let hover_bg = theme.app_hover;
				let active_bg = theme.app_selected;
				let line = theme.app_line;
				base.text_color(theme.ink)
					.border_color(line.opacity(0.8))
					.hover(move |style| style.bg(hover_bg).border_color(line))
					.active(move |style| style.bg(active_bg).border_color(line))
			}
			ButtonVariant::Gray => {
				let hover_bg = theme.app_hover;
				let line = theme.app_line;
				base.text_color(theme.ink)
					.bg(theme.app_button)
					.border_color(line.opacity(0.5))
					.hover(move |style| style.bg(hover_bg).border_color(line.opacity(0.7)))
			}
			ButtonVariant::Accent => {
				let accent_faint = theme.accent_faint;
				let accent_deep = theme.accent_deep;
				base.text_color(theme.white)
					.bg(theme.accent)
					.border_color(theme.accent)
					.shadow(vec![BoxShadow::new(
						px(0.0),
						px(1.0),
						theme.app_shade.opacity(0.1),
					)
					.blur_radius(px(4.0))])
					.hover(move |style| style.bg(accent_faint))
					.active(move |style| style.bg(accent_deep))
			}
		};

		styled
			.when_some(self.on_click, |element, on_click| {
				element.on_click(on_click)
			})
			.child(self.label)
	}
}
