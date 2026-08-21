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

mod stats;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
	fill, point, px, relative, size, App, Bounds, Context, Corners, DispatchPhase, Element,
	ElementId, Entity, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement,
	LayoutId, Pixels, Render, RenderImage, ScrollDelta, ScrollWheelEvent, Style, Window,
};
use image::Frame;
use smallvec::smallvec;

use crate::source::TileSource;
use crate::theme::ActiveTheme as _;
use stats::{resident_mb, FrameStats};

/// Logical-point tile size and gap, matching the reference grid.
const TARGET_CELL: f32 = 128.0;
const GAP: f32 = 2.0;
/// Baked tiles held resident (CPU bytes + sprite-atlas space). Visible set is
/// ~100 cells, so this gives a healthy scroll-back margin while bounding
/// memory: 512 x 256KB = 128MB CPU-side + the same in atlas pages.
const CACHE_CAP: usize = 512;
/// Finished bakes accepted per frame (the reference grid uses 1024).
const DRAIN_PER_FRAME: usize = 1024;

/// Default synthetic cell count, matching the reference renderer's proven scale.
const DEFAULT_CELLS: u32 = 129_000;

/// Grid configuration from the environment: `SD_GRID_CELLS` overrides the
/// synthetic cell count, `SD_GRID_BENCH=1` enables the scripted benchmark.
pub fn config_from_env() -> (u32, bool) {
	let cells = std::env::var("SD_GRID_CELLS")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(DEFAULT_CELLS);
	let bench = std::env::var("SD_GRID_BENCH").map_or(false, |v| v == "1");
	(cells, bench)
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

pub struct GridView {
	source: Box<dyn TileSource>,
	scroll_y: f32,
	cache: HashMap<u32, (Arc<RenderImage>, u64)>,
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
			scroll_y: 0.0,
			cache: HashMap::new(),
			tick: 0,
			stats: FrameStats::new(),
			auto: bench.then(AutoScroll::new),
			finished: false,
			frame_index: 0,
		}
	}

	fn cols(&self, width: f32) -> u32 {
		(((width + GAP) / (TARGET_CELL + GAP)).floor() as u32).max(1)
	}

	/// Displayed cell size: the row width shared across columns so tiles fill
	/// the viewport edge to edge (same policy as the reference grid).
	fn cell(&self, width: f32) -> f32 {
		let cols = self.cols(width);
		((width - (cols - 1) as f32 * GAP) / cols as f32).max(1.0)
	}

	fn max_scroll(&self, width: f32, height: f32) -> f32 {
		let cols = self.cols(width);
		let rows = (self.source.len() + cols - 1) / cols;
		let pitch = self.cell(width) + GAP;
		(rows as f32 * pitch - height).max(0.0)
	}

	pub fn scroll_by(&mut self, dy: f32, width: f32, height: f32) {
		self.scroll_y = (self.scroll_y - dy).clamp(0.0, self.max_scroll(width, height));
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
		let (background, placeholder) = {
			let theme = cx.theme();
			(theme.app, theme.app_box)
		};
		let t0 = self.stats.begin_frame();
		self.frame_index += 1;
		let width = f32::from(bounds.size.width);
		let height = f32::from(bounds.size.height);

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
		let tile = self.source.tile();
		for (idx, bgra) in self.source.drain(DRAIN_PER_FRAME) {
			let buffer = image::RgbaImage::from_raw(tile, tile, bgra)
				.expect("tile buffer has exact dimensions");
			let img = Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]));
			if let Some((old, _)) = self.cache.insert(idx, (img, self.tick)) {
				let _ = window.drop_image(old);
			}
		}

		// Layout + cull: identical derivation to the reference renderer.
		let count = self.source.len();
		let cols = self.cols(width);
		let cell = self.cell(width);
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
			let x = f32::from(bounds.origin.x) + col as f32 * pitch;
			let y = f32::from(bounds.origin.y) + row as f32 * pitch - self.scroll_y;
			let cell_bounds = Bounds::new(point(px(x), px(y)), size(px(cell), px(cell)));
			match self.cache.get_mut(&idx) {
				Some((img, used)) => {
					*used = self.tick;
					let _ = window.paint_image(
						cell_bounds,
						cell_bounds,
						Corners::default(),
						img.clone(),
						0,
						false,
					);
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
		}

		// LRU eviction: drop both our CPU copy and gpui's atlas tile.
		while self.cache.len() > CACHE_CAP {
			let oldest = self
				.cache
				.iter()
				.min_by_key(|(_, (_, used))| *used)
				.map(|(idx, _)| *idx)
				.expect("cache is non-empty");
			if let Some((img, _)) = self.cache.remove(&oldest) {
				let _ = window.drop_image(img);
			}
		}

		// Manual scrolling (trackpad / wheel) over the grid's hitbox.
		let entity = cx.entity();
		let hitbox = hitbox.clone();
		window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
			if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
				return;
			}
			let dy = match event.delta {
				ScrollDelta::Pixels(p) => f32::from(p.y),
				ScrollDelta::Lines(l) => l.y * (TARGET_CELL + GAP),
			};
			entity.update(cx, |view, cx| {
				view.scroll_by(dy, width, height);
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
		view.update(cx, |grid, cx| grid.paint_grid(bounds, hitbox, window, cx));
	}
}

impl IntoElement for GridElement {
	type Element = Self;

	fn into_element(self) -> Self::Element {
		self
	}
}
