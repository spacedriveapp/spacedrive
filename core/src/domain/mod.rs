//! Core domain models - the heart of Spacedrive's VDFS
//!
//! These models implement the new file data model design where:
//! - File represents any file/directory with rich metadata
//! - UserMetadata is always present (enabling immediate tagging)
//! - ContentIdentity is optional (for deduplication)

pub mod addressing;
pub mod content_identity;
pub mod device;
pub mod file;
pub mod location;
pub mod resource;
pub mod resource_manager;
pub mod space;
pub mod tag;
pub mod user_metadata;
pub mod volume;

// Re-export commonly used types
pub use addressing::{PathResolutionError, SdPath, SdPathBatch, SdPathParseError};
pub use content_identity::{ContentHashError, ContentHashGenerator, ContentIdentity, ContentKind};
pub use device::{Device, OperatingSystem};
pub use file::{EntryKind, File, Sidecar};
pub use location::{IndexMode, Location, ScanState};
pub use resource::Identifiable;
pub use resource_manager::ResourceManager;
pub use space::{
	GroupType, ItemType, Space, SpaceGroup, SpaceGroupWithItems, SpaceItem, SpaceLayout,
};
pub use tag::{
	OrganizationalPattern, PatternType, PrivacyLevel, RelationshipType, Tag, TagApplication,
	TagError, TagRelationship, TagSource, TagType,
};
pub use user_metadata::UserMetadata;
pub use volume::{
	DiskType as DomainDiskType, FileSystem as DomainFileSystem, MountType as DomainMountType,
	Volume, VolumeType,
};
