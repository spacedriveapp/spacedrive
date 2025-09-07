//! Sea-ORM entity definitions
//!
//! These map our domain models to database tables.

pub mod content_identity;
pub mod content_kind;
pub mod device;
pub mod directory_paths;
pub mod entry;
pub mod entry_closure;
pub mod label;
pub mod location;
pub mod metadata_tag;
pub mod mime_type;
pub mod tag;
pub mod user_metadata;
pub use metadata_tag as user_metadata_tag; // Alias for hierarchical metadata operations
pub mod audit_log;
pub mod collection;
pub mod collection_entry;
pub mod indexer_rule;
pub mod metadata_label;
pub mod sidecar;
pub mod sidecar_availability;
pub mod volume;

// Re-export all entities
pub use audit_log::Entity as AuditLog;
pub use collection::Entity as Collection;
pub use collection_entry::Entity as CollectionEntry;
pub use content_identity::Entity as ContentIdentity;
pub use device::Entity as Device;
pub use directory_paths::Entity as DirectoryPaths;
pub use entry::Entity as Entry;
pub use entry_closure::Entity as EntryClosure;
pub use indexer_rule::Entity as IndexerRule;
pub use label::Entity as Label;
pub use location::Entity as Location;
pub use metadata_tag::Entity as UserMetadataTag;
pub use sidecar::Entity as Sidecar;
pub use sidecar_availability::Entity as SidecarAvailability;
pub use tag::Entity as Tag;
pub use user_metadata::Entity as UserMetadata;
pub use volume::Entity as Volume;

// Re-export active models for easy access
pub use audit_log::ActiveModel as AuditLogActive;
pub use collection::ActiveModel as CollectionActive;
pub use collection_entry::ActiveModel as CollectionEntryActive;
pub use content_identity::ActiveModel as ContentIdentityActive;
pub use device::ActiveModel as DeviceActive;
pub use directory_paths::ActiveModel as DirectoryPathsActive;
pub use entry::ActiveModel as EntryActive;
pub use entry_closure::ActiveModel as EntryClosureActive;
pub use indexer_rule::ActiveModel as IndexerRuleActive;
pub use label::ActiveModel as LabelActive;
pub use location::ActiveModel as LocationActive;
pub use metadata_tag::ActiveModel as UserMetadataTagActive;
pub use sidecar::ActiveModel as SidecarActive;
pub use sidecar_availability::ActiveModel as SidecarAvailabilityActive;
pub use tag::ActiveModel as TagActive;
pub use user_metadata::ActiveModel as UserMetadataActive;
pub use volume::ActiveModel as VolumeActive;
