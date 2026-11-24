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
//! The shot list format in `sidecar` is always compiled, so readers such as
//! `media.sidecars` need no FFmpeg. Detection, its job and its ops are built
//! only with the `video-shots` cargo feature.
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

#[cfg(feature = "video-shots")]
mod action;
#[cfg(feature = "video-shots")]
pub mod detect;
#[cfg(feature = "video-shots")]
mod job;
#[cfg(feature = "video-shots")]
mod query;
mod sidecar;

#[cfg(feature = "video-shots")]
pub use action::{ShotDetectAction, ShotDetectInput};
#[cfg(feature = "video-shots")]
pub use detect::DetectParams;
#[cfg(feature = "video-shots")]
pub use job::{ShotDetectionJob, ShotDetectionOutput};
#[cfg(feature = "video-shots")]
pub use query::{ShotListInput, ShotListOutput, ShotListQuery};
pub use sidecar::{
	content_uuid_of_video, read_shots, shots_of_file, AnalyzedMedia, FramePoint, FrameRate, Shot,
	ShotList, SHOTS_VERSION,
};
