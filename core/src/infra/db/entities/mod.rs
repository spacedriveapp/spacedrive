//! Sea-ORM entity definitions
//!
//! These map our domain models to database tables.

pub mod cloud_credential;
pub mod device;
pub mod device_state_tombstone;

pub mod tag_staging;

pub mod assertion_outbox;
pub mod audit_log;
pub mod source;
pub mod space;
pub mod space_group;
pub mod space_item;
pub mod volume;

// Re-export all entities
pub use audit_log::Entity as AuditLog;
pub use cloud_credential::Entity as CloudCredential;
pub use device::Entity as Device;
pub use device_state_tombstone::Entity as DeviceStateTombstone;
pub use source::Entity as Source;
pub use space::Entity as Space;
pub use space_group::Entity as SpaceGroup;
pub use space_item::Entity as SpaceItem;
pub use volume::Entity as Volume;

// Re-export active models for easy access
pub use audit_log::ActiveModel as AuditLogActive;
pub use cloud_credential::ActiveModel as CloudCredentialActive;
pub use device::ActiveModel as DeviceActive;
pub use device_state_tombstone::ActiveModel as DeviceStateTombstoneActive;
pub use space::ActiveModel as SpaceActive;
pub use space_group::ActiveModel as SpaceGroupActive;
pub use space_item::ActiveModel as SpaceItemActive;
pub use volume::ActiveModel as VolumeActive;
