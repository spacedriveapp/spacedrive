//! The tile grid: a custom gpui Element that paints through gpui's own sprite
//! pipeline. Measured at 129k cells scrolling at a locked 120Hz (p50 frame
//! 8.33ms, ~0.3ms of paint CPU) on Apple Silicon at 2x.
//!
//! Each baked cell is an `Arc<RenderImage>` living in gpui's Metal sprite
//! atlas (BGRA8 pages, etagere-allocated, freed on `drop_image`); the element
//! culls to the viewport and paints only visible cells, so the total cell
//! count never touches the per-frame path. Pixels come from a
//! [`TileSource`] — synthetic tiles for the benchmark, real thumbnails read
//! out of `thumbs.pvcache` when a folder is given.
//!
//! The scripted flywheel benchmark from the spike is kept behind
//! `SD_GRID_BENCH=1` (synthetic cell count via `SD_GRID_CELLS`); it prints
//! frame stats to stderr and quits when the script completes.
//!
//! Beyond pixels the grid knows each cell's record and the tags it carries
//! ([`Cells`]), and which cells are selected. Selected cells are tinted,
//! ringed, and badged; tagged cells carry colored dots in their corner.

mod selection;
mod stats;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
	fill, point, px, quad, relative, rgb, size, transparent_black, App, BorderStyle, Bounds,
	ContentMask, Context, Corners, DispatchPhase, Edges, Element, ElementId, Entity,
	GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
	MouseButton, MouseDownEvent, PathBuilder, PinchEvent, Pixels, Point, Render, RenderImage,
	ScrollDelta, ScrollWheelEvent, SharedString, Style, TextAlign, TextRun, TouchPhase, Window,
};
use image::Frame;
use smallvec::smallvec;
use uuid::Uuid;

use crate::data::DEFAULT_TAG_COLOR;
use crate::source::TileSource;
use crate::theme::ActiveTheme as _;
pub use selection::Direction;
use selection::{step, Selection};
use stats::{resident_mb, FrameStats};

/// The largest rectangle of `aspect` (width / height) that fits inside `cell`,
/// centered. A tile is stored at the image's own proportions, so the cell holds
/// it rather than the other way round: the grid shows whole thumbnails and the
/// leftover space on the short axis stays background.
fn fit_within(cell: Bounds<Pixels>, aspect: f32) -> Bounds<Pixels> {
	let width = f32::from(cell.size.width);
	let height = f32::from(cell.size.height);
	let (w, h) = if aspect >= 1.0 {
		(width, width / aspect)
	} else {
		(height * aspect, height)
	};
	let (w, h) = (w.min(width).max(1.0), h.min(height).max(1.0));
	Bounds::new(
		point(
			cell.origin.x + px((width - w) / 2.0),
			cell.origin.y + px((height - h) / 2.0),
		),
		size(px(w), px(h)),
	)
}

/// Logical-point tile size and gap, matching the reference grid. `TARGET_CELL`
/// is the density the grid opens at; pinch zoom moves it within the bounds
/// below, from many small tiles up to a few large ones.
const TARGET_CELL: f32 = 128.0;
const MIN_CELL: f32 = 64.0;
/// The baked tile is `TILE` physical pixels on its long edge, so a cell drawn
/// larger than that upscales. At 2x this ceiling reaches it.
const MAX_CELL: f32 = 384.0;
const GAP: f32 = 2.0;
/// Baked tiles held resident (CPU bytes + sprite-atlas space). Visible set is
/// ~100 cells, so this gives a healthy scroll-back margin while bounding
/// memory: 512 x 256KB = 128MB CPU-side + the same in atlas pages.
const CACHE_CAP: usize = 512;
/// Finished bakes accepted per frame (the reference grid uses 1024).
const DRAIN_PER_FRAME: usize = 1024;

/// Default synthetic cell count, matching the reference renderer's proven scale.
const DEFAULT_CELLS: u32 = 129_000;

/// Selection chrome, in logical points: the inset ring's width, the tint's
/// opacity, and the check badge in the top-left corner.
const RING: f32 = 2.0;
const TINT: f32 = 0.1;
const BADGE: f32 = 16.0;
const BADGE_INSET: f32 = 4.0;

