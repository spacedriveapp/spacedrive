//! Spacedrive's native GPUI app.
//!
//! The shell: a hidden-titlebar macOS window with the SpaceUI dark palette —
//! left sidebar, a main pane switching between the tile grid and the explorer
//! list, and a status row reporting daemon liveness. All daemon traffic runs
//! through the async data plane in [`data`]; the frame loop only ever reads
//! snapshots.
//!
//! With a daemon running (`SD_NATIVE_INSTANCE` selects a named instance) the
//! sidebar shows the daemon's libraries and volumes, and clicking a volume or
//! location lists the real directory over `files.directory_listing`, kept
//! fresh by path-scoped event subscriptions. `SD_NATIVE_OPEN=<dir>` opens a
//! directory in the list pane at launch. Without a daemon the app still runs:
//! the sidebar keeps its static rows, the status dot reads offline, and the
//! grid works in its synthetic and folder-demo modes.
//!
//! With a folder argument (`spacedrive-native ~/Pictures`, or
//! `SD_NATIVE_FOLDER`) the grid shows real thumbnails: an in-process demo
//! bake fills `thumbs.pvcache` while the grid reads the same file through
//! the cross-process reader contract. Without one, the grid runs on
//! synthetic tiles. `SD_GRID_BENCH=1` runs the scripted flywheel benchmark
//! over either source (stats to stderr, quits when done); `SD_GRID_CELLS`
//! overrides the synthetic cell count.

mod data;
mod demo;
mod grid;
mod list;
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

use crate::data::{DataHandle, SidebarSnapshot};
use crate::grid::GridView;
use crate::list::ListView;
use crate::source::{PvcacheSource, SyntheticSource, TileSource, VisibleRange};
use crate::theme::{ActiveTheme as _, Theme};
use crate::ui::{Button, ButtonVariant, CircleButton, SidebarItem, SidebarSectionLabel};

const SIDEBAR_WIDTH: f32 = 220.0;
const TOOLBAR_HEIGHT: f32 = 44.0;
const STATUS_BAR_HEIGHT: f32 = 26.0;

fn main() {
	let (cells, bench) = grid::config_from_env();
	let source = build_source(cells);
	let data = data::spawn(std::env::var("SD_NATIVE_INSTANCE").ok());

	application().run(move |cx: &mut App| {
		gpui_component::init(cx);
		Theme::init(cx);

		let bounds = Bounds::centered(None, size(px(1280.0), px(860.0)), cx);
		let options = WindowOptions {
			window_bounds: Some(WindowBounds::Windowed(bounds)),
			titlebar: Some(TitlebarOptions {
				title: Some("Spacedrive".into()),
				appears_transparent: true,
				traffic_light_position: Some(point(px(12.0), px(12.0))),
			}),
			..Default::default()
		};

		cx.open_window(options, |window, cx| {
			let grid = cx.new(|cx| GridView::new(source, bench, cx));
			let list = cx.new(|cx| ListView::new(data.clone(), cx));
			let workspace = cx.new(|cx| Workspace::new(grid, list, data.clone(), cx));
			cx.new(|cx| Root::new(workspace, window, cx))
		})
		.expect("failed to open window");
		cx.activate(true);
	});
}

/// Choose the grid's tile source: real thumbnails over pvcache when a folder
/// is given, synthetic tiles otherwise.
fn build_source(synthetic_cells: u32) -> Box<dyn TileSource> {
	let Some(folder) = demo::folder_from_env() else {
		return Box::new(SyntheticSource::new(synthetic_cells));
	};
	if !folder.is_dir() {
		eprintln!("spacedrive-native: {} is not a directory", folder.display());
		std::process::exit(1);
	}
	let cache_path = demo::cache_path();
	// Seeded with a first-screenful estimate so the fill favors the top of
	// the grid before the first paint publishes the real range.
	let visible = VisibleRange::new(0, 512);
	let fill = demo::spawn(
		folder,
		cache_path.clone(),
		visible.clone(),
		demo::max_files_from_env(),
	);
	Box::new(PvcacheSource::new(
		cache_path,
		fill.entries_rx,
		fill.completions_rx,
		visible,
	))
}

