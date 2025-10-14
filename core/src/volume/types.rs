//! Volume type definitions
//!
//! This module re-exports volume types from the domain module.
//! The types are now unified in domain::volume.

// Re-export all volume types from domain
pub use crate::domain::volume::{
	ApfsContainer, ApfsVolumeInfo, ApfsVolumeRole, DiskType, FileSystem, MountType, PathMapping,
	SpacedriveVolumeId, TrackedVolume, Volume, VolumeDetectionConfig, VolumeEvent,
	VolumeFingerprint, VolumeInfo, VolumeType,
};
