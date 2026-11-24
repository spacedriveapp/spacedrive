//! # Media
//!
//! Work on video footage that runs in the daemon, because it needs host
//! FFmpeg and extensions cannot run it. `footage` is built in every
//! configuration so clients can always ask what a daemon can do; the parts it
//! dispatches to are behind their cargo features.

#[cfg(feature = "footage-clips")]
pub mod clips;
pub mod footage;
#[cfg(feature = "video-shots")]
pub mod shots;
