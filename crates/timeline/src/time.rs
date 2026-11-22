//! Frame rates, frame counts and timecode.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A frame rate as a reduced fraction of frames per second, such as
/// `24000/1001` for 23.976.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(try_from = "RawRate")]
pub struct Rate {
	pub num: u32,
	pub den: u32,
}

#[derive(Deserialize)]
struct RawRate {
	num: u32,
	den: u32,
}

impl TryFrom<RawRate> for Rate {
	type Error = String;

	fn try_from(raw: RawRate) -> Result<Self, Self::Error> {
		Rate::new(raw.num, raw.den)
	}
}

/// Which way a time between two frames moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
	/// To the frame at or before it, for an in point.
	Floor,
	/// To the frame at or after it, for an exclusive out point.
	Ceil,
	Nearest,
}

/// One instant as an agent reads it: the frame is the truth, seconds and
/// timecode are for people.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct TimePoint {
	pub frame: i64,
	pub seconds: f64,
	pub timecode: String,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
	while b != 0 {
		(a, b) = (b, a % b);
	}
	a
}

impl Rate {
	pub fn new(num: u32, den: u32) -> Result<Self, String> {
		if num == 0 || den == 0 {
			return Err(format!("{num}/{den} is not a frame rate"));
		}
		let divisor = gcd(num.into(), den.into()) as u32;
		Ok(Self {
			num: num / divisor,
			den: den / divisor,
		})
	}

	/// Reads `24000/1001`, `25`, or a decimal rate such as `29.97`, which
	/// maps to its NTSC fraction when it is one.
	pub fn parse(text: &str) -> Result<Self, String> {
		let text = text.trim();
		if let Some((num, den)) = text.split_once('/') {
			let num = num
				.trim()
				.parse()
				.map_err(|_| format!("bad frame rate '{text}'"))?;
			let den = den
				.trim()
				.parse()
				.map_err(|_| format!("bad frame rate '{text}'"))?;
			return Self::new(num, den);
		}
		let fps: f64 = text
			.parse()
			.map_err(|_| format!("bad frame rate '{text}'"))?;
		if !(fps.is_finite() && fps > 0.0) {
			return Err(format!("bad frame rate '{text}'"));
		}
		let ntsc = (fps * 1.001).round();
		if (fps.fract() != 0.0) && ((ntsc * 1000.0 / 1001.0) - fps).abs() < 0.005 {
			return Self::new(ntsc as u32 * 1000, 1001);
		}
		if fps.fract() == 0.0 {
			return Self::new(fps as u32, 1);
		}
		Self::new((fps * 1000.0).round() as u32, 1000)
	}

	pub fn fps(self) -> f64 {
		f64::from(self.num) / f64::from(self.den)
	}

	/// The whole number of frames a timecode second counts: 24 for 23.976,
	/// 30 for 29.97.
	pub fn timecode_base(self) -> u32 {
		self.num.div_ceil(self.den)
	}

	/// Whether drop-frame timecode exists at this rate (29.97 and 59.94).
	pub fn supports_drop_frame(self) -> bool {
		self.den == 1001 && self.timecode_base().is_multiple_of(30)
	}

	pub fn seconds(self, frame: i64) -> f64 {
		frame as f64 * f64::from(self.den) / f64::from(self.num)
	}

	/// The frame at `seconds`. A time within a millionth of a frame of a
	/// boundary counts as on it, so 51.3013 s at 23.976 lands on frame 1230
	/// whichever way it rounds.
	pub fn frame_at(self, seconds: f64, rounding: Rounding) -> i64 {
		let exact = seconds * self.fps();
		let near = exact.round();
		if (exact - near).abs() < 1e-6 {
			return near as i64;
		}
		match rounding {
			Rounding::Floor => exact.floor() as i64,
			Rounding::Ceil => exact.ceil() as i64,
			Rounding::Nearest => near as i64,
		}
	}

	/// `frames` at this rate as a frame count at `to`.
	pub fn rescale(self, frames: i64, to: Rate, rounding: Rounding) -> i64 {
		if self == to {
			return frames;
		}
		let num = i128::from(frames) * i128::from(self.den) * i128::from(to.num);
		let den = i128::from(self.num) * i128::from(to.den);
		let quotient = num.div_euclid(den);
		let remainder = num.rem_euclid(den);
		let rounded = match rounding {
			_ if remainder == 0 => quotient,
			Rounding::Floor => quotient,
			Rounding::Ceil => quotient + 1,
			Rounding::Nearest if remainder * 2 >= den => quotient + 1,
			Rounding::Nearest => quotient,
		};
		rounded as i64
	}

