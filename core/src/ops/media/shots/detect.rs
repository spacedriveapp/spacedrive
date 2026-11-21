//! # Cut detection
//!
//! Cuts are picked from FFmpeg's per-frame scene score.
//!
//! FFmpeg's `scene` value is how much a frame differs from the one before,
//! from 0 to 1. A hard cut between unrelated pictures scores well above
//! 0.3. A jump cut inside one setup scores lower, often 0.15 to 0.3, which
//! is also where a fast handheld pan lands. What separates the two is that a
//! cut is a single-frame spike and a pan raises every frame around it. So a
//! frame is a cut when it scores at least `threshold`, or when it scores at
//! least half of it and stands well above the frames around it. The minimum
//! shot length then keeps the strongest cut of any cluster, so a flash or a
//! shaky stretch cannot split a shot into slivers.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How eagerly frames count as cuts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct DetectParams {
	/// A frame scoring at least this is always a cut.
	pub threshold: f64,
	/// No shot is shorter than this, except a whole video that is.
	pub min_shot_seconds: f64,
}

impl Default for DetectParams {
	fn default() -> Self {
		Self {
			threshold: 0.3,
			min_shot_seconds: 1.0,
		}
	}
}

impl DetectParams {
	pub fn validate(&self) -> Result<(), String> {
		if !(0.05..=1.0).contains(&self.threshold) {
			return Err("threshold must be between 0.05 and 1".into());
		}
		if !(0.0..=600.0).contains(&self.min_shot_seconds) {
			return Err("min_shot_seconds must be between 0 and 600".into());
		}
		Ok(())
	}
}

/// How far a softer spike must stand above the mean of its surroundings.
const SPIKE_RATIO: f32 = 4.0;
/// The surroundings a spike is compared with, each side, in seconds.
const SPIKE_WINDOW_SECONDS: f64 = 0.5;
/// Floor for that mean, so a spike out of a still picture still has to
/// clear a real score.
const SPIKE_FLOOR: f32 = 0.01;

/// One decoded frame of the scanned file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameScore {
	/// Presentation time in the scanned file, for seeking to it.
	pub time: f64,
	/// Difference from the previous frame, 0 to 1.
	pub score: f32,
}

/// Read the output of FFmpeg's `metadata=print` filter after
/// `select='gte(scene,0)'`: a `frame:N pts:P pts_time:T` line and then a
/// `lavfi.scene_score=S` line for every decoded frame, in order.
pub fn parse_scores(output: &str) -> Vec<FrameScore> {
	let mut frames = Vec::new();
	let mut time = None;
	for line in output.lines() {
		let line = line.trim();
		if line.starts_with("frame:") {
			time = line
				.split_whitespace()
				.find_map(|field| field.strip_prefix("pts_time:"))
				.and_then(|value| value.parse::<f64>().ok());
		} else if let Some(score) = line.strip_prefix("lavfi.scene_score=") {
			let score = score.parse::<f32>().unwrap_or(0.0);
			let time = time.take().unwrap_or_else(|| {
				frames
					.last()
					.map_or(0.0, |previous: &FrameScore| previous.time)
			});
			frames.push(FrameScore {
				time,
				score: if score.is_finite() { score } else { 0.0 },
			});
		}
	}
	frames
}

