//! # Media
//!
//! Work on video footage that runs in the daemon, because it needs host
//! FFmpeg and extensions cannot run it, and the readers of what that work
//! wrote. The readers are always compiled, so a build without the footage
//! features still shows sidecars another build made.

#[cfg(feature = "footage-clips")]
pub mod clips;
pub mod shots;
pub mod sidecars;