/// Tag dots, in logical points: dot size and spacing, how many show before the
/// rest become a count, and the dark pill they sit on so they read over any
/// photo.
const DOT: f32 = 7.0;
const DOT_GAP: f32 = 3.0;
const DOTS_SHOWN: usize = 3;
const PILL_PAD: f32 = 4.0;
const PILL_INSET: f32 = 5.0;
const COUNT_TEXT: f32 = 10.0;

/// What the grid knows of each cell beyond its pixels: the record it shows and
/// the tags that record carries, both by grid index.
#[derive(Default)]
pub struct Cells {
	records: Vec<Uuid>,
	tags: Vec<Vec<Uuid>>,
	index_by_record: HashMap<Uuid, u32>,
}

impl Cells {
	pub fn new(records: Vec<Uuid>, tags: Vec<Vec<Uuid>>) -> Self {
		let index_by_record = records
			.iter()
			.enumerate()
			.map(|(index, record)| (*record, index as u32))
			.collect();
		Cells {
			records,
			tags,
			index_by_record,
		}
	}

	/// Take `tags` as what `record` carries, when it is one of these cells.
	fn set_tags(&mut self, record: Uuid, tags: &[Uuid]) {
		let Some(&index) = self.index_by_record.get(&record) else {
			return;
		};
		if let Some(cell) = self.tags.get_mut(index as usize) {
			cell.clear();
			cell.extend_from_slice(tags);
		}
	}

	fn tags(&self, index: u32) -> &[Uuid] {
		self.tags.get(index as usize).map_or(&[], Vec::as_slice)
	}

	fn record(&self, index: u32) -> Option<Uuid> {
		self.records.get(index as usize).copied()
	}
}

/// Benchmark configuration from the environment: the synthetic cell count
/// when one is asked for, and whether the scripted benchmark runs.
/// `SD_GRID_BENCH=1` enables the benchmark, `SD_GRID_CELLS` sets the count.
///
/// Synthetic tiles are for measurement, never for a window with no folder:
/// with neither variable set there is no count, and the grid starts empty.
pub fn config_from_env() -> (Option<u32>, bool) {
	let explicit = std::env::var("SD_GRID_CELLS")
		.ok()
		.and_then(|v| v.parse().ok());
	let bench = std::env::var("SD_GRID_BENCH").map_or(false, |v| v == "1");
	(explicit.or(bench.then_some(DEFAULT_CELLS)), bench)
}