	/// `frames` as an exact rational number of seconds in FCPXML's notation:
	/// `0s`, `5s`, or `1231230/24000s`. The denominator stays the rate's
	/// numerator, as Final Cut writes it, so a reader sees the frame count.
	pub fn rational_seconds(self, frames: i64) -> String {
		let sign = if frames < 0 { "-" } else { "" };
		let num = u128::from(frames.unsigned_abs()) * u128::from(self.den);
		let den = u128::from(self.num);
		if num % den == 0 {
			format!("{}{}s", if num == 0 { "" } else { sign }, num / den)
		} else {
			format!("{sign}{num}/{den}s")
		}
	}

	/// One frame's duration in FCPXML's notation, such as `1001/24000s`.
	pub fn frame_duration(self) -> String {
		self.rational_seconds(1)
	}

	/// The instant `frame` frames into a file whose first frame carries the
	/// timecode `start_timecode` (in frames at this rate).
	pub fn point(self, frame: i64, start_timecode: i64, drop_frame: bool) -> TimePoint {
		TimePoint {
			frame,
			seconds: (self.seconds(frame) * 10_000.0).round() / 10_000.0,
			timecode: timecode::format(start_timecode + frame, self, drop_frame),
		}
	}
}

impl std::fmt::Display for Rate {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}/{}", self.num, self.den)
	}
}

/// SMPTE timecode, non-drop and drop-frame.
pub mod timecode {
	use super::Rate;

	fn dropped_per_minute(rate: Rate) -> i64 {
		i64::from(rate.timecode_base() / 15)
	}

	/// `HH:MM:SS:FF`, or `HH:MM:SS;FF` for drop-frame. Hours are not wrapped
	/// at 24.
	pub fn format(frames: i64, rate: Rate, drop_frame: bool) -> String {
		let base = i64::from(rate.timecode_base());
		let drop = drop_frame && rate.supports_drop_frame();
		let sign = if frames < 0 { "-" } else { "" };
		let mut frames = frames.abs();
		if drop {
			let dropped = dropped_per_minute(rate);
			let per_ten_minutes = base * 600 - dropped * 9;
			let per_minute = base * 60 - dropped;
			let tens = frames / per_ten_minutes;
			let rest = frames % per_ten_minutes;
			frames += dropped * 9 * tens;
			if rest > dropped {
				frames += dropped * ((rest - dropped) / per_minute);
			}
		}
		let ff = frames % base;
		let seconds = frames / base;
		format!(
			"{sign}{:02}:{:02}:{:02}{}{:02}",
			seconds / 3600,
			(seconds / 60) % 60,
			seconds % 60,
			if drop { ';' } else { ':' },
			ff
		)
	}

