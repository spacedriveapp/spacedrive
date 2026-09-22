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

mod data;
mod grid;
mod source;
mod theme;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
	div, point, px, size, App, AppContext as _, Bounds, Context, Entity, InteractiveElement as _,
	IntoElement, ParentElement as _, Render, Styled as _, TitlebarOptions, Window, WindowBounds,
	WindowControlArea, WindowOptions,
};
use gpui_component::Root;
use gpui_platform::application;

use crate::data::{DataHandle, FocusSnapshot, FolderState};
use crate::grid::GridView;
use crate::source::{EmptySource, PvcacheSource, SyntheticSource, TileSource};
use crate::theme::{ActiveTheme as _, Theme};
use crate::ui::{Button, ButtonVariant};

const TOOLBAR_HEIGHT: f32 = 44.0;
const STATUS_BAR_HEIGHT: f32 = 26.0;

fn main() {
	let (cells, bench) = grid::config_from_env();
	let data = data::spawn(
		std::env::var("SD_NATIVE_INSTANCE").ok(),
		data::focus_group_from_env(),
	);

	application().run(move |cx: &mut App| {
		gpui_component::init(cx);
		Theme::init(cx);

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
			let photos = cx.new(|cx| Photos::new(grid, data.clone(), bench, cx));
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
	/// The folder the grid is showing, which lags the focus by one retarget.
	folder: Option<PathBuf>,
	/// The benchmark owns the grid's source; adopting a folder would pull it
	/// away mid-run, so the window ignores focus for the duration.
	bench: bool,
}

impl Photos {
	fn new(grid: Entity<GridView>, data: DataHandle, bench: bool, cx: &mut Context<Self>) -> Self {
		let photos = Photos {
			grid,
			focus: data.focus(),
			data,
			folder: None,
			bench,
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

		photos
	}

	/// Take whatever folders the plane has opened. The last one wins: a fast
	/// walk through several directories leaves only the one now in focus.
	fn adopt_folders(&mut self, cx: &mut Context<Self>) {
		if self.bench {
			return;
		}
		while let Some(open) = self.data.take_folder() {
			let source = PvcacheSource::new(
				open.cache_path,
				open.len,
				open.entries_rx,
				open.completions_rx,
				open.visible,
			);
			self.grid.update(cx, |grid, cx| {
				grid.set_source(Box::new(source) as Box<dyn TileSource>, cx);
			});
			self.folder = Some(open.path);
		}
	}

	fn toggle_following(&mut self, cx: &mut Context<Self>) {
		self.data.set_following(!self.focus.following);
		cx.notify();
	}

	fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let photos = cx.entity();
		let following = self.focus.following;
		let title = self
			.folder
			.as_ref()
			.and_then(|folder| folder.file_name())
			.map(|name| name.to_string_lossy().into_owned())
			.or_else(|| self.folder.as_ref().map(|f| f.display().to_string()));
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
			FolderState::Empty => "No photos in this folder".to_string(),
			FolderState::NoSource => "This folder is not on an indexed drive".to_string(),
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
			.child(div().flex_1())
			.child(self.data.socket_addr().to_string())
	}
}

impl Render for Photos {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.size_full()
			.flex()
			.flex_col()
			.bg(theme.app)
			.text_color(theme.ink)
			.text_size(theme.text_sm)
			.child(self.render_toolbar(cx))
			.child(
				div()
					.flex_1()
					.min_h(px(0.0))
					.child(if self.folder.is_some() || self.bench {
						self.grid.clone().into_any_element()
					} else {
						self.render_empty_state(cx).into_any_element()
					}),
			)
			.child(self.render_status_bar(cx))
	}
}
