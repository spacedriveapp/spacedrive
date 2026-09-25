//! Photos: Spacedrive's native GPUI app.
//!
//! One window, one grid. Photos does not browse on its own — it follows a
//! Spacedrive file explorer window through the daemon's navigation focus, so
//! navigating in the explorer re-renders this window as the media view of that
//! folder. All daemon traffic runs through the async data plane in [`data`];
//! the frame loop only ever reads snapshots.
//!
//! Following is a toggle. Off, the window holds whatever folder it is showing.
//! `SD_NATIVE_FOCUS_GROUP` picks which group to follow (windows launched from
//! one explorer share its group); `SD_NATIVE_INSTANCE` selects a named daemon
//! instance.
//!
//! Tiles come from the daemon's thumbnail hot tier: this process maps the
//! source's cache file read-only and uploads slots straight to the atlas, and
//! never writes to it. `SD_GRID_BENCH=1` runs the scripted flywheel benchmark
//! over synthetic tiles (stats to stderr, quits when done); `SD_GRID_CELLS`
//! overrides the synthetic cell count.
//!
//! Photos selects the way the explorer does, and tags what is selected in
//! [`tag_mode`]: T enters it, the number keys toggle the palette's tags, and
//! Esc leaves it. Space shows the original under the cursor in
//! [`quick_look`], and the preview follows the cursor while it is open.

mod data;
mod grid;
mod quick_look;
mod source;
mod tag_mode;
mod theme;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
	actions, div, point, px, rgb, size, Action, App, AppContext as _, Bounds, Context, Entity,
	FocusHandle, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render,
	SharedString, Styled as _, Task, TitlebarOptions, Window, WindowBounds, WindowControlArea,
	WindowOptions,
};
use gpui_component::Root;
use gpui_platform::application;
use sd_core::domain::SdPath;
use tokio::sync::mpsc;

use crate::data::{DataHandle, FocusSnapshot, FolderChange, FolderState, TagSnapshot};
use crate::grid::{Cells, Direction, GridView};
use crate::quick_look::QuickLook;
use crate::source::{EmptySource, PvcacheSource, SyntheticSource, TileSource};
use crate::tag_mode::{Slot, TagBar, PALETTE_SIZE};
use crate::theme::{ActiveTheme as _, Theme};
use crate::ui::{Button, ButtonVariant};

const TOOLBAR_HEIGHT: f32 = 44.0;
const STATUS_BAR_HEIGHT: f32 = 26.0;

/// How long a failed tag request stays reported on the bar.
const NOTICE_DURATION: Duration = Duration::from_secs(5);

/// The key context the window's bindings live in.
const KEY_CONTEXT: &str = "Photos";

actions!(
	photos,
	[
		/// Enter tag mode.
		EnterTagMode,
		/// Close the preview, else leave tag mode, else drop the selection.
		Cancel,
		/// Show the original under the cursor, or close the preview.
		TogglePreview,
		SelectAll,
		MoveLeft,
		MoveRight,
		MoveUp,
		MoveDown,
	]
);

/// Toggle palette slot `slot`'s tag on the selection.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = photos, no_json)]
struct ToggleTag {
	slot: usize,
}

fn bind_keys(cx: &mut App) {
	let context = Some(KEY_CONTEXT);
	cx.bind_keys([
		KeyBinding::new("t", EnterTagMode, context),
		KeyBinding::new("escape", Cancel, context),
		KeyBinding::new("space", TogglePreview, context),
		KeyBinding::new("secondary-a", SelectAll, context),
		KeyBinding::new("left", MoveLeft, context),
		KeyBinding::new("right", MoveRight, context),
		KeyBinding::new("up", MoveUp, context),
		KeyBinding::new("down", MoveDown, context),
	]);
	cx.bind_keys(
		(0..PALETTE_SIZE)
			.map(|slot| KeyBinding::new(tag_mode::slot_key(slot), ToggleTag { slot }, context)),
	);
}

