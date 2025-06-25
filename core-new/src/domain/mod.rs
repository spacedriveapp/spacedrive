//! Core domain models - the heart of Spacedrive's VDFS
//! 
//! These models implement the new file data model design where:
//! - Entry represents any file/directory
//! - UserMetadata is always present (enabling immediate tagging)
//! - ContentIdentity is optional (for deduplication)

pub mod content_identity;
pub mod device;
pub mod entry;
pub mod location;
pub mod user_metadata;
pub mod volume;

// Re-export commonly used types
pub use content_identity::{ContentKind, MediaData, ContentHashGenerator, ContentHashError};
pub use device::{Device, OperatingSystem};
pub use entry::{Entry, EntryKind, SdPathSerialized};
pub use location::{Location, IndexMode, ScanState};
pub use user_metadata::{UserMetadata, Tag, Label};
pub use volume::{Volume as DomainVolume, VolumeType, MountType as DomainMountType, DiskType as DomainDiskType, FileSystem as DomainFileSystem};