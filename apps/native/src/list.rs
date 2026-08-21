//! The explorer list view: real directory contents from the daemon, drawn as
//! SpaceUI-styled rows (icon, name, size, modified).
//!
//! The view is a pure consumer of the data plane: it renders the current
//! [`ListingSnapshot`] and wakes itself on watch-channel changes — no awaits
//! and no daemon traffic ever happen on the frame path. Rows come through a
//! `uniform_list`, so only the visible slice is laid out regardless of
//! directory size. Arrow keys move the selection, Enter (or double-click)
//! descends into directories.

use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, px, uniform_list, App, ClickEvent, Context, Entity, FocusHandle, FontWeight,
	InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollStrategy,
	SharedString, StatefulInteractiveElement as _, Styled as _, UniformListScrollHandle, Window,
};

use crate::data::{DataHandle, FileRow, ListingPhase, ListingSnapshot};
use crate::theme::ActiveTheme as _;

const ROW_HEIGHT: f32 = 28.0;
const SIZE_COL_WIDTH: f32 = 90.0;
const MODIFIED_COL_WIDTH: f32 = 150.0;

pub struct ListView {
	data: DataHandle,
	snapshot: Arc<ListingSnapshot>,
	selected: Option<usize>,
	focus_handle: FocusHandle,
	scroll: UniformListScrollHandle,
}

impl ListView {
	pub fn new(data: DataHandle, cx: &mut Context<Self>) -> Self {
		let mut listing = data.watch_listing();
		cx.spawn(async move |this, cx| {
			while listing.changed().await.is_ok() {
				let alive = this.update(cx, |view, cx| view.sync_snapshot(cx));
				if alive.is_err() {
					break;
				}
			}
		})
		.detach();

		ListView {
			snapshot: data.listing(),
			data,
			selected: None,
			focus_handle: cx.focus_handle(),
			scroll: UniformListScrollHandle::new(),
		}
	}

	pub fn focus_handle(&self) -> FocusHandle {
		self.focus_handle.clone()
	}

	/// Absorb a data-plane update: a new target resets selection and scroll,
	/// a refresh of the same directory clamps the selection to the new rows.
	fn sync_snapshot(&mut self, cx: &mut Context<Self>) {
		let next = self.data.listing();
		if next.target != self.snapshot.target {
			self.selected = None;
			self.scroll.scroll_to_item(0, ScrollStrategy::Top);
		} else if let Some(selected) = self.selected {
			if next.rows.is_empty() {
				self.selected = None;
			} else if selected >= next.rows.len() {
				self.selected = Some(next.rows.len() - 1);
			}
		}
		self.snapshot = next;
		cx.notify();
	}

	fn select(&mut self, index: usize, cx: &mut Context<Self>) {
		if index < self.snapshot.rows.len() && self.selected != Some(index) {
			self.selected = Some(index);
			cx.notify();
		}
	}

	fn move_selection(&mut self, delta: i64, cx: &mut Context<Self>) {
		let count = self.snapshot.rows.len();
		if count == 0 {
			return;
		}
		let next = match self.selected {
			Some(current) => (current as i64 + delta).clamp(0, count as i64 - 1) as usize,
			None if delta < 0 => count - 1,
			None => 0,
		};
		if self.selected != Some(next) {
			self.selected = Some(next);
			self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
			cx.notify();
		}
	}

	/// Descend into the row if it is a directory.
	fn activate(&mut self, index: usize) {
		if let Some(row) = self.snapshot.rows.get(index) {
			if row.is_dir {
				self.data.open_directory(row.path.clone());
			}
		}
	}

	fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
		match event.keystroke.key.as_str() {
			"up" => self.move_selection(-1, cx),
			"down" => self.move_selection(1, cx),
			"enter" => {
				if let Some(selected) = self.selected {
					self.activate(selected);
				}
			}
			_ => {}
		}
	}

	fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.flex_shrink_0()
			.flex()
			.items_center()
			.gap(px(8.0))
			.h(px(26.0))
			.px(px(12.0))
			.border_b_1()
			.border_color(theme.app_line)
			.text_size(theme.text_xs)
			.font_weight(FontWeight::MEDIUM)
			.text_color(theme.ink_faint)
			// Leading gap mirrors the row icon column.
			.child(div().w(px(16.0)).flex_shrink_0())
			.child(div().flex_1().child("Name"))
			.child(div().w(px(SIZE_COL_WIDTH)).flex_shrink_0().child("Size"))
			.child(
				div()
					.w(px(MODIFIED_COL_WIDTH))
					.flex_shrink_0()
					.child("Modified"),
			)
	}

	fn render_row(
		&self,
		index: usize,
		row: &FileRow,
		entity: &Entity<Self>,
		cx: &App,
	) -> impl IntoElement {
		let theme = cx.theme();
		let selected = self.selected == Some(index);
		let hover_bg = theme.app_hover;
		let is_dir = row.is_dir;
		let icon_color = if is_dir {
			theme.accent
		} else {
			theme.app_light_box
		};
		let click_entity = entity.clone();
		let focus_handle = self.focus_handle.clone();
		div()
			.id(index)
			.flex()
			.items_center()
			.gap(px(8.0))
			.h(px(ROW_HEIGHT))
			.px(px(12.0))
			.rounded(theme.radius_md)
			.text_size(theme.text_sm)
			.text_color(theme.ink)
			.when(selected, |element| element.bg(theme.app_selected))
			.when(!selected, move |element| {
				element.hover(move |style| style.bg(hover_bg))
			})
			.on_click(move |event: &ClickEvent, window, cx| {
				window.focus(&focus_handle, cx);
				let open = event.click_count() >= 2;
				click_entity.update(cx, |view, cx| {
					view.select(index, cx);
					if open {
						view.activate(index);
					}
				});
			})
			// Icon placeholder: a tinted square until real icons arrive.
			.child(
				div()
					.size(px(14.0))
					.flex_shrink_0()
					.rounded(px(4.0))
					.bg(icon_color.opacity(if is_dir { 0.85 } else { 1.0 })),
			)
			.child(div().flex_1().truncate().child(SharedString::from(row.name.clone())))
			.child(
				div()
					.w(px(SIZE_COL_WIDTH))
					.flex_shrink_0()
					.text_size(theme.text_xs)
					.text_color(theme.ink_faint)
					.child(SharedString::from(row.size_label.clone())),
			)
			.child(
				div()
					.w(px(MODIFIED_COL_WIDTH))
					.flex_shrink_0()
					.text_size(theme.text_xs)
					.text_color(theme.ink_faint)
					.child(SharedString::from(row.modified_label.clone())),
			)
	}

	/// Centered faint message for the phases with nothing to draw.
	fn render_message(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.flex_1()
			.flex()
			.items_center()
			.justify_center()
			.text_size(theme.text_sm)
			.text_color(theme.ink_faint)
			.child(message)
	}
}

impl Render for ListView {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let snapshot = self.snapshot.clone();

		let body: gpui::AnyElement = if snapshot.rows.is_empty() {
			let message: SharedString = match &snapshot.phase {
				ListingPhase::Idle => "Select a volume or folder".into(),
				ListingPhase::Loading => "Loading…".into(),
				ListingPhase::Ready => "Empty folder".into(),
				ListingPhase::Offline => "Daemon offline".into(),
				ListingPhase::NoLibrary => "No library on this daemon yet".into(),
				ListingPhase::Error(error) => format!("Listing failed: {error}").into(),
			};
			self.render_message(message, cx).into_any_element()
		} else {
			let entity = cx.entity();
			let view = entity.clone();
			div()
				.flex_1()
				.min_h(px(0.0))
				.child(
					uniform_list("file-rows", snapshot.rows.len(), move |range, _window, cx| {
						let rows = view.read(cx).snapshot.rows.clone();
						range
							.filter_map(|index| {
								let row = rows.get(index)?;
								Some(view.read(cx).render_row(index, row, &view, cx))
							})
							.collect()
					})
					.size_full()
					.track_scroll(&self.scroll),
				)
				.into_any_element()
		};

		div()
			.size_full()
			.flex()
			.flex_col()
			.bg(theme.app)
			.track_focus(&self.focus_handle)
			.on_key_down(cx.listener(Self::on_key_down))
			.child(self.render_header(cx))
			.child(body)
	}
}