fn main() {
	let (cells, bench) = grid::config_from_env();
	let data = data::spawn(
		std::env::var("SD_NATIVE_INSTANCE").ok(),
		data::focus_group_from_env(),
	);

	application().run(move |cx: &mut App| {
		gpui_component::init(cx);
		Theme::init(cx);
		bind_keys(cx);

		// The window is the whole app. Closing it quits, so the next launch
		// from the Apps menu opens a window instead of finding a process
		// with none.
		cx.on_window_closed(|cx, _| {
			if cx.windows().is_empty() {
				cx.quit();
			}
		})
		.detach();

		let bounds = Bounds::centered(None, size(px(1280.0), px(860.0)), cx);
		let options = WindowOptions {
			window_bounds: Some(WindowBounds::Windowed(bounds)),
			titlebar: Some(TitlebarOptions {
				title: Some("Photos".into()),
				appears_transparent: true,
				traffic_light_position: Some(point(px(12.0), px(12.0))),
			}),
			..Default::default()
		};

		cx.open_window(options, |window, cx| {
			let source: Box<dyn TileSource> = match cells {
				Some(cells) => Box::new(SyntheticSource::new(cells)),
				None => Box::new(EmptySource),
			};
			let grid = cx.new(|cx| GridView::new(source, bench, cx));
			let quick_look = QuickLook::attach(window);
			let photos = cx.new(|cx| Photos::new(grid, data.clone(), quick_look, bench, cx));
			// Keys reach the window through the focused view, and nothing
			// else in it takes focus.
			let focus = photos.read(cx).focus_handle.clone();
			window.focus(&focus, cx);
			cx.new(|cx| Root::new(photos, window, cx))
		})
		.expect("failed to open window");
		cx.activate(true);
	});
}

struct Photos {
	grid: Entity<GridView>,
	data: DataHandle,
	focus: Arc<FocusSnapshot>,
	focus_handle: FocusHandle,
	/// What the toolbar calls the listing the grid is showing, which lags the
	/// focus by one retarget.
	title: Option<String>,
	/// Applies the shown listing's later pages and tag changes to the grid.
	/// Replacing it ends the previous listing's.
	folder_changes: Option<Task<()>>,
	/// The benchmark owns the grid's source; adopting a folder would pull it
	/// away mid-run, so the window ignores focus for the duration.
	bench: bool,
	tag_mode: bool,
	tags: Arc<TagSnapshot>,
	/// The failure on show, and the number of the latest one taken from the
	/// plane, so each is shown once.
	notice: Option<SharedString>,
	notice_seen: u64,
	/// The window's hold on the system preview panel, where there is one.
	quick_look: Option<QuickLook>,
	/// Numbers each ask for a file's path to preview, so only the latest
	/// answer reaches the panel.
	preview_ask: u64,
}

impl Photos {
	fn new(
		grid: Entity<GridView>,
		data: DataHandle,
		quick_look: Option<QuickLook>,
		bench: bool,
		cx: &mut Context<Self>,
	) -> Self {
		// The tag bar, the status bar, and an open preview all follow the
		// grid's selection.
		cx.observe(&grid, |photos, _, cx| {
			photos.follow_preview(cx);
			cx.notify();
		})
		.detach();

		let photos = Photos {
			grid,
			focus: data.focus(),
			focus_handle: cx.focus_handle(),
			tags: data.tags(),
			data,
			title: None,
			folder_changes: None,
			bench,
			tag_mode: false,
			notice: None,
			notice_seen: 0,
			quick_look,
			preview_ask: 0,
		};

		// Wake on data-plane snapshot changes; the channel is a tokio watch,
		// which awaits fine on gpui's executor. A snapshot change is also the
		// signal that a folder may be waiting to be picked up.
		let mut focus = photos.data.watch_focus();
		cx.spawn(async move |this, cx| {
			while focus.changed().await.is_ok() {
				let alive = this.update(cx, |photos, cx| {
					photos.focus = photos.data.focus();
					photos.adopt_folders(cx);
					cx.notify();
				});
				if alive.is_err() {
					break;
				}
			}
		})
		.detach();

		let mut tags = photos.data.watch_tags();
		cx.spawn(async move |this, cx| {
			while tags.changed().await.is_ok() {
				if this.update(cx, |photos, cx| photos.adopt_tags(cx)).is_err() {
					break;
				}
			}
		})
		.detach();

		photos
	}

