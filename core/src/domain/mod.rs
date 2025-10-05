//! Core domain models - the heart of Spacedrive's VDFS
//!
//! These models implement the new file data model design where:
//! - Entry represents any file/directory
//! - UserMetadata is always present (enabling immediate tagging)
//! - ContentIdentity is optional (for deduplication)

pub mod addressing;
pub mod content_identity;
pub mod device;
pub mod entry;
pub mod file;
pub mod location;
pub mod tag;
pub mod user_metadata;
pub mod volume;

// Re-export commonly used types
pub use addressing::{PathResolutionError, SdPath, SdPathBatch, SdPathParseError};
pub use content_identity::{
	ContentHashError, ContentHashGenerator, ContentIdentity, ContentKind, MediaData,
};
pub use device::{Device, OperatingSystem};
pub use entry::{Entry, EntryKind, SdPathSerialized};
pub use file::{File, FileConstructionData, Sidecar};
pub use location::{IndexMode, Location, ScanState};
pub use tag::{
	OrganizationalPattern, PatternType, PrivacyLevel, RelationshipType, Tag, TagApplication,
	TagError, TagRelationship, TagSource, TagType,
};
pub use user_metadata::{Label, Tag as UserMetadataTag, UserMetadata};
pub use volume::{
	DiskType as DomainDiskType, FileSystem as DomainFileSystem, MountType as DomainMountType,
	Volume as DomainVolume, VolumeType,
};
