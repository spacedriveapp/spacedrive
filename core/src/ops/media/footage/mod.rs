//! # Footage actions
//!
//! `core::ops::media::footage` is the one surface a client uses to run the
//! footage work on a video or a folder of them: transcription, proxies, shot
//! detection, the Videos extension's analysis, and timeline export. Each of
//! those lives behind its own cargo feature, so a client built from the
//! generated types could not otherwise know which of them a daemon has. These
//! ops are registered in every build: `media.footage.capabilities` says what
//! this daemon can run and why not, `media.footage.status` reports what a
//! file already has, and `media.footage.run` and `media.footage.export`
//! hand off to the feature's own action, or answer that the build lacks it.
//!
//! Nothing here runs on its own. Every job starts from a person's request.
//!
//! ## Example
//! ```json
//! { "path": { "Physical": { "device_slug": "studio", "path": "/Footage/day1.mov" } }, "action": "shots" }
//! ```
//! sent as the `media.footage.run` library action queues shot detection on
//! one file and returns its job id.

mod capabilities;
mod export;
mod run;
mod status;

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

pub use capabilities::{
	FootageCapabilities, FootageCapabilitiesInput, FootageCapabilitiesQuery, FootageSupport,
};
pub use export::{FootageExportAction, FootageExportInput, FootageExportOutput};
pub use run::{FootageRunAction, FootageRunInput, FootageRunOutput};
pub use status::{FootageStatusInput, FootageStatusOutput, FootageStatusQuery};

use crate::domain::SdPath;

/// The Videos extension, whose `analyze` job describes each shot.
pub const VIDEOS_EXTENSION_ID: &str = "com.spacedrive.videos";

/// A job a person can start on footage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FootageJob {
	/// Timestamped transcripts with a downloaded Whisper model.
	Transcribe,
	/// A 720p proxy and speech audio.
	Proxy,
	/// The shot list and one keyframe per shot.
	Shots,
	/// Scene descriptions, places and faces per shot, by the Videos extension.
	Analyze,
}

/// A timeline file format an editor opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TimelineFormat {
	/// FCPXML 1.10, for Final Cut Pro and Resolve.
	Fcpxml,
	/// CMX 3600, for Resolve and Premiere Pro.
	Edl,
	/// OpenTimelineIO JSON.
	Otio,
}

impl TimelineFormat {
	pub const ALL: [Self; 3] = [Self::Fcpxml, Self::Edl, Self::Otio];

	pub fn extension(self) -> &'static str {
		match self {
			Self::Fcpxml => "fcpxml",
			Self::Edl => "edl",
			Self::Otio => "otio",
		}
	}
}

/// Which file each clip points the editor at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportMedia {
	#[default]
	Original,
	/// The 720p proxy, which has the original's frames.
	Proxy,
}

/// The path on this device that `path` names. Footage work reads the bytes
/// with FFmpeg and writes content sidecars here, so another device's path
/// is refused rather than guessed at.
pub(crate) fn local_footage_path(path: &SdPath) -> Result<PathBuf, String> {
	let local = path
		.as_local_path()
		.ok_or("Footage work needs a file or folder on this device")?;
	if !local.is_absolute()
		|| local
			.components()
			.any(|part| matches!(part, Component::ParentDir))
	{
		return Err("The path must be absolute, without parent traversal".into());
	}
	Ok(local.to_path_buf())
}

/// Videos at `path`: the file itself, or every video beneath a folder at any
/// depth, hidden ones skipped, in path order so an export follows the folder.
pub(crate) async fn videos_at(path: &Path) -> std::io::Result<Vec<PathBuf>> {
	if tokio::fs::metadata(path).await?.is_file() {
		return Ok(vec![path.to_path_buf()]);
	}
	let mut videos = Vec::new();
	let mut directories = vec![path.to_path_buf()];
	while let Some(directory) = directories.pop() {
		let mut entries = tokio::fs::read_dir(&directory).await?;
		while let Some(entry) = entries.next_entry().await? {
			if entry.file_name().to_string_lossy().starts_with('.') {
				continue;
			}
			let kind = entry.file_type().await?;
			if kind.is_dir() {
				directories.push(entry.path());
			} else if kind.is_file() && crate::service::thumbs::is_video(&entry.path()) {
				videos.push(entry.path());
			}
		}
	}
	videos.sort();
	Ok(videos)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn videos_at_walks_a_folder_in_path_order_and_skips_hidden_files() {
		let root = tempfile::tempdir().unwrap();
		let day = root.path().join("day1");
		tokio::fs::create_dir_all(day.join(".cache")).await.unwrap();
		for path in [
			day.join("b.mov"),
			day.join("a.mp4"),
			day.join("notes.txt"),
			day.join(".hidden.mp4"),
			day.join(".cache/c.mp4"),
			root.path().join("z.mkv"),
		] {
			tokio::fs::write(&path, b"").await.unwrap();
		}

		assert_eq!(
			videos_at(root.path()).await.unwrap(),
			vec![
				day.join("a.mp4"),
				day.join("b.mov"),
				root.path().join("z.mkv")
			]
		);
		assert_eq!(
			videos_at(&day.join("b.mov")).await.unwrap(),
			vec![day.join("b.mov")]
		);
	}

	#[test]
	fn a_path_on_another_device_is_refused() {
		let remote = SdPath::Physical {
			device_slug: "another-device".into(),
			path: "/Footage/a.mp4".into(),
		};
		assert!(local_footage_path(&remote).is_err());
		let footage = std::env::temp_dir().join("Footage");
		assert!(local_footage_path(&SdPath::local(footage.join("..").join("etc"))).is_err());
		assert!(local_footage_path(&SdPath::local("Footage/a.mp4")).is_err());
		assert_eq!(
			local_footage_path(&SdPath::local(footage.join("a.mp4"))).unwrap(),
			footage.join("a.mp4")
		);
	}
}
