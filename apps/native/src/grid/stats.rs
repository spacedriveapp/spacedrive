//! Frame-time instrumentation: paint-to-paint deltas (cadence, shows dropped
//! frames) and in-paint CPU time (headroom, independent of vsync), reported as
//! percentiles to stderr.

use std::time::Instant;

pub struct FrameStats {
	/// Milliseconds between successive paints.
	pub deltas: Vec<f32>,
	/// Milliseconds spent inside our paint pass.
	pub paint_cpu: Vec<f32>,
	last_paint: Option<Instant>,
	/// Peak count of cells painted in a single frame.
	pub max_cells_painted: usize,
	/// Named segment boundaries into `deltas`, for per-phase reporting.
	segments: Vec<(String, usize)>,
}

impl FrameStats {
	pub fn new() -> Self {
		FrameStats {
			deltas: Vec::with_capacity(10_000),
			paint_cpu: Vec::with_capacity(10_000),
			last_paint: None,
			max_cells_painted: 0,
			segments: Vec::new(),
		}
	}

	/// Begin a named segment at the current frame index.
	pub fn mark(&mut self, label: &str) {
		self.segments.push((label.to_string(), self.deltas.len()));
	}

	/// Call at the top of paint; returns the timestamp to pass to `end_frame`.
	pub fn begin_frame(&mut self) -> Instant {
		let now = Instant::now();
		if let Some(prev) = self.last_paint {
			self.deltas.push((now - prev).as_secs_f32() * 1000.0);
		}
		self.last_paint = now.into();
		now
	}

	pub fn end_frame(&mut self, start: Instant, cells_painted: usize) {
		self.paint_cpu.push(start.elapsed().as_secs_f32() * 1000.0);
		self.max_cells_painted = self.max_cells_painted.max(cells_painted);
	}

	pub fn report(&self, label: &str) {
		self.report_range(label, 0, self.deltas.len());
		for (i, (name, start)) in self.segments.iter().enumerate() {
			let end = self
				.segments
				.get(i + 1)
				.map(|(_, s)| *s)
				.unwrap_or(self.deltas.len());
			self.report_range(name, *start, end);
		}
		eprintln!(
			"[{label}] max cells painted in one frame: {}",
			self.max_cells_painted
		);
	}

	fn report_range(&self, label: &str, start: usize, end: usize) {
		let p = |v: &[f32]| -> (f32, f32, f32, f32) {
			if v.is_empty() {
				return (0.0, 0.0, 0.0, 0.0);
			}
			let mut v = v.to_vec();
			v.sort_by(|a, b| a.partial_cmp(b).unwrap());
			let at = |q: f32| v[((v.len() - 1) as f32 * q) as usize];
			(at(0.5), at(0.95), at(0.99), *v.last().unwrap())
		};
		let end_cpu = end.min(self.paint_cpu.len());
		let (d50, d95, d99, dmax) = p(&self.deltas[start.min(end)..end]);
		let (c50, c95, c99, cmax) = p(&self.paint_cpu[start.min(end_cpu)..end_cpu]);
		eprintln!(
			"[{label}] frames={} | frame ms p50={d50:.2} p95={d95:.2} p99={d99:.2} max={dmax:.2} | paint cpu ms p50={c50:.2} p95={c95:.2} p99={c99:.2} max={cmax:.2}",
			end - start.min(end),
		);
	}
}

/// Resident set size of this process in megabytes, via `ps` (spike-grade but
/// honest — it is the same number Activity Monitor reports as memory).
pub fn resident_mb() -> f32 {
	let out = std::process::Command::new("ps")
		.args(["-o", "rss=", "-p", &std::process::id().to_string()])
		.output();
	match out {
		Ok(o) => String::from_utf8_lossy(&o.stdout)
			.trim()
			.parse::<f32>()
			.map(|kb| kb / 1024.0)
			.unwrap_or(0.0),
		Err(_) => 0.0,
	}
}
