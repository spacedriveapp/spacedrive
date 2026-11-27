//! Sea-ORM entity definitions
//!
//! These map our domain models to database tables.

pub mod cloud_credential;
pub mod device;
pub mod device_state_tombstone;

pub mod tag_staging;

pub mod actor;
pub mod agent_grant;
pub mod agent_grant_spent_refresh;
pub mod assertion_outbox;
pub mod audit_log;
pub mod grant_audit;
pub mod layout;
pub mod layout_group;
pub mod layout_item;
pub mod source;
pub mod volume;

// Re-export all entities
pub use audit_log::Entity as AuditLog;
pub use cloud_credential::Entity as CloudCredential;
pub use device::Entity as Device;
pub use device_state_tombstone::Entity as DeviceStateTombstone;
pub use layout::Entity as Layout;
pub use layout_group::Entity as LayoutGroup;
pub use layout_item::Entity as LayoutItem;
pub use source::Entity as Source;
pub use volume::Entity as Volume;

// Re-export active models for easy access
pub use audit_log::ActiveModel as AuditLogActive;
pub use cloud_credential::ActiveModel as CloudCredentialActive;
pub use device::ActiveModel as DeviceActive;
pub use device_state_tombstone::ActiveModel as DeviceStateTombstoneActive;
pub use layout::ActiveModel as LayoutActive;
pub use layout_group::ActiveModel as LayoutGroupActive;
pub use layout_item::ActiveModel as LayoutItemActive;
pub use volume::ActiveModel as VolumeActive;
