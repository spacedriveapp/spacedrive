//! Sea-ORM entity definitions
//! 
//! These map our domain models to database tables.

pub mod content_kind;
pub mod mime_type;
pub mod device;
pub mod location;
pub mod path_prefix;
pub mod entry;
pub mod user_metadata;
pub mod content_identity;
pub mod tag;
pub mod label;
pub mod metadata_tag;
pub mod metadata_label;

// Re-export all entities
pub use device::Entity as Device;
pub use location::Entity as Location;
pub use path_prefix::Entity as PathPrefix;
pub use entry::Entity as Entry;
pub use user_metadata::Entity as UserMetadata;
pub use content_identity::Entity as ContentIdentity;
pub use tag::Entity as Tag;
pub use label::Entity as Label;

// Re-export active models for easy access
pub use device::ActiveModel as DeviceActive;
pub use location::ActiveModel as LocationActive;
pub use path_prefix::ActiveModel as PathPrefixActive;
pub use entry::ActiveModel as EntryActive;
pub use user_metadata::ActiveModel as UserMetadataActive;
pub use content_identity::ActiveModel as ContentIdentityActive;
pub use tag::ActiveModel as TagActive;
pub use label::ActiveModel as LabelActive;