/// The scripted-benchmark auto-scroll: a flywheel pass over the whole range so
/// the measurement doesn't need a human on the trackpad.
struct AutoScroll {
	/// (label, seconds, velocity px/s). Velocity is resolved against the live
	/// max scroll when the phase starts, so "traverse" phases really cover the
	/// full range regardless of window size.
	phases: Vec<(&'static str, f32, PhaseVel)>,
	phase: usize,
	elapsed_in_phase: f32,
	velocity: f32,
	last_tick: Instant,
	started: bool,
}

enum PhaseVel {
	/// Fixed logical px/s.
	Fixed(f32),
	/// Cover this fraction of the full content height over the phase duration.
	/// Sign gives direction.
	Traverse(f32),
}

impl AutoScroll {
	fn new() -> Self {
		AutoScroll {
			phases: vec![
				("warmup", 2.0, PhaseVel::Fixed(1200.0)),
				("fast-scroll", 6.0, PhaseVel::Fixed(6000.0)),
				("flywheel-down", 12.0, PhaseVel::Traverse(1.0)),
				("dwell", 1.0, PhaseVel::Fixed(0.0)),
				("flywheel-up", 12.0, PhaseVel::Traverse(-1.0)),
				("settle", 2.0, PhaseVel::Fixed(3000.0)),
			],
			phase: 0,
			elapsed_in_phase: 0.0,
			velocity: 0.0,
			last_tick: Instant::now(),
			started: false,
		}
	}
}

/// A live pinch. Captured at `Started` and held until the gesture settles: the
/// grid keeps `cols0` columns and scales `pitch0` by `g`, anchored so the
/// content point under the fingers stays under them.
struct PinchState {
	/// Accumulated scale, 1.0 at gesture start.
	g: f32,
	/// Columns held for the duration of the gesture.
	cols0: u32,
	/// Tile pitch (cell + gap) at gesture start.
	pitch0: f32,
	/// Focal point in grid-local points.
	fx: f32,
	fy: f32,
	/// Focal point in pitch units, which is scale-independent, so the anchor
	/// solves to a scroll offset at any scale.
	ux: f32,
	uy: f32,
	/// The cell under the focal point and its sub-cell vertical fraction, so
	/// the settle keeps that item fixed across the column-count reflow.
	focal_item: u32,
	frac_y: f32,
}

pub struct GridView {
	source: Box<dyn TileSource>,
	/// The record and tags behind each of the source's cells.
	cells: Cells,
	/// A source and its cells handed over between frames. The swap happens
	/// inside paint, where the window handle needed to release the old atlas
	/// tiles exists.
	next: Option<(Box<dyn TileSource>, Cells)>,
	selection: Selection,
	/// Tag colors by tag id, for the dots on tagged cells.
	tag_colors: HashMap<Uuid, Hsla>,
	/// Width and height at the last paint, which keyboard movement lays out
	/// against.
	viewport: Option<(f32, f32)>,
	scroll_y: f32,
	/// Horizontal offset, nonzero only mid-pinch: the column count is held for
	/// the gesture, so the grid overflows sideways as its tiles grow.
	scroll_x: f32,
	/// Live tile size in logical points, driven by pinch zoom. The steady-state
	/// layout fills the row width at this density.
	target_cell: f32,
	/// The in-flight pinch, if one is live.
	pinch: Option<PinchState>,
	/// Resident tiles: the uploaded image, its aspect (width / height, so the
	/// cell can hold the shape without re-reading the buffer), and the tick it
	/// was last painted on for LRU eviction.
	cache: HashMap<u32, (Arc<RenderImage>, f32, u64)>,
	tick: u64,
	stats: FrameStats,
	auto: Option<AutoScroll>,
	finished: bool,
	frame_index: u64,
}

impl GridView {
	pub fn new(source: Box<dyn TileSource>, bench: bool, cx: &mut Context<Self>) -> Self {
		// `request_animation_frame` arms only the next frame; a frame the
		// platform drops (occluded window, sleeping display) ends the chain
		// with nothing left to re-arm it. This watchdog re-notifies at a low
		// rate whenever the benchmark script or outstanding tiles still need
		// frames, and stays silent while the grid is idle.
		cx.spawn(async move |this, cx| loop {
			cx.background_executor()
				.timer(std::time::Duration::from_millis(100))
				.await;
			let alive = this.update(cx, |grid, cx| {
				let script_running = grid.auto.is_some() && !grid.finished;
				if script_running || grid.source.has_pending() {
					cx.notify();
				}
			});
			if alive.is_err() {
				break;
			}
		})
		.detach();

		GridView {
			source,
			cells: Cells::default(),
			next: None,
			selection: Selection::default(),
			tag_colors: HashMap::new(),
			viewport: None,
			scroll_y: 0.0,
			scroll_x: 0.0,
			target_cell: TARGET_CELL,
			pinch: None,
			cache: HashMap::new(),
			tick: 0,
			stats: FrameStats::new(),
			auto: bench.then(AutoScroll::new),
			finished: false,
			frame_index: 0,
		}
	}

	/// Point the grid at a different set of cells. The swap lands on the next
	/// frame, which is where the old tiles can be released, and the selection
	/// goes with the old cells.
	pub fn set_source(
		&mut self,
		source: Box<dyn TileSource>,
		cells: Cells,
		cx: &mut Context<Self>,
	) {
		self.next = Some((source, cells));
		cx.notify();
	}

	/// Take `tags` as what `record` carries, in the cells shown and in any
	/// waiting to be.
	pub fn set_record_tags(&mut self, record: Uuid, tags: &[Uuid]) {
		self.cells.set_tags(record, tags);
		if let Some((_, cells)) = self.next.as_mut() {
			cells.set_tags(record, tags);
		}
	}

	pub fn set_tag_colors(&mut self, colors: HashMap<Uuid, Hsla>, cx: &mut Context<Self>) {
		self.tag_colors = colors;
		cx.notify();
	}

	pub fn selection_len(&self) -> usize {
		self.selection.len()
	}

	/// The records of the selected cells, in grid order.
	pub fn selected_records(&self) -> Vec<Uuid> {
		self.selection
			.iter()
			.filter_map(|index| self.cells.record(index))
			.collect()
	}

	/// Whether every selected cell's record carries `tag`. An empty selection
	/// carries nothing.
	pub fn selection_carries(&self, tag: Uuid) -> bool {
		!self.selection.is_empty()
			&& self
				.selection
				.iter()
				.all(|index| self.cells.tags(index).contains(&tag))
	}

