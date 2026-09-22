//! Tag mode: keyboard tagging over the selection, the native twin of the
//! explorer's `TagAssignmentMode`.
//!
//! The palette is the library's first ten tags, one per number key: 1 to 9,
//! then 0. A key toggles its tag on every selected photo. When all of them
//! carry it, it comes off all of them, and otherwise it goes on all of them.
//! The bar floats over the bottom of the grid while the mode is on.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, px, rgb, App, BoxShadow, FontWeight, Hsla, InteractiveElement as _, IntoElement,
	ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
	Window,
};

use crate::data::TagInfo;
use crate::theme::ActiveTheme as _;
use crate::ui::{Button, ButtonVariant};

/// Palette slots, one per number key.
pub const PALETTE_SIZE: usize = 10;

/// The key for palette slot `slot`, which is also its label on the bar.
pub fn slot_key(slot: usize) -> &'static str {
	["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"][slot]
}

/// One palette entry: the tag, and whether every selected photo carries it.
pub struct Slot {
	pub tag: TagInfo,
	pub active: bool,
}

type ToggleHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type DoneHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct TagBar {
	slots: Vec<Slot>,
	selected: usize,
	notice: Option<SharedString>,
	on_toggle: ToggleHandler,
	on_done: DoneHandler,
}

impl TagBar {
	pub fn new(
		slots: Vec<Slot>,
		selected: usize,
		notice: Option<SharedString>,
		on_toggle: impl Fn(usize, &mut Window, &mut App) + 'static,
		on_done: impl Fn(&mut Window, &mut App) + 'static,
	) -> Self {
		TagBar {
			slots,
			selected,
			notice,
			on_toggle: Rc::new(on_toggle),
			on_done: Rc::new(on_done),
		}
	}
}

impl RenderOnce for TagBar {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		// Creating a tag announces nothing and this bar has no way to make
		// one, so an empty palette says where tags come from.
		let help = if self.slots.is_empty() {
			Some("No tags yet. Create one in Spacedrive to tag from here.")
		} else if self.selected == 0 {
			Some("Select photos to start tagging • Press 1-9/0 to toggle tags • Esc to exit")
		} else {
			None
		};
		let count = match self.selected {
			0 => None,
			selected => Some(format!("{selected} selected")),
		};
		let on_done = self.on_done;

		let label = div()
			.flex()
			.flex_shrink_0()
			.items_center()
			.gap(px(8.0))
			.child(
				div()
					.text_size(theme.text_sm)
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(theme.sidebar_ink)
					.child("Tag Mode"),
			)
			.when_some(count, |label, count| {
				label.child(
					div()
						.text_size(theme.text_xs)
						.text_color(theme.sidebar_ink_dull)
						.child(count),
				)
			});

		let palette = div().flex().flex_1().min_w(px(0.0)).gap(px(6.0)).children(
			self.slots
				.into_iter()
				.enumerate()
				.map(|(index, slot)| render_slot(index, slot, self.on_toggle.clone(), cx)),
		);

		div()
			.absolute()
			.bottom(px(8.0))
			.left(px(4.0))
			.right(px(4.0))
			.child(
				div()
					.px(px(16.0))
					.py(px(12.0))
					.rounded(px(12.0))
					.border_1()
					.border_color(theme.sidebar_line.opacity(0.5))
					.bg(theme.sidebar.opacity(0.92))
					.shadow(vec![BoxShadow::new(
						px(0.0),
						px(4.0),
						theme.app_shade.opacity(0.3),
					)
					.blur_radius(px(16.0))])
					.child(
						div()
							.flex()
							.items_center()
							.gap(px(12.0))
							.child(label)
							.child(palette)
							.child(
								Button::new("tag-mode-done", "Done")
									.variant(ButtonVariant::Accent)
									.on_click(move |_, window, cx| on_done(window, cx)),
							),
					)
					.when_some(help, |bar, help| {
						bar.child(
							div()
								.mt(px(8.0))
								.flex()
								.justify_center()
								.text_size(theme.text_xs)
								.text_color(theme.sidebar_ink_faint)
								.child(help),
						)
					})
					.when_some(self.notice, |bar, notice| {
						bar.child(
							div()
								.mt(px(8.0))
								.flex()
								.justify_center()
								.text_size(theme.text_xs)
								.text_color(theme.status_error)
								.child(notice),
						)
					}),
			)
	}
}

/// A palette button: its key, the tag's name in the tag's color, and a check
/// when every selected photo carries it.
fn render_slot(index: usize, slot: Slot, on_toggle: ToggleHandler, cx: &App) -> impl IntoElement {
	let theme = cx.theme();
	let color: Hsla = rgb(slot.tag.color).into();
	let fill = color.opacity(if slot.active { 0.25 } else { 0.125 });
	let hover = color.opacity(0.25);
	div()
		.id(("tag-slot", index))
		.flex()
		.flex_shrink_0()
		.items_center()
		.gap(px(8.0))
		.px(px(10.0))
		.py(px(4.0))
		.rounded(px(6.0))
		.border_1()
		.border_color(if slot.active {
			color.opacity(0.6)
		} else {
			fill
		})
		.bg(fill)
		.text_color(color)
		.text_size(theme.text_sm)
		.font_weight(FontWeight::MEDIUM)
		.hover(move |style| style.bg(hover))
		.on_click(move |_, window, cx| on_toggle(index, window, cx))
		.child(
			div()
				.min_w(px(16.0))
				.px(px(4.0))
				.rounded(px(3.0))
				.bg(theme.black.opacity(0.2))
				.flex()
				.justify_center()
				.text_size(px(10.0))
				.font_weight(FontWeight::BOLD)
				.child(slot_key(index)),
		)
		.child(div().max_w(px(120.0)).truncate().child(slot.tag.name))
		.when(slot.active, |button| {
			button.child(div().text_size(theme.text_xs).child("✓"))
		})
}
