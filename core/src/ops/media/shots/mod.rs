//! # Shot detection
//!
//! `core::ops::media::shots` splits a video into shots with FFmpeg's scene
//! score and writes the result as two content sidecars: a shot list at
//! `shots/default.json` and one keyframe per shot at
//! `keyframes/<shot id>.jpg`. Shots are the unit that scene descriptions,
//! faces and clip export attach to, so a shot's id is derived from the content
//! uuid and its frame range. A rerun that finds the same boundaries keeps the
//! same ids and reuses the keyframes already written.
//!
//! The job reads the 720p proxy when one exists and the original otherwise.
//! A proxy keeps the original's frame timing, so frame numbers mean the same
//! thing either way. Times are whole frames at the original's frame rate, with
//! seconds and timecode beside them for people; see
//! `docs/core/virtual-sidecars.mdx`.
//!
//! Everything here is built only with the `video-shots` cargo feature.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(library: &sd_core::library::Library, path: &std::path::Path) -> std::io::Result<()> {
//! use sd_core::ops::media::shots::shots_of_file;
//!
//! if let Some(list) = shots_of_file(library.path(), path).await? {
//! 	for shot in &list.shots {
//! 		tracing::info!(id = %shot.id, start = shot.in_point.frame, end = shot.out_point.frame, "shot");
//! 	}
//! }
//! # Ok(())
//! # }
//! ```

mod action;
pub mod detect;
mod job;
mod query;
mod sidecar;

pub use action::{ShotDetectAction, ShotDetectInput};
pub use detect::DetectParams;
pub use job::{ShotDetectionJob, ShotDetectionOutput};
pub use query::{ShotListInput, ShotListOutput, ShotListQuery};
pub use sidecar::{
	read_shots, shots_of_file, AnalyzedMedia, FramePoint, FrameRate, Shot, ShotList, SHOTS_VERSION,
};