	pub fn select_all(&mut self, cx: &mut Context<Self>) {
		self.selection.select_all(self.source.len());
		cx.notify();
	}

	/// Drop the selection. Returns whether there was one to drop.
	pub fn clear_selection(&mut self, cx: &mut Context<Self>) -> bool {
		if self.selection.is_empty() {
			return false;
		}
		self.selection.clear();
		cx.notify();
		true
	}

	/// Move a single selection one cell, scrolling it into view. With nothing
	/// selected yet, the first cell on screen is where it starts.
	pub fn move_selection(&mut self, direction: Direction, cx: &mut Context<Self>) {
		let len = self.source.len();
		let Some((width, height)) = self.viewport else {
			return;
		};
		if len == 0 {
			return;
		}
		let (cols, cell) = self.layout(width);
		let target = match self.selection.focus() {
			Some(focus) => step(focus.min(len - 1), direction, cols, len),
			None => ((self.scroll_y / (cell + GAP)).floor() as u32 * cols).min(len - 1),
		};
		self.selection.select_only(target);
		self.reveal(target, width, height);
		cx.notify();
	}

	/// Scroll the least distance that shows all of cell `index`.
	fn reveal(&mut self, index: u32, width: f32, height: f32) {
		let (cols, cell) = self.layout(width);
		let top = (index / cols) as f32 * (cell + GAP);
		let bottom = top + cell;
		if top < self.scroll_y {
			self.scroll_y = top;
		} else if bottom > self.scroll_y + height {
			self.scroll_y = bottom - height;
		}
		self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll(width, height));
	}

