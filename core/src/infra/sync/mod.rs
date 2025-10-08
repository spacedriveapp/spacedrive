//! Sync infrastructure
//!
//! This module contains the core sync infrastructure including the sync log database,
//! sync log entity, and sync-related types and utilities.

pub mod leader;
pub mod registry;
pub mod sync_log_db;
pub mod sync_log_entity;
pub mod sync_log_migration;
pub mod syncable;
pub mod transaction_manager;

pub use leader::{LeadershipManager, SyncLeadership, SyncRole};
pub use registry::{apply_sync_entry, get_registry, SyncableModelRegistration};
pub use sync_log_db::{SyncLogDb, SyncLogError};
pub use sync_log_entity::{ChangeType, SyncLogEntry, SyncLogModel};
pub use syncable::Syncable;
pub use transaction_manager::{BulkOperation, BulkOperationMetadata, TransactionManager, TxError};