	/// Frames from `00:00:00:00` to `text`. A `;` or `.` before the frames
	/// field marks drop-frame, as cameras and FFprobe write it.
	pub fn parse(text: &str, rate: Rate) -> Result<(i64, bool), String> {
		let text = text.trim();
		let bad = || format!("bad timecode '{text}'");
		let drop = text.contains(';') || text.contains('.');
		let fields: Vec<i64> = text
			.split([':', ';', '.'])
			.map(|field| field.parse::<i64>().map_err(|_| bad()))
			.collect::<Result<_, _>>()?;
		let [hours, minutes, seconds, frames] = fields[..] else {
			return Err(bad());
		};
		let base = i64::from(rate.timecode_base());
		if minutes >= 60
			|| seconds >= 60
			|| frames >= base
			|| hours < 0
			|| minutes < 0
			|| seconds < 0
			|| frames < 0
		{
			return Err(bad());
		}
		let drop = drop && rate.supports_drop_frame();
		let total_minutes = hours * 60 + minutes;
		let mut count = ((total_minutes * 60) + seconds) * base + frames;
		if drop {
			count -= dropped_per_minute(rate) * (total_minutes - total_minutes / 10);
		}
		Ok((count, drop))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ntsc24() -> Rate {
		Rate::new(24000, 1001).unwrap()
	}

	#[test]
	fn rates_reduce_and_parse() {
		assert_eq!(Rate::new(48000, 2002).unwrap(), ntsc24());
		assert_eq!(Rate::parse("24000/1001").unwrap(), ntsc24());
		assert_eq!(Rate::parse("23.976").unwrap(), ntsc24());
		assert_eq!(
			Rate::parse("29.97").unwrap(),
			Rate::new(30000, 1001).unwrap()
		);
		assert_eq!(Rate::parse("25").unwrap(), Rate::new(25, 1).unwrap());
		assert_eq!(Rate::parse("12.5").unwrap(), Rate::new(25, 2).unwrap());
		assert!(Rate::parse("0/1").is_err());
		assert!(Rate::parse("fast").is_err());
	}

	#[test]
	fn fcpxml_time_is_exact() {
		let rate = ntsc24();
		assert_eq!(rate.frame_duration(), "1001/24000s");
		assert_eq!(rate.rational_seconds(1230), "1231230/24000s");
		assert_eq!(rate.rational_seconds(0), "0s");
		assert_eq!(Rate::new(25, 1).unwrap().rational_seconds(50), "2s");
		assert_eq!(Rate::new(25, 1).unwrap().rational_seconds(51), "51/25s");
	}

	#[test]
	fn seconds_round_the_way_a_cut_needs() {
		let rate = ntsc24();
		assert_eq!(rate.frame_at(rate.seconds(1230), Rounding::Floor), 1230);
		assert_eq!(rate.frame_at(rate.seconds(1230), Rounding::Ceil), 1230);
		assert_eq!(rate.frame_at(51.80, Rounding::Floor), 1241);
		assert_eq!(rate.frame_at(51.80, Rounding::Ceil), 1242);
		assert_eq!(rate.frame_at(51.80, Rounding::Nearest), 1242);
	}

	#[test]
	fn rescale_between_rates() {
		let from = Rate::new(25, 1).unwrap();
		assert_eq!(from.rescale(25, ntsc24(), Rounding::Nearest), 24);
		assert_eq!(from.rescale(137, ntsc24(), Rounding::Nearest), 131);
		assert_eq!(from.rescale(137, from, Rounding::Ceil), 137);
		assert_eq!(from.rescale(1, ntsc24(), Rounding::Floor), 0);
		assert_eq!(from.rescale(1, ntsc24(), Rounding::Ceil), 1);
	}

	#[test]
	fn non_drop_timecode_round_trips() {
		let rate = ntsc24();
		assert_eq!(timecode::format(1230, rate, false), "00:00:51:06");
		assert_eq!(
			timecode::format(24 * 3600 * 14 + 24 * 80, rate, false),
			"14:01:20:00"
		);
		assert_eq!(
			timecode::parse("14:01:20:00", rate).unwrap(),
			(24 * 3600 * 14 + 24 * 80, false)
		);
		// A drop-frame marker on a rate without drop-frame is read as non-drop.
		assert_eq!(timecode::parse("00:00:01;00", rate).unwrap(), (24, false));
	}

	#[test]
	fn drop_frame_timecode_skips_two_numbers_a_minute() {
		let rate = Rate::new(30000, 1001).unwrap();
		assert_eq!(timecode::format(1799, rate, true), "00:00:59;29");
		assert_eq!(timecode::format(1800, rate, true), "00:01:00;02");
		assert_eq!(timecode::format(17982, rate, true), "00:10:00;00");
		assert_eq!(timecode::format(107892, rate, true), "01:00:00;00");
		for frame in [0, 1799, 1800, 17981, 17982, 107892, 123456] {
			let text = timecode::format(frame, rate, true);
			assert_eq!(
				timecode::parse(&text, rate).unwrap(),
				(frame, true),
				"{text}"
			);
		}
		let rate = Rate::new(60000, 1001).unwrap();
		assert_eq!(timecode::format(3600, rate, true), "00:01:00;04");
		assert_eq!(timecode::parse("00:01:00;04", rate).unwrap(), (3600, true));
	}

	#[test]
	fn points_carry_the_start_timecode() {
		let start = timecode::parse("14:01:20:00", ntsc24()).unwrap().0;
		let point = ntsc24().point(1230, start, false);
		assert_eq!(point.timecode, "14:02:11:06");
		assert_eq!(point.seconds, 51.3013);
	}

	#[test]
	fn rates_refuse_zero_when_deserialized() {
		assert!(serde_json::from_str::<Rate>(r#"{"num":0,"den":1}"#).is_err());
		assert_eq!(
			serde_json::from_str::<Rate>(r#"{"num":48000,"den":2002}"#).unwrap(),
			ntsc24()
		);
	}
}