	/// Take whatever listings the plane has opened. The last one wins: a fast
	/// walk through several directories leaves only the one now in focus.
	fn adopt_folders(&mut self, cx: &mut Context<Self>) {
		if self.bench {
			return;
		}
		while let Some(open) = self.data.take_folder() {
			let source = PvcacheSource::new(
				open.records.len() as u32,
				open.feed_rx,
				open.completions_rx,
				open.visible,
			);
			let cells = Cells::new(open.records, open.paths, open.tags);
			self.grid.update(cx, |grid, cx| {
				grid.set_source(Box::new(source) as Box<dyn TileSource>, cells, cx);
			});
			self.folder_changes = Some(Self::follow_changes(open.changes, cx));
			self.title = Some(open.title);
		}
	}

	/// Apply a listing's later pages and tag changes to the grid as they
	/// arrive, a burst at a time and in the order they happened.
	fn follow_changes(
		mut changes: mpsc::UnboundedReceiver<FolderChange>,
		cx: &mut Context<Self>,
	) -> Task<()> {
		cx.spawn(async move |this, cx| {
			while let Some(first) = changes.recv().await {
				let mut batch = vec![first];
				while let Ok(more) = changes.try_recv() {
					batch.push(more);
				}
				let applied = this.update(cx, |photos, cx| {
					photos.grid.update(cx, |grid, cx| {
						for change in batch {
							match change {
								FolderChange::Appended {
									records,
									paths,
									tags,
								} => grid.append_cells(records, paths, tags, cx),
								FolderChange::Tags(changes) => {
									for change in &changes {
										grid.set_record_tags(change.record, &change.tags);
									}
								}
							}
						}
						cx.notify();
					});
				});
				if applied.is_err() {
					break;
				}
			}
		})
	}

	/// A new tag snapshot: tag colors to the grid, and a new failure to the bar.
	fn adopt_tags(&mut self, cx: &mut Context<Self>) {
		let snapshot = self.data.tags();
		let colors = snapshot
			.tags
			.iter()
			.map(|tag| (tag.id, rgb(tag.color).into()))
			.collect();
		self.grid
			.update(cx, |grid, cx| grid.set_tag_colors(colors, cx));
		if let Some(notice) = snapshot
			.notice
			.as_ref()
			.filter(|notice| notice.seq > self.notice_seen)
		{
			self.notice_seen = notice.seq;
			self.show_notice(notice.message.clone().into(), cx);
		}
		self.tags = snapshot;
		cx.notify();
	}

	fn show_notice(&mut self, message: SharedString, cx: &mut Context<Self>) {
		self.notice = Some(message);
		let seq = self.notice_seen;
		cx.spawn(async move |this, cx| {
			cx.background_executor().timer(NOTICE_DURATION).await;
			let _ = this.update(cx, |photos, cx| {
				if photos.notice_seen == seq {
					photos.notice = None;
					cx.notify();
				}
			});
		})
		.detach();
	}

	fn toggle_following(&mut self, cx: &mut Context<Self>) {
		self.data.set_following(!self.focus.following);
		cx.notify();
	}

	fn set_tag_mode(&mut self, on: bool, cx: &mut Context<Self>) {
		self.tag_mode = on;
		self.notice = None;
		if on {
			self.data.refresh_tags();
		}
		cx.notify();
	}

	/// The tags number keys toggle, in key order.
	fn palette(&self) -> &[data::TagInfo] {
		let tags = &self.tags.tags;
		&tags[..tags.len().min(PALETTE_SIZE)]
	}

