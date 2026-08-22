//! Built-in collections: entries identified at index time, never
//! pattern-matched at query time.
//!
//! Classification is a pure function of data the arena already holds (file
//! name + content kind), so flags are recomputed on snapshot restore the
//! same way size rollups are — no snapshot format change, no derived file,
//! and re-running identification is free when heuristics improve.

use crate::domain::ContentKind;

/// Bit assigned to each built-in collection. A `u32` of flags rides per
/// entry; collections compose by mask.
pub const SCREENSHOTS: u32 = 1 << 0;
pub const SCREEN_RECORDINGS: u32 = 1 << 1;

/// The sidebar addresses collections by slug.
pub fn mask_for_slug(slug: &str) -> Option<u32> {
	match slug {
		"screenshots" => Some(SCREENSHOTS),
		"screen-recordings" => Some(SCREEN_RECORDINGS),
		_ => None,
	}
}

pub fn display_name(slug: &str) -> Option<&'static str> {
	match slug {
		"screenshots" => Some("Screenshots"),
		"screen-recordings" => Some("Screen Recordings"),
		_ => None,
	}
}

/// Name prefixes emitted by screenshot tools. Matching is
/// case-insensitive and prefix-anchored; loose substring matching is
/// deliberately avoided (a file merely *about* screenshots is not one).
const SCREENSHOT_PREFIXES: &[&str] = &[
	"screenshot",    // macOS Ventura+, Android, Windows 11
	"screen shot",   // older macOS
	"cleanshot",     // CleanShot X
	"scr-",          // various Android vendors
	"screencapture", // macOS `screencapture` CLI default
	"greenshot",     // Greenshot (Windows)
	"monosnap",      // Monosnap
];

const SCREEN_RECORDING_PREFIXES: &[&str] = &[
	"screen recording", // macOS/iOS
	"screencast",
	"simulator screen recording",
];

/// Flags for one entry, from its file name and content kind.
pub fn classify(file_name: &str, kind: ContentKind) -> u32 {
	let mut flags = 0u32;
	// Prefix checks run on a lowercased copy once; names are short.
	let lower = file_name.to_lowercase();

	if kind == ContentKind::Image && SCREENSHOT_PREFIXES.iter().any(|p| lower.starts_with(p)) {
		flags |= SCREENSHOTS;
	}
	if kind == ContentKind::Video
		&& SCREEN_RECORDING_PREFIXES
			.iter()
			.any(|p| lower.starts_with(p))
	{
		flags |= SCREEN_RECORDINGS;
	}
	flags
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn screenshot_names_classify() {
		for name in [
			"Screenshot 2026-08-19 at 4.32.14 PM.png",
			"Screen Shot 2019-01-01 at 09.00.00.png",
			"CleanShot 2026-08-19 at 12.00.00@2x.png",
			"SCR-20260819-abcd.png",
		] {
			assert_eq!(classify(name, ContentKind::Image), SCREENSHOTS, "{name}");
		}
	}

	#[test]
	fn non_screenshots_do_not() {
		// Right name, wrong kind: a document about screenshots.
		assert_eq!(classify("Screenshot tips.pdf", ContentKind::Document), 0);
		// Wrong name, right kind.
		assert_eq!(classify("IMG_2041.HEIC", ContentKind::Image), 0);
		// Substring but not prefix.
		assert_eq!(classify("my screenshot.png", ContentKind::Image), 0);
	}

	#[test]
	fn screen_recordings_classify() {
		assert_eq!(
			classify(
				"Screen Recording 2026-01-13 at 4.32.14 AM.mov",
				ContentKind::Video
			),
			SCREEN_RECORDINGS
		);
	}
}
