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
pub mod library;
pub mod location;
pub mod media_data;
pub mod resource;
pub mod resource_manager;
pub mod resource_registry;
pub mod sidecar;
pub mod space;
pub mod tag;
pub mod volume;

// Re-export commonly used types
pub use addressing::{PathResolutionError, SdPath, SdPathBatch, SdPathParseError};
pub use content_identity::{ContentHashError, ContentHashGenerator, ContentIdentity, ContentKind};
pub use device::{ConnectionMethod, Device, OperatingSystem};
pub use file::{EntryKind, File, Sidecar};
pub use library::Library;
pub use location::Location;
pub use media_data::{AudioMediaData, ImageMediaData, VideoMediaData};
pub use resource::{EventEmitter, Identifiable};
pub use resource_manager::ResourceManager;
pub use sidecar::{SidecarFormat, SidecarKind, SidecarStatus, SidecarVariant};
pub use space::{
	GroupType, ItemType, Space, SpaceGroup, SpaceGroupWithItems, SpaceItem, SpaceLayout,
};
pub use tag::{Tag, TagError};
pub use volume::{
	DiskType as DomainDiskType, FileSystem as DomainFileSystem, MountType as DomainMountType,
	Volume, VolumeType,
};