/// Which view fills the main pane.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
	Grid,
	List,
}

struct Workspace {
	grid: Entity<GridView>,
	list: Entity<ListView>,
	data: DataHandle,
	pane: Pane,
	sidebar: Arc<SidebarSnapshot>,
	/// The listing target, mirrored here so the sidebar can highlight the
	/// active volume or folder.
	active_target: Option<PathBuf>,
}

impl Workspace {
	fn new(
		grid: Entity<GridView>,
		list: Entity<ListView>,
		data: DataHandle,
		cx: &mut Context<Self>,
	) -> Self {
		// Wake on data-plane snapshot changes; the channels are tokio watches,
		// which await fine on gpui's executor.
		let mut sidebar = data.watch_sidebar();
		cx.spawn(async move |this, cx| {
			while sidebar.changed().await.is_ok() {
				let alive = this.update(cx, |workspace, cx| {
					workspace.sidebar = workspace.data.sidebar();
					cx.notify();
				});
				if alive.is_err() {
					break;
				}
			}
		})
		.detach();
		let mut listing = data.watch_listing();
		cx.spawn(async move |this, cx| {
			while listing.changed().await.is_ok() {
				let alive = this.update(cx, |workspace, cx| {
					let target = workspace.data.listing().target.clone();
					if workspace.active_target != target {
						workspace.active_target = target;
						cx.notify();
					}
				});
				if alive.is_err() {
					break;
				}
			}
		})
		.detach();

		// A startup directory opens straight into the list pane.
		let initial_target = std::env::var_os("SD_NATIVE_OPEN").map(PathBuf::from);
		let pane = match &initial_target {
			Some(path) => {
				data.open_directory(path.clone());
				Pane::List
			}
			None => Pane::Grid,
		};

		Workspace {
			grid,
			list,
			sidebar: data.sidebar(),
			active_target: data.listing().target.clone(),
			data,
			pane,
		}
	}

	/// Load a directory into the list pane and bring it to the front.
	fn open_target(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
		self.data.open_directory(path);
		self.show_pane(Pane::List, window, cx);
	}

	fn show_pane(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
		self.pane = pane;
		if pane == Pane::List {
			let focus = self.list.read(cx).focus_handle();
			window.focus(&focus, cx);
		}
		cx.notify();
	}

	/// A sidebar row that opens `path` in the list pane.
	fn location_item(
		&self,
		id: (&'static str, usize),
		label: String,
		path: PathBuf,
		cx: &mut Context<Self>,
	) -> SidebarItem {
		let workspace = cx.entity();
		let selected = self.active_target.as_deref() == Some(&path);
		SidebarItem::new(id, label)
			.selected(selected)
			.on_click(move |_, window, cx| {
				let path = path.clone();
				workspace.update(cx, |workspace, cx| {
					workspace.open_target(path, window, cx);
				});
			})
	}

	fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let snapshot = self.sidebar.clone();
		let mut nav = div()
			.flex_1()
			.px(px(8.0))
			.flex()
			.flex_col()
			.gap(px(2.0))
			.overflow_hidden();

		if snapshot.online {
			if !snapshot.libraries.is_empty() {
				nav = nav.child(SidebarSectionLabel::new("Libraries"));
				for (index, library) in snapshot.libraries.iter().enumerate() {
					let data = self.data.clone();
					let id = library.id;
					nav = nav.child(
						SidebarItem::new(("library", index), library.name.clone())
							.selected(snapshot.current_library == Some(id))
							.on_click(move |_, _, _| data.select_library(id)),
					);
				}
			}
			nav = nav.child(SidebarSectionLabel::new("Volumes"));
			if snapshot.volumes.is_empty() {
				nav = nav.child(
					SidebarItem::new(
						"volumes-empty",
						if snapshot.current_library.is_some() {
							"No volumes"
						} else {
							"No library yet"
						},
					),
				);
			}
			for (index, volume) in snapshot.volumes.iter().enumerate() {
				nav = nav.child(
					self.location_item(
						("volume", index),
						volume.name.clone(),
						volume.mount_point.clone(),
						cx,
					)
					.detail(volume.capacity_label.clone()),
				);
			}
			nav = nav.child(SidebarSectionLabel::new("Locations"));
			if let Some(home) = std::env::home_dir() {
				nav = nav
					.child(self.location_item(("location", 0), "Home".into(), home.clone(), cx))
					.child(self.location_item(
						("location", 1),
						"Downloads".into(),
						home.join("Downloads"),
						cx,
					));
			}
		} else {
			// Offline placeholder: the scaffold's static rows.
			nav = nav
				.child(SidebarSectionLabel::new("Library"))
				.child(SidebarItem::new("nav-overview", "Overview").selected(true))
				.child(SidebarItem::new("nav-recents", "Recents"))
				.child(SidebarItem::new("nav-photos", "Photos"))
				.child(SidebarSectionLabel::new("Locations"))
				.child(SidebarItem::new("nav-home", "Home"))
				.child(SidebarItem::new("nav-downloads", "Downloads"));
		}

		let theme = cx.theme();
		div()
			.w(px(SIDEBAR_WIDTH))
			.flex_shrink_0()
			.flex()
			.flex_col()
			.bg(theme.sidebar)
			.border_r_1()
			.border_color(theme.sidebar_divider)
			// Traffic-light strip: empty chrome that drags the window.
			.child(
				div()
					.h(px(TOOLBAR_HEIGHT))
					.flex_shrink_0()
					.window_control_area(WindowControlArea::Drag),
			)
			.child(nav)
	}

	fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let grid_workspace = cx.entity();
		let list_workspace = cx.entity();
		let target_label = self
			.active_target
			.as_ref()
			.map(|path| path.display().to_string());
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
			.child(CircleButton::new("nav-back", "‹"))
			.child(CircleButton::new("nav-forward", "›"))
			.child(
				div()
					.flex_1()
					.min_w(px(0.0))
					.flex()
					.justify_center()
					.text_size(theme.text_xs)
					.text_color(theme.ink_faint)
					.when_some(target_label, |element, label| {
						element.child(div().truncate().child(label))
					}),
			)
			.child(
				Button::new("pane-grid", "Grid")
					.variant(if self.pane == Pane::Grid {
						ButtonVariant::Gray
					} else {
						ButtonVariant::Default
					})
					.on_click(move |_, window, cx| {
						grid_workspace.update(cx, |workspace, cx| {
							workspace.show_pane(Pane::Grid, window, cx);
						});
					}),
			)
			.child(
				Button::new("pane-list", "List")
					.variant(if self.pane == Pane::List {
						ButtonVariant::Gray
					} else {
						ButtonVariant::Default
					})
					.on_click(move |_, window, cx| {
						list_workspace.update(cx, |workspace, cx| {
							workspace.show_pane(Pane::List, window, cx);
						});
					}),
			)
			.child(CircleButton::new("new-item", "+").accent(true))
	}

	fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let (dot, label) = if self.sidebar.online {
			(theme.status_success, "Daemon connected")
		} else {
			(theme.ink_faint, "Daemon offline")
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
			.child(div().flex_1())
			.child(self.data.socket_addr().to_string())
	}
}

impl Render for Workspace {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let main: gpui::AnyElement = match self.pane {
			Pane::Grid => self.grid.clone().into_any_element(),
			Pane::List => self.list.clone().into_any_element(),
		};
		div()
			.size_full()
			.flex()
			.flex_col()
			.bg(theme.app)
			.text_color(theme.ink)
			.text_size(theme.text_sm)
			.child(
				div()
					.flex_1()
					.min_h(px(0.0))
					.flex()
					.child(self.render_sidebar(cx))
					.child(
						div()
							.flex_1()
							.min_w(px(0.0))
							.flex()
							.flex_col()
							.child(self.render_toolbar(cx))
							.child(div().flex_1().min_h(px(0.0)).child(main)),
					),
			)
			.child(self.render_status_bar(cx))
	}
}