	/// Toggle palette slot `slot`'s tag on the selection: off every selected
	/// photo when all of them carry it, onto all of them otherwise.
	fn toggle_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
		let Some(tag) = self.palette().get(slot) else {
			return;
		};
		let grid = self.grid.read(cx);
		let records = grid.selected_records();
		if records.is_empty() {
			return;
		}
		let apply = !grid.selection_carries(tag.id);
		self.data.tag(tag.id, records, apply);
		self.notice = None;
		cx.notify();
	}

	fn enter_tag_mode(&mut self, _: &EnterTagMode, _: &mut Window, cx: &mut Context<Self>) {
		if !self.tag_mode {
			self.set_tag_mode(true, cx);
		}
	}

	fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
		if let Some(quick_look) = self.open_preview() {
			quick_look.close(cx);
		} else if self.tag_mode {
			self.set_tag_mode(false, cx);
		} else {
			self.grid.update(cx, |grid, cx| {
				grid.clear_selection(cx);
			});
		}
	}

	/// The preview panel, when it is open.
	fn open_preview(&self) -> Option<&QuickLook> {
		self.quick_look
			.as_ref()
			.filter(|quick_look| quick_look.is_open())
	}

	/// Space: show the original of the photo under the cursor, or put the
	/// preview away.
	fn toggle_preview(&mut self, _: &TogglePreview, _: &mut Window, cx: &mut Context<Self>) {
		let Some(quick_look) = &self.quick_look else {
			return;
		};
		if quick_look.is_open() {
			quick_look.close(cx);
		} else if let Some(path) = self.grid.read(cx).cursor_path().cloned() {
			self.preview(path, cx);
		}
	}

	/// Keep an open preview on the photo under the cursor as it moves, and put
	/// it away once nothing is selected.
	fn follow_preview(&mut self, cx: &mut Context<Self>) {
		let Some(quick_look) = self.open_preview() else {
			return;
		};
		match self.grid.read(cx).cursor_path().cloned() {
			Some(path) => self.preview(path, cx),
			None => {
				quick_look.close(cx);
				// An answer still on its way must not open it again.
				self.preview_ask += 1;
			}
		}
	}

	/// Show the file at `path` in the preview panel, at the path on this
	/// machine the data plane finds for it: its own on this device, and inside
	/// the mounted share for one on another, which the panel then streams.
	/// Only the latest ask is shown, so a slow answer for a photo the cursor
	/// has left does not pull the panel back to it.
	fn preview(&mut self, path: SdPath, cx: &mut Context<Self>) {
		self.preview_ask += 1;
		let ask = self.preview_ask;
		let data = self.data.clone();
		cx.spawn(async move |this, cx| {
			let found = data.local_path(path).await;
			let _ = this.update(cx, |photos, cx| {
				if photos.preview_ask != ask {
					return;
				}
				match found {
					Ok(local) => {
						if let Some(quick_look) = &photos.quick_look {
							quick_look.show(&local, cx);
						}
					}
					Err(error) => {
						photos.show_notice(format!("Could not preview: {error:#}").into(), cx)
					}
				}
			});
		})
		.detach();
	}

	fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
		self.grid.update(cx, |grid, cx| grid.select_all(cx));
	}

	fn toggle_tag(&mut self, action: &ToggleTag, _: &mut Window, cx: &mut Context<Self>) {
		if self.tag_mode {
			self.toggle_slot(action.slot, cx);
		}
	}

	fn move_selection(&mut self, direction: Direction, cx: &mut Context<Self>) {
		self.grid
			.update(cx, |grid, cx| grid.move_selection(direction, cx));
	}

	fn render_tag_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let grid = self.grid.read(cx);
		let slots = self
			.palette()
			.iter()
			.map(|tag| Slot {
				active: grid.selection_carries(tag.id),
				tag: tag.clone(),
			})
			.collect();
		let photos = cx.entity();
		let done = photos.clone();
		TagBar::new(
			slots,
			grid.selection_len(),
			self.notice.clone(),
			move |slot, _, cx| photos.update(cx, |photos, cx| photos.toggle_slot(slot, cx)),
			move |_, cx| done.update(cx, |photos, cx| photos.set_tag_mode(false, cx)),
		)
	}

	fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let photos = cx.entity();
		let following = self.focus.following;
		let title = self.title.clone();
		div()
			.h(px(TOOLBAR_HEIGHT))
			.flex_shrink_0()
			.flex()
			.items_center()
			.gap(px(8.0))
			.px(px(12.0))
			.border_b_1()
			.border_color(theme.app_line)
			.window_control_area(WindowControlArea::Drag)
			// Room for the traffic lights, which float over a transparent
			// titlebar rather than reserving space of their own.
			.child(div().w(px(64.0)).flex_shrink_0())
			.child(
				div()
					.flex_1()
					.min_w(px(0.0))
					.flex()
					.justify_center()
					.text_size(theme.text_sm)
					.text_color(theme.ink)
					.when_some(title, |element, title| {
						element.child(div().truncate().child(title))
					}),
			)
			.child(
				Button::new("tag-mode", "Tags")
					.variant(if self.tag_mode {
						ButtonVariant::Accent
					} else {
						ButtonVariant::Default
					})
					.on_click({
						let photos = photos.clone();
						move |_, _, cx| {
							photos.update(cx, |photos, cx| {
								let on = !photos.tag_mode;
								photos.set_tag_mode(on, cx);
							});
						}
					}),
			)
			.child(
				Button::new("follow", if following { "Following" } else { "Follow" })
					.variant(if following {
						ButtonVariant::Accent
					} else {
						ButtonVariant::Default
					})
					.on_click(move |_, _, cx| {
						photos.update(cx, |photos, cx| photos.toggle_following(cx));
					}),
			)
	}

	/// Shown until a folder arrives. Following is the normal way one does, so
	/// the copy says where to go rather than offering a picker this window
	/// does not have.
	fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let message = match &self.focus.folder {
			FolderState::Loading => "Loading".to_string(),
			FolderState::Empty if self.focus.searching => "No photos match the search".to_string(),
			FolderState::Empty => "No photos in this folder".to_string(),
			FolderState::NoSource => "This folder is not in a tracked source".to_string(),
			FolderState::NoLibrary => "No library open in Spacedrive".to_string(),
			FolderState::Error(error) => error.clone(),
			FolderState::Idle | FolderState::Ready(_) if !self.focus.following => {
				"Following is off".to_string()
			}
			FolderState::Idle | FolderState::Ready(_) => "Open a folder in Spacedrive".to_string(),
		};
		div()
			.size_full()
			.flex()
			.items_center()
			.justify_center()
			.text_size(theme.text_sm)
			.text_color(theme.ink_faint)
			.child(message)
	}

	fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let (dot, label) = if self.focus.online {
			(theme.status_success, "Daemon connected".to_string())
		} else {
			(theme.ink_faint, "Daemon offline".to_string())
		};
		let count = match self.focus.folder {
			FolderState::Ready(count) => count,
			_ => 0,
		};
		let selected = self.grid.read(cx).selection_len();
		div()
			.h(px(STATUS_BAR_HEIGHT))
			.flex_shrink_0()
			.flex()
			.items_center()
			.gap(px(6.0))
			.px(px(10.0))
			.border_t_1()
			.border_color(theme.app_line)
			.bg(theme.sidebar)
			.text_size(theme.text_xs)
			.text_color(theme.ink_faint)
			.child(div().size(px(7.0)).rounded_full().bg(dot))
			.child(label)
			.child(match count {
				1 => "1 photo".to_string(),
				count => format!("{count} photos"),
			})
			.when(selected > 0, |bar| {
				bar.child(format!("{selected} selected"))
			})
			.child(div().flex_1())
			.child(self.data.socket_addr().to_string())
	}
}

