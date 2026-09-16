//! # Thumbnail hot tier
//!
//! One mmap'd tile cache per volume, filled on demand or by a generation job.
//!
//! Clients ask for the tiles they are about to draw ([`ops::thumbs`]), the
//! daemon bakes what is missing, and every client reads the same result. A GPU
//! client maps `volumes/<id>/thumbs.pvcache` read-only and uploads tiles
//! straight to its atlas; DOM clients read the same slots over loopback HTTP.
//!
//! The daemon owns the writer because the format requires it: pvcache is
//! single-writer by contract and does not enforce it with a lock, so two
//! processes appending slots to one file would corrupt it. [`ThumbService`]
//! holds every writer handle on this machine, which is what makes that
//! contract true rather than hoped for.
//!
//! Viewport requests take priority over generation jobs. Validity includes the
//! file's size and mtime, the recipe revision, and video decoder availability.
//! Completions are announced as batched `thumbnail` resource
//! events, so a client that is showing a placeholder knows when to re-read.

mod ffmpeg;
#[cfg(target_os = "macos")]
mod platform;
mod service;
mod thumbstrip;

pub use service::{
	GenerationOutcome, ThumbService, Thumbnail, ThumbnailGenerationMode, Thumbstrip,
	ThumbstripIdentity, TileIdentity, TILE,
};
