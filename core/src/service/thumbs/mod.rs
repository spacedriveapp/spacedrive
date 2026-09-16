//! The thumbnail hot tier: one mmap'd tile cache per source, filled on demand.
//!
//! Clients ask for the tiles they are about to draw ([`ops::thumbs`]), the
//! daemon bakes what is missing, and every client reads the same result. A GPU
//! client maps `sources/<id>/thumbs.pvcache` read-only and uploads tiles
//! straight to its atlas; DOM clients read the same slots over loopback HTTP.
//!
//! The daemon owns the writer because the format requires it: pvcache is
//! single-writer by contract and does not enforce it with a lock, so two
//! processes appending slots to one file would corrupt it. [`ThumbService`]
//! holds every writer handle on this machine, which is what makes that
//! contract true rather than hoped for.
//!
//! Nothing is baked speculatively. A request names the paths in view, in
//! priority order, and a tile already fresh for its `(size, mtime)` version
//! costs a lookup. Completions are announced as batched `thumbnail` resource
//! events, so a client that is showing a placeholder knows when to re-read.

mod ffmpeg;
#[cfg(target_os = "macos")]
mod platform;
mod service;
mod thumbstrip;

pub use service::{ThumbService, Thumbnail, Thumbstrip, ThumbstripIdentity, TileIdentity, TILE};