	/// The cell under `position`, a window point, for a grid painted at
	/// `bounds`. A point in the gap between cells is on none.
	fn cell_at(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> Option<u32> {
		let (cols, cell) = self.layout(f32::from(bounds.size.width));
		let pitch = cell + GAP;
		let x = f32::from(position.x - bounds.origin.x) + self.scroll_x;
		let y = f32::from(position.y - bounds.origin.y) + self.scroll_y;
		if x < 0.0 || y < 0.0 {
			return None;
		}
		let (col, row) = ((x / pitch) as u32, (y / pitch) as u32);
		if col >= cols || x - col as f32 * pitch > cell || y - row as f32 * pitch > cell {
			return None;
		}
		let index = row * cols + col;
		(index < self.source.len()).then_some(index)
	}

	/// A left click on the grid: select by the explorer's rules. A plain click
	/// on empty space clears the selection, as it does in Finder.
	fn on_click(&mut self, event: &MouseDownEvent, bounds: Bounds<Pixels>) -> bool {
		let toggle = event.modifiers.secondary();
		let extend = event.modifiers.shift;
		match self.cell_at(event.position, bounds) {
			Some(index) => self.selection.click(index, toggle, extend),
			None if !toggle && !extend && !self.selection.is_empty() => self.selection.clear(),
			None => return false,
		}
		true
	}

	/// How many cells the current source is offering.
	pub fn len(&self) -> u32 {
		self.source.len()
	}

	/// Columns and displayed cell size for the current width. While a pinch is
	/// live the column count is held and the tile scales continuously, so the
	/// grid overflows sideways; otherwise the row width is shared across
	/// columns and tiles fill the viewport edge to edge (same policy as the
	/// reference grid).
	fn layout(&self, width: f32) -> (u32, f32) {
		match &self.pinch {
			Some(pinch) => (pinch.cols0, (pinch.pitch0 * pinch.g - GAP).max(1.0)),
			None => {
				let cols = (((width + GAP) / (self.target_cell + GAP)).floor() as u32).max(1);
				(
					cols,
					((width - (cols - 1) as f32 * GAP) / cols as f32).max(1.0),
				)
			}
		}
	}

	fn max_scroll(&self, width: f32, height: f32) -> f32 {
		let (cols, cell) = self.layout(width);
		let rows = (self.source.len() + cols - 1) / cols;
		(rows as f32 * (cell + GAP) - height).max(0.0)
	}

	pub fn scroll_by(&mut self, dy: f32, width: f32, height: f32) {
		self.scroll_y = (self.scroll_y - dy).clamp(0.0, self.max_scroll(width, height));
	}

	/// Drive pinch zoom from a trackpad magnify event. The gesture holds its
	/// column count and scales the tile continuously, anchored so the content
	/// under the fingers stays put; on release it settles onto the nearest
	/// fill-width layout at the new density, keeping that same item fixed
	/// across the reflow.
	///
	/// `focal` is the gesture centroid in window coordinates.
	fn on_pinch(
		&mut self,
		phase: TouchPhase,
		delta: f32,
		focal: Point<Pixels>,
		bounds: Bounds<Pixels>,
	) {
		let width = f32::from(bounds.size.width);
		let height = f32::from(bounds.size.height);
		match phase {
			// macOS reports `NSEventPhaseMayBegin` as `Started`, so a gesture
			// can open without ever moving. Harmless: the scale starts at 1.0,
			// which reproduces the layout the capture was taken from.
			TouchPhase::Started => {
				let (cols0, cell0) = self.layout(width);
				let pitch0 = cell0 + GAP;
				let fx = (f32::from(focal.x) - f32::from(bounds.origin.x)).max(0.0);
				let fy = (f32::from(focal.y) - f32::from(bounds.origin.y)).max(0.0);
				let ux = (fx + self.scroll_x) / pitch0;
				let uy = (fy + self.scroll_y) / pitch0;
				let col = (ux.floor() as i64).clamp(0, cols0 as i64 - 1) as u32;
				let row = uy.floor().max(0.0) as u32;
				let focal_item = (row * cols0 + col).min(self.source.len().saturating_sub(1));
				self.pinch = Some(PinchState {
					g: 1.0,
					cols0,
					pitch0,
					fx,
					fy,
					ux,
					uy,
					focal_item,
					frac_y: uy - uy.floor(),
				});
			}
			TouchPhase::Moved => {
				if let Some(pinch) = self.pinch.as_mut() {
					pinch.g = (pinch.g * (1.0 + delta)).clamp(
						(MIN_CELL + GAP) / pinch.pitch0,
						(MAX_CELL + GAP) / pinch.pitch0,
					);
					// Re-anchor the focal content point under the fingers,
					// which do not move during a magnify. Left unclamped so
					// the anchor holds at the ends of the range; the settle
					// below is what brings the offsets back in bounds.
					let pitch = pinch.pitch0 * pinch.g;
					self.scroll_x = pinch.ux * pitch - pinch.fx;
					self.scroll_y = pinch.uy * pitch - pinch.fy;
				}
			}
			TouchPhase::Ended | TouchPhase::Cancelled => {
				if let Some(pinch) = self.pinch.take() {
					self.target_cell = (pinch.pitch0 * pinch.g - GAP).clamp(MIN_CELL, MAX_CELL);
					// The column count changes here. Keep the focal item under
					// the same screen point through the reflow.
					let (cols_new, cell_new) = self.layout(width);
					let anchor = (pinch.focal_item / cols_new) as f32 + pinch.frac_y;
					let max = self.max_scroll(width, height);
					self.scroll_y = (anchor * (cell_new + GAP) - pinch.fy).clamp(0.0, max);
					self.scroll_x = 0.0;
				}
			}
		}
	}

	/// Advance the scripted scroll. Returns false once the script is done.
	fn advance_auto(&mut self, width: f32, height: f32) -> bool {
		let max = self.max_scroll(width, height);
		let Some(auto) = self.auto.as_mut() else {
			return false;
		};
		let now = Instant::now();
		if !auto.started {
			auto.started = true;
			auto.last_tick = now;
			let label = auto.phases[0].0;
			auto.velocity = resolve_velocity(&auto.phases[0].2, auto.phases[0].1, max);
			self.stats.mark(label);
			return true;
		}
		let dt = (now - auto.last_tick).as_secs_f32();
		auto.last_tick = now;
		self.scroll_y = (self.scroll_y + auto.velocity * dt).clamp(0.0, max);
		auto.elapsed_in_phase += dt;
		if auto.elapsed_in_phase >= auto.phases[auto.phase].1 {
			auto.phase += 1;
			auto.elapsed_in_phase = 0.0;
			if auto.phase >= auto.phases.len() {
				return false;
			}
			let (label, dur, vel) = &auto.phases[auto.phase];
			auto.velocity = resolve_velocity(vel, *dur, max);
			self.stats.mark(label);
		}
		true
	}

	/// The whole per-frame path: drain bakes, cull, paint, evict, and drive
	/// the benchmark script. Runs inside the element's paint phase.
	fn paint_grid(
		&mut self,
		bounds: Bounds<Pixels>,
		hitbox: &Hitbox,
		window: &mut Window,
		cx: &mut Context<GridView>,
	) {
		let (background, placeholder, chrome) = {
			let theme = cx.theme();
			(
				theme.app,
				theme.app_box,
				Chrome {
					accent: theme.accent,
					white: theme.white,
					shade: theme.black,
				},
			)
		};
		let t0 = self.stats.begin_frame();
		self.frame_index += 1;
		let width = f32::from(bounds.size.width);
		let height = f32::from(bounds.size.height);
		self.viewport = Some((width, height));

		// A retarget queued since the last frame: the old cells are gone, so
		// their CPU copies and atlas tiles go with them, the selection made
		// among them is dropped, and the view returns to the top of the new set.
		if let Some((source, cells)) = self.next.take() {
			self.source = source;
			self.cells = cells;
			self.selection.clear();
			self.scroll_y = 0.0;
			self.scroll_x = 0.0;
			self.pinch = None;
			for (_, (img, ..)) in self.cache.drain() {
				let _ = window.drop_image(img);
			}
		}

		// Absorb background progress (a folder walk landing, bake
		// completions) before anything reads the cell count.
		self.source.poll();

		let auto_running = if self.auto.is_some() && !self.finished {
			let running = self.advance_auto(width, height);
			if !running {
				self.finished = true;
				self.finish_report();
				cx.quit();
			}
			running
		} else {
			false
		};

		// Accept finished tiles. The RenderImage is created here (CPU-side);
		// gpui uploads it into the sprite atlas lazily on its first paint. A
		// redelivery (stale pixels refreshed by a rebake) replaces the cached
		// image, so the old one's atlas tile is released explicitly.
		for (idx, bitmap) in self.source.drain(DRAIN_PER_FRAME) {
			let aspect = bitmap.width as f32 / bitmap.height.max(1) as f32;
			let buffer = image::RgbaImage::from_raw(bitmap.width, bitmap.height, bitmap.bgra)
				.expect("tile buffer has exact dimensions");
			let img = Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]));
			if let Some((old, ..)) = self.cache.insert(idx, (img, aspect, self.tick)) {
				let _ = window.drop_image(old);
			}
		}

		// Layout + cull: identical derivation to the reference renderer.
		let count = self.source.len();
		let (cols, cell) = self.layout(width);
		let pitch = cell + GAP;
		let first_row = (self.scroll_y / pitch).floor() as u32;
		let visible_rows = (height / pitch).ceil() as u32 + 1;
		let first = (first_row * cols).min(count);
		let last = ((first_row + visible_rows) * cols).min(count);
		self.source.set_visible_range(first, last);

		window.paint_quad(fill(bounds, background));

		self.tick += 1;
		let mut painted = 0usize;
		for idx in first..last {
			let row = idx / cols;
			let col = idx % cols;
			let x = f32::from(bounds.origin.x) + col as f32 * pitch - self.scroll_x;
			let y = f32::from(bounds.origin.y) + row as f32 * pitch - self.scroll_y;
			let cell_bounds = Bounds::new(point(px(x), px(y)), size(px(cell), px(cell)));
			match self.cache.get_mut(&idx) {
				Some((img, aspect, used)) => {
					*used = self.tick;
					let frame = fit_within(cell_bounds, *aspect);
					let _ =
						window.paint_image(frame, frame, Corners::default(), img.clone(), 0, false);
					painted += 1;
				}
				None => {
					// No pixels yet: placeholder quad, and ask the source.
					// The request is idempotent while one is outstanding, and
					// a source that knows nothing will come drops it.
					window.paint_quad(fill(cell_bounds, placeholder));
					self.source.request(idx);
				}
			}
			if self.selection.contains(idx) {
				paint_selected(window, cell_bounds, &chrome);
			} else if self.selection.focus() == Some(idx) {
				paint_ring(window, cell_bounds, chrome.accent.opacity(0.5));
			}
			let tags = self.cells.tags(idx);
			if !tags.is_empty() {
				paint_tag_dots(window, cx, cell_bounds, tags, &self.tag_colors, &chrome);
			}
		}

		// LRU eviction: drop both our CPU copy and gpui's atlas tile.
		while self.cache.len() > CACHE_CAP {
			let oldest = self
				.cache
				.iter()
				.min_by_key(|(_, (_, _, used))| *used)
				.map(|(idx, _)| *idx)
				.expect("cache is non-empty");
			if let Some((img, ..)) = self.cache.remove(&oldest) {
				let _ = window.drop_image(img);
			}
		}

		// Manual scrolling (trackpad / wheel) over the grid's hitbox.
		let entity = cx.entity();
		let wheel_hitbox = hitbox.clone();
		window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
			if phase != DispatchPhase::Bubble || !wheel_hitbox.is_hovered(window) {
				return;
			}
			let dy = match event.delta {
				ScrollDelta::Pixels(p) => f32::from(p.y),
				ScrollDelta::Lines(l) => l.y * pitch,
			};
			entity.update(cx, |view, cx| {
				view.scroll_by(dy, width, height);
				cx.notify();
			});
		});

		// Clicks select.
		let entity = cx.entity();
		let click_hitbox = hitbox.clone();
		window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
			if phase != DispatchPhase::Bubble
				|| event.button != MouseButton::Left
				|| !click_hitbox.is_hovered(window)
			{
				return;
			}
			entity.update(cx, |view, cx| {
				if view.on_click(event, bounds) {
					cx.notify();
				}
			});
		});

		// Pinch zoom. gpui carries the trackpad magnify gesture itself, so
		// this is the same subscription shape as the wheel above.
		let entity = cx.entity();
		let pinch_hitbox = hitbox.clone();
		window.on_mouse_event(move |event: &PinchEvent, phase, window, cx| {
			// A gesture that opens over the grid keeps it until it settles,
			// so a pinch that drifts off the hitbox mid-zoom is not dropped
			// half-applied.
			let live = entity.read(cx).pinch.is_some();
			if phase != DispatchPhase::Bubble || !(live || pinch_hitbox.is_hovered(window)) {
				return;
			}
			entity.update(cx, |view, cx| {
				view.on_pinch(event.phase, event.delta, event.position, bounds);
				cx.notify();
			});
		});

		// Keep frames coming while the script runs or the source still owes
		// tiles. Frames are demand driven: skipping the re-request on any
		// path that still needs frames stalls the app, and re-requesting when
		// nothing is owed burns CPU at idle — `has_pending` settles to false
		// even for cells that can never fill, so this reaches silence.
		if auto_running || self.source.has_pending() {
			window.request_animation_frame();
		}

		self.stats.end_frame(t0, painted);
		if self.auto.is_some() && self.frame_index % 300 == 0 {
			eprintln!(
				"frame {} | scroll {:.0} | cache {} tiles | rss {:.0} MB",
				self.frame_index,
				self.scroll_y,
				self.cache.len(),
				resident_mb(),
			);
		}
	}

	fn finish_report(&self) {
		eprintln!("==== grid benchmark complete ====");
		eprintln!("cells: {}", self.source.len());
		self.stats.report("grid-total");
		eprintln!(
			"cache: {} tiles resident (cap {CACHE_CAP}) | rss {:.0} MB",
			self.cache.len(),
			resident_mb(),
		);
	}
}

