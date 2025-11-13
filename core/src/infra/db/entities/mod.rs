//! Sea-ORM entity definitions
//!
//! These map our domain models to database tables.

pub mod audio_media_data;
pub mod content_identity;
pub mod content_kind;
pub mod device;
pub mod device_state_tombstone;
pub mod directory_paths;
pub mod entry;
pub mod entry_closure;
pub mod image_media_data;
pub mod location;
pub mod mime_type;
pub mod user_metadata;

// Tagging system
pub mod tag;
pub mod tag_closure;
pub mod tag_relationship;
pub mod tag_usage_pattern;
pub mod user_metadata_tag;

pub mod audit_log;
pub mod collection;
pub mod collection_entry;
pub mod indexer_rule;
pub mod sidecar;
pub mod sidecar_availability;
pub mod space;
pub mod space_group;
pub mod space_item;
pub mod sync_conduit;
pub mod sync_generation;
pub mod video_media_data;
pub mod volume;

// Re-export all entities
pub use audio_media_data::Entity as AudioMediaData;
pub use audit_log::Entity as AuditLog;
pub use collection::Entity as Collection;
pub use collection_entry::Entity as CollectionEntry;
pub use content_identity::Entity as ContentIdentity;
pub use device::Entity as Device;
pub use device_state_tombstone::Entity as DeviceStateTombstone;
pub use directory_paths::Entity as DirectoryPaths;
pub use entry::Entity as Entry;
pub use entry_closure::Entity as EntryClosure;
pub use image_media_data::Entity as ImageMediaData;
pub use indexer_rule::Entity as IndexerRule;
pub use location::Entity as Location;
pub use sidecar::Entity as Sidecar;
pub use sidecar_availability::Entity as SidecarAvailability;
pub use space::Entity as Space;
pub use space_group::Entity as SpaceGroup;
pub use space_item::Entity as SpaceItem;
pub use sync_conduit::Entity as SyncConduit;
pub use sync_generation::Entity as SyncGeneration;
pub use user_metadata::Entity as UserMetadata;
pub use video_media_data::Entity as VideoMediaData;
pub use volume::Entity as Volume;

// Tagging entities
pub use tag::Entity as Tag;
pub use tag_closure::Entity as TagClosure;
pub use tag_relationship::Entity as TagRelationship;
pub use tag_usage_pattern::Entity as TagUsagePattern;
pub use user_metadata_tag::Entity as UserMetadataTag;

// Re-export active models for easy access
pub use audio_media_data::ActiveModel as AudioMediaDataActive;
pub use audit_log::ActiveModel as AuditLogActive;
pub use collection::ActiveModel as CollectionActive;
pub use collection_entry::ActiveModel as CollectionEntryActive;
pub use content_identity::ActiveModel as ContentIdentityActive;
pub use device::ActiveModel as DeviceActive;
pub use device_state_tombstone::ActiveModel as DeviceStateTombstoneActive;
pub use directory_paths::ActiveModel as DirectoryPathsActive;
pub use entry::ActiveModel as EntryActive;
pub use entry_closure::ActiveModel as EntryClosureActive;
pub use image_media_data::ActiveModel as ImageMediaDataActive;
pub use indexer_rule::ActiveModel as IndexerRuleActive;
pub use location::ActiveModel as LocationActive;
pub use sidecar::ActiveModel as SidecarActive;
pub use sidecar_availability::ActiveModel as SidecarAvailabilityActive;
pub use space::ActiveModel as SpaceActive;
pub use space_group::ActiveModel as SpaceGroupActive;
pub use space_item::ActiveModel as SpaceItemActive;
pub use sync_conduit::ActiveModel as SyncConduitActive;
pub use sync_generation::ActiveModel as SyncGenerationActive;
pub use user_metadata::ActiveModel as UserMetadataActive;
pub use video_media_data::ActiveModel as VideoMediaDataActive;
pub use volume::ActiveModel as VolumeActive;

// Tagging active models
pub use tag::ActiveModel as TagActive;
pub use tag_closure::ActiveModel as TagClosureActive;
pub use tag_relationship::ActiveModel as TagRelationshipActive;
pub use tag_usage_pattern::ActiveModel as TagUsagePatternActive;
pub use user_metadata_tag::ActiveModel as UserMetadataTagActive;