impl Render for Photos {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let showing = self.title.is_some() || self.bench;
		div()
			.size_full()
			.flex()
			.flex_col()
			.bg(theme.app)
			.text_color(theme.ink)
			.text_size(theme.text_sm)
			.track_focus(&self.focus_handle)
			.key_context(KEY_CONTEXT)
			.on_action(cx.listener(Self::enter_tag_mode))
			.on_action(cx.listener(Self::cancel))
			.on_action(cx.listener(Self::toggle_preview))
			.on_action(cx.listener(Self::select_all))
			.on_action(cx.listener(Self::toggle_tag))
			.on_action(
				cx.listener(|photos, _: &MoveLeft, _, cx| {
					photos.move_selection(Direction::Left, cx)
				}),
			)
			.on_action(cx.listener(|photos, _: &MoveRight, _, cx| {
				photos.move_selection(Direction::Right, cx)
			}))
			.on_action(
				cx.listener(|photos, _: &MoveUp, _, cx| photos.move_selection(Direction::Up, cx)),
			)
			.on_action(
				cx.listener(|photos, _: &MoveDown, _, cx| {
					photos.move_selection(Direction::Down, cx)
				}),
			)
			.child(self.render_toolbar(cx))
			.child(
				div()
					.relative()
					.flex_1()
					.min_h(px(0.0))
					.child(if showing {
						self.grid.clone().into_any_element()
					} else {
						self.render_empty_state(cx).into_any_element()
					})
					.when(self.tag_mode, |area| area.child(self.render_tag_bar(cx))),
			)
			.child(self.render_status_bar(cx))
	}
}