fn resolve_velocity(vel: &PhaseVel, duration: f32, max_scroll: f32) -> f32 {
	match vel {
		PhaseVel::Fixed(v) => *v,
		PhaseVel::Traverse(fraction) => max_scroll * fraction / duration,
	}
}

/// The theme colors cell chrome is drawn in, read once per frame.
struct Chrome {
	accent: Hsla,
	white: Hsla,
	shade: Hsla,
}

/// A selected cell: tinted, ringed, and badged with a check.
fn paint_selected(window: &mut Window, cell: Bounds<Pixels>, chrome: &Chrome) {
	window.paint_quad(fill(cell, chrome.accent.opacity(TINT)));
	paint_ring(window, cell, chrome.accent);

	let badge = Bounds::new(
		cell.origin + point(px(BADGE_INSET), px(BADGE_INSET)),
		size(px(BADGE), px(BADGE)),
	);
	window.paint_quad(quad(
		badge,
		Corners::all(px(BADGE / 2.0)),
		chrome.accent,
		Edges::default(),
		transparent_black(),
		BorderStyle::Solid,
	));
	let at = |x: f32, y: f32| badge.origin + point(px(BADGE * x), px(BADGE * y));
	let mut check = PathBuilder::stroke(px(1.75));
	check.move_to(at(0.28, 0.52));
	check.line_to(at(0.44, 0.68));
	check.line_to(at(0.74, 0.34));
	if let Ok(path) = check.build() {
		window.paint_path(path, chrome.white);
	}
}

