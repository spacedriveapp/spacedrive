//! Sidebar chrome: the uppercase section label and the nav row, shaped after
//! the web app's `SpacesSidebar`.

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, px, App, ClickEvent, ElementId, FontWeight, InteractiveElement as _, IntoElement,
	ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
	Window,
};

use crate::theme::ActiveTheme as _;

/// The small uppercase heading above a group of nav rows.
///
/// The web spec adds letter tracking; gpui has no letter-spacing at this rev,
/// so the label carries weight and case only.
#[derive(IntoElement)]
pub struct SidebarSectionLabel {
	label: SharedString,
}

impl SidebarSectionLabel {
	pub fn new(label: impl Into<SharedString>) -> Self {
		SidebarSectionLabel {
			label: label.into(),
		}
	}
}

impl RenderOnce for SidebarSectionLabel {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.px(px(8.0))
			.pt(px(12.0))
			.pb(px(4.0))
			.text_size(theme.text_tiny)
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(theme.sidebar_ink_faint)
			.child(self.label.to_uppercase())
	}
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One nav row: quiet by default, filled when hovered, selected state carries
/// the brighter ink.
#[derive(IntoElement)]
pub struct SidebarItem {
	id: ElementId,
	label: SharedString,
	detail: Option<SharedString>,
	selected: bool,
	on_click: Option<ClickHandler>,
}

impl SidebarItem {
	pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
		SidebarItem {
			id: id.into(),
			label: label.into(),
			detail: None,
			selected: false,
			on_click: None,
		}
	}

	/// Quiet right-aligned secondary text (a capacity, a count).
	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		let detail = detail.into();
		if !detail.is_empty() {
			self.detail = Some(detail);
		}
		self
	}

	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
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

impl RenderOnce for SidebarItem {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		let hover_bg = theme.sidebar_button;
		div()
			.id(self.id)
			.flex()
			.items_center()
			.gap(px(8.0))
			.h(px(28.0))
			.px(px(8.0))
			.rounded(theme.radius_md)
			.text_size(theme.text_sm)
			.font_weight(FontWeight::MEDIUM)
			.when(self.selected, |element| {
				element
					.bg(theme.sidebar_selected)
					.text_color(theme.sidebar_ink)
			})
			.when(!self.selected, move |element| {
				element
					.text_color(theme.sidebar_ink_dull)
					.hover(move |style| style.bg(hover_bg))
			})
			.when_some(self.on_click, |element, on_click| {
				element.on_click(on_click)
			})
			.child(div().flex_1().truncate().child(self.label))
			.when_some(self.detail, |element, detail| {
				element.child(
					div()
						.flex_shrink_0()
						.text_size(theme.text_tiny)
						.text_color(theme.sidebar_ink_faint)
						.child(detail),
				)
			})
	}
}
