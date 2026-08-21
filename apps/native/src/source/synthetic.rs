//! Synthetic tile production: procedurally generated BGRA8 cells, unique per
//! index so no cache can fake the workload, baked on a background worker pool
//! the way the production grid's bake crate does. This is the benchmark
//! source; `PvcacheSource` is the real one.

use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use super::{num_workers, TileSource, TILE};

/// A 3x5 blocky digit font for stamping the cell index into the tile — enough
/// to visually confirm every cell is unique without a text raster dependency.
const DIGITS: [[u8; 5]; 10] = [
	[0b111, 0b101, 0b101, 0b101, 0b111], // 0
	[0b010, 0b110, 0b010, 0b010, 0b111], // 1
	[0b111, 0b001, 0b111, 0b100, 0b111], // 2
	[0b111, 0b001, 0b111, 0b001, 0b111], // 3
	[0b101, 0b101, 0b111, 0b001, 0b001], // 4
	[0b111, 0b100, 0b111, 0b001, 0b111], // 5
	[0b111, 0b100, 0b111, 0b101, 0b111], // 6
	[0b111, 0b001, 0b010, 0b010, 0b010], // 7
	[0b111, 0b101, 0b111, 0b101, 0b111], // 8
	[0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// The synthetic source: a fixed cell count over a procedural generator.
pub struct SyntheticSource {
	count: u32,
	pool: TilePool,
}

impl SyntheticSource {
	pub fn new(count: u32) -> Self {
		Self {
			count,
			pool: TilePool::new(num_workers()),
		}
	}
}

impl TileSource for SyntheticSource {
	fn poll(&mut self) {}

	fn len(&self) -> u32 {
		self.count
	}

	fn tile(&self) -> u32 {
		TILE
	}

	fn request(&mut self, idx: u32) {
		self.pool.request(idx);
	}

	fn has_pending(&self) -> bool {
		self.pool.has_pending()
	}

	fn drain(&mut self, max: usize) -> Vec<(u32, Vec<u8>)> {
		self.pool.drain(max)
	}

	fn set_visible_range(&mut self, _first: u32, _last: u32) {}
}

/// Generate one BGRA8 tile. Every cell gets a distinct hue (golden-ratio hash),
/// one of three pattern families, and its index stamped in the corner.
fn generate_tile(idx: u32) -> Vec<u8> {
	let n = (TILE * TILE * 4) as usize;
	let mut buf = vec![0u8; n];

	// Distinct hue per cell; neighbors land far apart on the wheel.
	let hue = (idx as f32 * 0.618_034) % 1.0;
	let (r0, g0, b0) = hsl(hue, 0.55, 0.45);
	let (r1, g1, b1) = hsl((hue + 0.13) % 1.0, 0.65, 0.25);
	let family = idx % 3;

	for y in 0..TILE {
		for x in 0..TILE {
			let fx = x as f32 / (TILE - 1) as f32;
			let fy = y as f32 / (TILE - 1) as f32;
			// Blend factor per pattern family: diagonal gradient, radial
			// gradient, or a checkerboard modulated by a vertical gradient.
			let t = match family {
				0 => (fx + fy) * 0.5,
				1 => {
					let dx = fx - 0.5;
					let dy = fy - 0.5;
					((dx * dx + dy * dy).sqrt() * 1.6).min(1.0)
				}
				_ => {
					let check = ((x / 32) + (y / 32)) % 2;
					if check == 0 {
						fy * 0.6
					} else {
						0.4 + fy * 0.6
					}
				}
			};
			let r = lerp(r0, r1, t);
			let g = lerp(g0, g1, t);
			let b = lerp(b0, b1, t);
			let o = ((y * TILE + x) * 4) as usize;
			buf[o] = b;
			buf[o + 1] = g;
			buf[o + 2] = r;
			buf[o + 3] = 255;
		}
	}

	stamp_index(&mut buf, idx);
	buf
}

/// Stamp the decimal index near the top-left corner in white blocks.
fn stamp_index(buf: &mut [u8], idx: u32) {
	let digits: Vec<u32> = {
		let mut v = Vec::new();
		let mut n = idx;
		loop {
			v.push(n % 10);
			n /= 10;
			if n == 0 {
				break;
			}
		}
		v.reverse();
		v
	};
	let scale = 8u32; // each font block is 8x8 px
	let (ox, oy) = (12u32, 12u32);
	for (di, d) in digits.iter().enumerate() {
		let glyph = DIGITS[*d as usize];
		let gx = ox + di as u32 * 4 * scale;
		for (row, bits) in glyph.iter().enumerate() {
			for col in 0..3u32 {
				if bits & (1 << (2 - col)) != 0 {
					fill_block(buf, gx + col * scale, oy + row as u32 * scale, scale);
				}
			}
		}
	}
}

fn fill_block(buf: &mut [u8], x0: u32, y0: u32, s: u32) {
	for y in y0..(y0 + s).min(TILE) {
		for x in x0..(x0 + s).min(TILE) {
			let o = ((y * TILE + x) * 4) as usize;
			buf[o] = 255;
			buf[o + 1] = 255;
			buf[o + 2] = 255;
			buf[o + 3] = 255;
		}
	}
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
	(a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)) as u8
}

/// HSL → sRGB bytes.
fn hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
	let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
	let hp = h * 6.0;
	let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
	let (r, g, b) = match hp as u32 {
		0 => (c, x, 0.0),
		1 => (x, c, 0.0),
		2 => (0.0, c, x),
		3 => (0.0, x, c),
		4 => (x, 0.0, c),
		_ => (c, 0.0, x),
	};
	let m = l - c / 2.0;
	(
		((r + m) * 255.0) as u8,
		((g + m) * 255.0) as u8,
		((b + m) * 255.0) as u8,
	)
}

