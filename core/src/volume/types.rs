//! Volume type definitions
//!
//! This module re-exports volume types from the domain module.
//! The types are now unified in domain::volume.

// Re-export all volume types from domain
pub use crate::domain::volume::{
	ApfsContainer, ApfsVolumeInfo, ApfsVolumeRole, DiskType, EncryptionType, FileSystem, MountType,
	PathMapping, SpacedriveVolumeId, TrackedVolume, Volume, VolumeDetectionConfig, VolumeEncryption,
	VolumeEvent, VolumeFingerprint, VolumeInfo, VolumeType,
};
