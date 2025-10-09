//! Sync infrastructure (Leaderless Hybrid Architecture)
//!
//! Core sync components for peer-to-peer synchronization:
//! - HLC for distributed ordering
//! - Per-peer logs for shared resource changes
//! - Syncable trait for model registration
//! - Transaction manager for atomic commits
//!
//! Legacy files (leader-based, will be removed):
//! - legacy_sync_log_* (deprecated)

pub mod deterministic;
pub mod hlc;
pub mod peer_log;
pub mod registry;
pub mod syncable;
pub mod transaction;

pub use deterministic::{deterministic_album_uuid, deterministic_tag_uuid};
pub use hlc::{HLCGenerator, HLC};
pub use peer_log::{ChangeType, PeerLog, PeerLogError, SharedChangeEntry};
pub use registry::{
	apply_shared_change, apply_state_change, get_table_name, is_device_owned, ApplyError,
	SyncableModelRegistration,
};
pub use syncable::Syncable;
pub use transaction::{BulkOperation, BulkOperationMetadata, TransactionManager, TxError};