/// Background bake pool: the UI thread requests cell indices, workers generate
/// tiles, finished BGRA buffers come back on a channel drained once per frame.
struct TilePool {
	req_tx: Sender<u32>,
	done_rx: Receiver<(u32, Vec<u8>)>,
	/// Indices requested but not yet received, so a cell is baked at most once
	/// per residency in the cache.
	pending: HashSet<u32>,
}

impl TilePool {
	fn new(workers: usize) -> Self {
		let (req_tx, req_rx) = channel::<u32>();
		let (done_tx, done_rx) = channel::<(u32, Vec<u8>)>();
		let req_rx = Arc::new(Mutex::new(req_rx));
		for _ in 0..workers.max(1) {
			let rx = req_rx.clone();
			let tx = done_tx.clone();
			thread::spawn(move || loop {
				let idx = {
					let guard = rx.lock().unwrap();
					guard.recv()
				};
				match idx {
					Ok(i) => {
						if tx.send((i, generate_tile(i))).is_err() {
							break;
						}
					}
					Err(_) => break,
				}
			});
		}
		TilePool {
			req_tx,
			done_rx,
			pending: HashSet::new(),
		}
	}

	/// Queue a bake unless one is already in flight for this cell.
	fn request(&mut self, idx: u32) {
		if self.pending.insert(idx) {
			let _ = self.req_tx.send(idx);
		}
	}

	/// Whether any bake requests are still in flight.
	fn has_pending(&self) -> bool {
		!self.pending.is_empty()
	}

	/// Drain up to `max` finished tiles.
	fn drain(&mut self, max: usize) -> Vec<(u32, Vec<u8>)> {
		let mut out = Vec::new();
		while out.len() < max {
			match self.done_rx.try_recv() {
				Ok((i, buf)) => {
					self.pending.remove(&i);
					out.push((i, buf));
				}
				Err(_) => break,
			}
		}
		out
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::{Duration, Instant};

	#[test]
	fn synthetic_source_delivers_requested_tiles() {
		let mut source = SyntheticSource::new(10);
		assert_eq!(source.len(), 10);
		assert_eq!(source.tile(), TILE);
		assert!(!source.has_pending());

		source.request(3);
		source.request(3); // idempotent while in flight
		assert!(source.has_pending());

		let deadline = Instant::now() + Duration::from_secs(10);
		let mut tiles = Vec::new();
		while tiles.is_empty() && Instant::now() < deadline {
			tiles = source.drain(16);
			thread::sleep(Duration::from_millis(5));
		}
		assert_eq!(tiles.len(), 1, "one request in flight yields one tile");
		let (idx, bgra) = &tiles[0];
		assert_eq!(*idx, 3);
		assert_eq!(bgra.len(), (TILE * TILE * 4) as usize);
		assert!(!source.has_pending());
	}
}
