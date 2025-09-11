//! Volume operations module
//!
//! This module provides operations for managing volumes in Spacedrive:
//! - Tracking/untracking volumes in libraries
//! - Speed testing volume performance

pub mod speed_test;
pub mod track;
pub mod untrack;

pub use speed_test::{action::VolumeSpeedTestAction, VolumeSpeedTestOutput};
pub use track::{action::VolumeTrackAction, VolumeTrackOutput};
pub use untrack::{action::VolumeUntrackAction, VolumeUntrackOutput};
