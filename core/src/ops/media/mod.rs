//! # Media
//!
//! Work on video footage that runs in the daemon, because it needs host
//! FFmpeg and extensions cannot run it, and the readers of what that work
//! wrote. The readers and `footage` are always compiled, so clients can
//! always ask what a daemon can do and a build without the footage features
//! still shows sidecars another build made. The jobs `footage` dispatches to
//! are behind their cargo features.

#[cfg(feature = "footage-clips")]
pub mod clips;
pub mod footage;
pub mod shots;
pub mod sidecars;
