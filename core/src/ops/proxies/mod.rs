//! # Video proxies and speech audio
//!
//! Footage is read by every media step that follows indexing: transcripts,
//! shot detection, scene descriptions, faces and clip export. Decoding 4K
//! H.265 for each of them is the cost this module removes. One job writes a
//! small H.264 proxy and a 16 kHz mono WAV for each video, keyed by content,
//! and the later steps read those instead of the original.
//!
//! The FFmpeg libraries linked through `sd-ffmpeg` do the work, so it needs
//! no `ffmpeg` executable on the machine. The layout is in
//! `docs/core/virtual-sidecars.mdx` and resolved by
//! [`crate::domain::ContentSidecar`].

pub mod generate;
mod recipe;
