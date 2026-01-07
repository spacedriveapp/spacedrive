//! Volume operations module
//!
//! This module provides operations for managing volumes in Spacedrive:
//! - Tracking/untracking volumes in libraries
//! - Speed testing volume performance
//! - Adding/removing cloud volumes
//! - Listing volumes
//! - Ephemeral indexing entire volumes
//! - Querying encryption status for secure delete optimization

pub mod add_cloud;
pub mod encryption;
pub mod index;
pub mod list;
pub mod refresh;
pub mod remove_cloud;
pub mod speed_test;
pub mod track;
pub mod untrack;

pub use add_cloud::{action::VolumeAddCloudAction, VolumeAddCloudOutput};
pub use encryption::{
	PathEncryptionInfo, VolumeEncryptionOutput, VolumeEncryptionQuery, VolumeEncryptionQueryInput,
};
pub use index::{IndexVolumeAction, IndexVolumeInput, IndexVolumeOutput};
pub use list::{
	VolumeEncryptionInfo, VolumeFilter, VolumeListOutput, VolumeListQuery, VolumeListQueryInput,
};
pub use refresh::{action::VolumeRefreshAction, VolumeRefreshOutput};
pub use remove_cloud::{action::VolumeRemoveCloudAction, VolumeRemoveCloudOutput};
pub use speed_test::{action::VolumeSpeedTestAction, VolumeSpeedTestOutput};
pub use track::{action::VolumeTrackAction, VolumeTrackOutput};
pub use untrack::{action::VolumeUntrackAction, VolumeUntrackOutput};

// Register volume indexing action
crate::register_library_action!(index::IndexVolumeAction, "volumes.index");