/// A ring inset along a cell's edges.
fn paint_ring(window: &mut Window, cell: Bounds<Pixels>, color: Hsla) {
	window.paint_quad(quad(
		cell,
		Corners::default(),
		transparent_black(),
		Edges::all(px(RING)),
		color,
		BorderStyle::Solid,
	));
}

/// Dots for a cell's tags on a dark pill in its bottom-left corner: the first
/// few in their colors, then a count of the rest, as the explorer's cards show
/// them.
fn paint_tag_dots(
	window: &mut Window,
	cx: &mut App,
	cell: Bounds<Pixels>,
	tags: &[Uuid],
	colors: &HashMap<Uuid, Hsla>,
	chrome: &Chrome,
) {
	let shown = tags.len().min(DOTS_SHOWN);
	let rest = tags.len() - shown;
	let count = (rest > 0).then(|| {
		let text = SharedString::from(format!("+{rest}"));
		let run = TextRun {
			len: text.len(),
			font: window.text_style().font(),
			color: chrome.white.opacity(0.9),
			background_color: None,
			underline: None,
			strikethrough: None,
		};
		window
			.text_system()
			.shape_line(text, px(COUNT_TEXT), &[run], None)
	});

	let dots_width = shown as f32 * DOT + shown.saturating_sub(1) as f32 * DOT_GAP;
	let count_width = count
		.as_ref()
		.map_or(0.0, |line| DOT_GAP + f32::from(line.width));
	let height = DOT + 2.0 * PILL_PAD;
	let pill = Bounds::new(
		point(
			cell.origin.x + px(PILL_INSET),
			cell.origin.y + cell.size.height - px(PILL_INSET + height),
		),
		size(px(dots_width + count_width + 2.0 * PILL_PAD), px(height)),
	);
	window.paint_quad(quad(
		pill,
		Corners::all(px(height / 2.0)),
		chrome.shade.opacity(0.55),
		Edges::default(),
		transparent_black(),
		BorderStyle::Solid,
	));

	for (slot, tag) in tags.iter().take(shown).enumerate() {
		// A tag made since the library's tags were last read has no color
		// here yet, and draws in the default until they are read again.
		let color = colors
			.get(tag)
			.copied()
			.unwrap_or_else(|| rgb(DEFAULT_TAG_COLOR).into());
		let dot = Bounds::new(
			pill.origin + point(px(PILL_PAD + slot as f32 * (DOT + DOT_GAP)), px(PILL_PAD)),
			size(px(DOT), px(DOT)),
		);
		window.paint_quad(quad(
			dot,
			Corners::all(px(DOT / 2.0)),
			color,
			Edges::default(),
			transparent_black(),
			BorderStyle::Solid,
		));
	}

	if let Some(line) = count {
		let origin = pill.origin + point(px(PILL_PAD + dots_width + DOT_GAP), px(0.0));
		let _ = line.paint(origin, px(height), TextAlign::Left, None, window, cx);
	}
}

