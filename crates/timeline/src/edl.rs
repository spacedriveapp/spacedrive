//! CMX 3600 EDL, which Resolve and Premiere Pro import.
//!
//! An EDL names media by reel, not by path. Each event also carries a
//! `* FROM CLIP NAME` and a `* SOURCE FILE` comment, which is what Resolve and
//! Premiere relink by. Source timecode is each file's own, at its own rate;
//! record timecode starts at `00:00:00:00` at the timeline's rate.

use std::fmt::Write;

use crate::{timecode, Timeline, TimelineError};

/// Longest reel name written. CMX 3600 allows 8, but every editor this
/// targets reads 32, and 8 characters cannot tell camera files apart.
pub const REEL_MAX: usize = 32;

/// A reel name from a file name: its stem, with characters outside
/// `A-Z a-z 0-9 _` replaced by `_`.
pub fn reel_name(file_name: &str) -> String {
	let stem = std::path::Path::new(file_name)
		.file_stem()
		.map(|stem| stem.to_string_lossy().into_owned())
		.unwrap_or_default();
	let reel: String = stem
		.chars()
		.map(|c| {
			if c.is_ascii_alphanumeric() || c == '_' {
				c
			} else {
				'_'
			}
		})
		.take(REEL_MAX)
		.collect();
	if reel.is_empty() {
		"AX".to_string()
	} else {
		reel
	}
}

/// Comments end at the line, so a name with a line break cannot start a new
/// event.
fn one_line(text: &str) -> String {
	text.chars()
		.map(|c| if c.is_control() { ' ' } else { c })
		.collect()
}

pub fn write(timeline: &Timeline) -> Result<String, TimelineError> {
	let edits = timeline.edits()?;
	let mut edl = String::new();
	let _ = writeln!(edl, "TITLE: {}", one_line(&timeline.name));
	let drop = timeline.drop_frame && timeline.rate.supports_drop_frame();
	let _ = writeln!(
		edl,
		"FCM: {}",
		if drop { "DROP FRAME" } else { "NON-DROP FRAME" }
	);
	for (index, edit) in edits.iter().enumerate() {
		let media = edit.media;
		let source_tc = |frame: i64| {
			timecode::format(media.start_timecode + frame, media.rate, media.drop_frame)
		};
		let record_tc = |frame: i64| timecode::format(frame, timeline.rate, drop);
		let track = if media.audio.is_some() { "AA/V" } else { "V" };
		let _ = writeln!(
			edl,
			"\n{:03}  {:<8} {:<5} C        {} {} {} {}",
			index + 1,
			reel_name(&media.name),
			track,
			source_tc(edit.clip.source_in),
			source_tc(edit.clip.source_out),
			record_tc(edit.record_in),
			record_tc(edit.record_out),
		);
		let _ = writeln!(edl, "* FROM CLIP NAME: {}", one_line(&media.name));
		if let Some(label) = &edit.clip.label {
			let _ = writeln!(edl, "* COMMENT: {}", one_line(label));
		}
		let _ = writeln!(
			edl,
			"* SOURCE FILE: {}",
			one_line(&media.path.to_string_lossy())
		);
	}
	Ok(edl)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::timeline::tests::sample;

	#[test]
	fn events_carry_source_and_record_timecode() {
		let edl = write(&sample()).unwrap();
		let expected = "TITLE: Train <rough> cut
FCM: NON-DROP FRAME

001  A001_C004 AA/V  C        14:02:11:06 14:02:14:06 00:00:00:00 00:00:03:00
* FROM CLIP NAME: A001_C004.mov
* SOURCE FILE: /Volumes/Footage/Japan Trip/A001_C004.mov

002  A003_C011___co V     C        00:00:12:13 00:00:18:00 00:00:03:00 00:00:08:11
* FROM CLIP NAME: A003_C011 & co.mov
* COMMENT: platform wide
* SOURCE FILE: /Volumes/Footage/Japan Trip/A003_C011 & co.mov

003  A001_C004 AA/V  C        14:02:43:08 14:02:45:08 00:00:08:11 00:00:10:11
* FROM CLIP NAME: A001_C004.mov
* SOURCE FILE: /Volumes/Footage/Japan Trip/A001_C004.mov
";
		assert_eq!(edl, expected);
	}

	#[test]
	fn reel_names_are_safe_and_bounded() {
		assert_eq!(reel_name("A001_C004.mov"), "A001_C004");
		assert_eq!(reel_name("día 2.mp4"), "d_a_2");
		assert_eq!(reel_name(".mov"), "_mov");
		assert_eq!(reel_name(&"x".repeat(40)).len(), REEL_MAX);
	}
}
