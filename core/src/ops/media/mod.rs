//! # Media
//!
//! Work on video footage that runs in the daemon, because it needs host
//! FFmpeg and extensions cannot run it.

#[cfg(feature = "footage-clips")]
pub mod clips;
pub mod shots;