/// The frames that open a new shot, ascending, each with its score. Frame 0
/// is never one of them.
///
/// The minimum shot length is measured on the frames' own timestamps, so a
/// variable frame rate video keeps shots of the length the person sees.
pub fn pick_cuts(frames: &[FrameScore], fps: f64, params: DetectParams) -> Vec<(u64, f32)> {
	let scores: Vec<f32> = frames.iter().map(|frame| frame.score).collect();
	let window = (SPIKE_WINDOW_SECONDS * fps).round().max(1.0) as usize;
	let threshold = params.threshold as f32;
	let soft = threshold / 2.0;
	let mut candidates: Vec<usize> = (1..scores.len())
		.filter(|&i| {
			let score = scores[i];
			if score >= threshold {
				return true;
			}
			if score < soft {
				return false;
			}
			let lo = i.saturating_sub(window);
			let hi = (i + window + 1).min(scores.len());
			let around = (hi - lo - 1).max(1) as f32;
			let mean = (scores[lo..hi].iter().sum::<f32>() - score) / around;
			score >= SPIKE_RATIO * mean.max(SPIKE_FLOOR)
		})
		.collect();

	// Strongest first, earliest on a tie, so a rerun picks the same cuts.
	candidates.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
	let min = params.min_shot_seconds;
	let time = |frame: usize| frames[frame].time;
	let start = time(0);
	let end = time(frames.len() - 1) + 1.0 / fps;
	let mut accepted: Vec<usize> = Vec::new();
	for frame in candidates {
		if time(frame) - start < min || end - time(frame) < min {
			continue;
		}
		let position = accepted.partition_point(|&cut| cut < frame);
		let clear_before = position == 0 || time(frame) - time(accepted[position - 1]) >= min;
		let clear_after =
			position == accepted.len() || time(accepted[position]) - time(frame) >= min;
		if clear_before && clear_after {
			accepted.insert(position, frame);
		}
	}
	accepted
		.into_iter()
		.map(|frame| (frame as u64, scores[frame]))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn still(frames: usize) -> Vec<f32> {
		vec![0.002; frames]
	}

	fn at_24(scores: &[f32]) -> Vec<FrameScore> {
		scores
			.iter()
			.enumerate()
			.map(|(i, &score)| FrameScore {
				time: i as f64 / 24.0,
				score,
			})
			.collect()
	}

	#[test]
	fn parses_metadata_print_output() {
		let output = "frame:0    pts:0       pts_time:0\nlavfi.scene_score=0.000000\nframe:1    pts:800     pts_time:0.0417014\nlavfi.scene_score=0.106277\n";
		let frames = parse_scores(output);
		assert_eq!(frames.len(), 2);
		assert_eq!(frames[1].time, 0.0417014);
		assert!((frames[1].score - 0.106277).abs() < 1e-6);
	}

	#[test]
	fn hard_cuts_split() {
		let mut scores = still(240);
		scores[100] = 0.6;
		scores[180] = 0.45;
		assert_eq!(
			pick_cuts(&at_24(&scores), 24.0, DetectParams::default()),
			vec![(100, 0.6), (180, 0.45)]
		);
	}

	#[test]
	fn an_isolated_soft_spike_is_a_jump_cut() {
		let mut scores = still(240);
		scores[120] = 0.2;
		assert_eq!(
			pick_cuts(&at_24(&scores), 24.0, DetectParams::default()),
			vec![(120, 0.2)]
		);
	}

	#[test]
	fn handheld_motion_does_not_split() {
		let scores: Vec<f32> = (0..480)
			.map(|i| 0.12 + 0.1 * ((i as f32) * 0.7).sin().abs())
			.collect();
		assert!(pick_cuts(&at_24(&scores), 24.0, DetectParams::default()).is_empty());
	}

	#[test]
	fn the_minimum_shot_length_keeps_the_strongest_cut() {
		let mut scores = still(240);
		scores[100] = 0.4;
		scores[105] = 0.9;
		scores[110] = 0.5;
		scores[5] = 0.9;
		scores[236] = 0.9;
		assert_eq!(
			pick_cuts(&at_24(&scores), 24.0, DetectParams::default()),
			vec![(105, 0.9)]
		);
	}

	#[test]
	fn the_minimum_length_follows_timestamps() {
		let mut frames = at_24(&still(240));
		for frame in frames.iter_mut().skip(100) {
			frame.time += 5.0;
		}
		frames[100].score = 0.6;
		frames[110].score = 0.5;
		assert_eq!(
			pick_cuts(&frames, 24.0, DetectParams::default()),
			vec![(100, 0.6)]
		);
		frames[90].score = 0.9;
		assert_eq!(
			pick_cuts(&frames, 24.0, DetectParams::default()),
			vec![(90, 0.9), (100, 0.6)]
		);
	}
}
