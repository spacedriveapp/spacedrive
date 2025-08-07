//! Sea-ORM entity definitions
//! 
//! These map our domain models to database tables.

pub mod content_kind;
pub mod mime_type;
pub mod device;
pub mod location;
pub mod entry;
pub mod entry_closure;
pub mod directory_paths;
pub mod user_metadata;
pub mod content_identity;
pub mod tag;
pub mod label;
pub mod metadata_tag;
pub use metadata_tag as user_metadata_tag; // Alias for hierarchical metadata operations
pub mod metadata_label;
pub mod audit_log;
pub mod volume;

// Re-export all entities
pub use device::Entity as Device;
pub use location::Entity as Location;
pub use entry::Entity as Entry;
pub use entry_closure::Entity as EntryClosure;
pub use directory_paths::Entity as DirectoryPaths;
pub use user_metadata::Entity as UserMetadata;
pub use content_identity::Entity as ContentIdentity;
pub use tag::Entity as Tag;
pub use label::Entity as Label;
pub use metadata_tag::Entity as UserMetadataTag;
pub use audit_log::Entity as AuditLog;
pub use volume::Entity as Volume;

// Re-export active models for easy access
pub use device::ActiveModel as DeviceActive;
pub use location::ActiveModel as LocationActive;
pub use entry::ActiveModel as EntryActive;
pub use entry_closure::ActiveModel as EntryClosureActive;
pub use directory_paths::ActiveModel as DirectoryPathsActive;
pub use user_metadata::ActiveModel as UserMetadataActive;
pub use content_identity::ActiveModel as ContentIdentityActive;
pub use tag::ActiveModel as TagActive;
pub use label::ActiveModel as LabelActive;
pub use metadata_tag::ActiveModel as UserMetadataTagActive;
pub use audit_log::ActiveModel as AuditLogActive;
pub use volume::ActiveModel as VolumeActive;