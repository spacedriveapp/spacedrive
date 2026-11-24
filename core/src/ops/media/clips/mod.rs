//! # Footage clips
//!
//! `core::ops::media::clips` answers footage questions with clips: a range of
//! one video, the words spoken in it, and the shot it sits in. Three ops make
//! up the surface the CLI, the generated clients and the agent MCP share:
//! `media.clips.search` finds the moments where words were said,
//! `media.footage.read` reads one video's shots and transcript, and
//! `media.timeline.export` turns an ordered list of clips into FCPXML, a CMX
//! 3600 EDL and OpenTimelineIO that reference the originals or their proxies.
//!
//! Nothing here stores clips. A clip is computed from the sidecars of the
//! video's content uuid: the transcript, the shot list and the proxy. Every
//! time is a whole frame at the original's frame rate, so an in point an agent
//! reads is the frame an editor opens on. `docs/core/footage-clips.mdx` is the
//! contract, and `sd-timeline` writes the files.
//!
//! Everything here is built only with the `footage-clips` cargo feature.
//!
//! ## Example
//! ```json
//! { "scope": { "Physical": { "device_slug": "studio", "path": "/Footage/japan" } }, "query": "train" }
//! ```
//! sent as `media.clips.search` returns one clip per moment "train" was said.

mod export;
mod footage;
mod read;
mod search;

pub(crate) use export::render as render_timeline;
pub use export::{
	ExportClip, ExportMedia, TimelineDocument, TimelineExportAction, TimelineExportInput,
	TimelineExportOutput, TimelineFormat, TimelineSummary, WrittenTimeline, MAX_EXPORT_CLIPS,
};
pub use footage::TimeInput;
pub(crate) use footage::{display_name, Footage};
pub use read::{
	FootageReadInput, FootageReadOutput, FootageReadQuery, FootageSegment, FootageShot,
	MAX_READ_SEGMENTS,
};
pub use search::{
	Clip, ClipSearchInput, ClipSearchOutput, ClipSearchQuery, ClipSnap, ClipTranscript,
	DEFAULT_PAD_SECONDS, MAX_PAD_SECONDS,
};