impl Render for GridView {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		GridElement { view: cx.entity() }
	}
}

/// The custom element: fills its container and paints the visible grid slice.
pub struct GridElement {
	view: Entity<GridView>,
}

impl Element for GridElement {
	type RequestLayoutState = ();
	type PrepaintState = Hitbox;

	fn id(&self) -> Option<ElementId> {
		None
	}

	fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
		None
	}

	fn request_layout(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		window: &mut Window,
		cx: &mut App,
	) -> (LayoutId, Self::RequestLayoutState) {
		let mut style = Style::default();
		style.size.width = relative(1.0).into();
		style.size.height = relative(1.0).into();
		(window.request_layout(style, [], cx), ())
	}

	fn prepaint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		window: &mut Window,
		_cx: &mut App,
	) -> Self::PrepaintState {
		window.insert_hitbox(bounds, HitboxBehavior::Normal)
	}

	fn paint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		hitbox: &mut Self::PrepaintState,
		window: &mut Window,
		cx: &mut App,
	) {
		let view = self.view.clone();
		// A partially scrolled row runs past the element on every axis, and a
		// live pinch overflows sideways as well, so the paint is masked to the
		// element rather than trusting the cull to stay inside it.
		window.with_content_mask(Some(ContentMask { bounds }), |window| {
			view.update(cx, |grid, cx| grid.paint_grid(bounds, hitbox, window, cx));
		});
	}
}

impl IntoElement for GridElement {
	type Element = Self;

	fn into_element(self) -> Self::Element {
		self
	}
}
