//! # Timelines for editors
//!
//! `sd-timeline` turns an ordered list of clips into the files video editors
//! import: FCPXML for Final Cut Pro, a CMX 3600 EDL, and OpenTimelineIO JSON.
//! It holds no media code and reads no files, so the core can build a
//! [`Timeline`] from whatever it knows about the footage and the writers stay
//! pure functions that tests can check byte for byte.
//!
//! Every time is a whole frame count at a [`Rate`], a reduced fraction. An
//! editor's in point is exact only when the file says the same frame the agent
//! picked, and floating seconds cannot promise that at 23.976 fps. Seconds and
//! timecode strings are derived from frames, never the other way round, except
//! at the edge where a caller hands in seconds and says how to round.
//!
//! ## Example
//! ```rust
//! use std::path::PathBuf;
//! use sd_timeline::{Clip, Media, Rate, Timeline};
//!
//! let rate = Rate::new(24000, 1001).unwrap();
//! let media = Media {
//!     path: PathBuf::from("/footage/A001_C004.mov"),
//!     name: "A001_C004.mov".into(),
//!     rate,
//!     width: 1920,
//!     height: 1080,
//!     start_timecode: 0,
//!     drop_frame: false,
//!     duration: 21600,
//!     audio: None,
//! };
//! let timeline = Timeline {
//!     name: "Train rough cut".into(),
//!     rate,
//!     width: 1920,
//!     height: 1080,
//!     drop_frame: false,
//!     media: vec![media],
//!     clips: vec![Clip { media: 0, source_in: 1230, source_out: 1302, label: None }],
//! };
//! let fcpxml = sd_timeline::fcpxml::write(&timeline).unwrap();
//! assert!(fcpxml.contains("start=\"1231230/24000s\""));
//! ```

pub mod edl;
pub mod fcpxml;
pub mod otio;
mod time;
mod timeline;

pub use time::{timecode, Rate, Rounding, TimePoint};
pub use timeline::{Audio, Clip, Edit, Media, Timeline, TimelineError};

/// A `file://` URL for an absolute path, with every byte outside the
/// unreserved set and `/` percent-encoded, so spaces and non-ASCII names
/// survive every editor's URL parser.
pub fn file_url(path: &std::path::Path) -> String {
	let text = path.to_string_lossy().replace('\\', "/");
	let mut url = String::from("file://");
	if !text.starts_with('/') {
		url.push('/');
	}
	for byte in text.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
				url.push(byte as char)
			}
			b':' if url.len() <= 9 => url.push(':'),
			_ => url.push_str(&format!("%{byte:02X}")),
		}
	}
	url
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::Path;

	#[test]
	fn file_urls_escape_spaces_and_unicode() {
		assert_eq!(
			file_url(Path::new("/Volumes/Footage/Day 2/Café.mov")),
			"file:///Volumes/Footage/Day%202/Caf%C3%A9.mov"
		);
		assert_eq!(
			file_url(Path::new("C:\\Footage\\a.mov")),
			"file:///C:/Footage/a.mov"
		);
	}
}
