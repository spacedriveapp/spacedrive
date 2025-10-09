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

pub mod dependency_graph;
pub mod deterministic;
pub mod fk_mapper;
pub mod hlc;
pub mod peer_log;
pub mod registry;
pub mod syncable;
pub mod transaction;
pub mod transport;

pub use dependency_graph::{compute_sync_order, DependencyError};
pub use deterministic::{
	deterministic_system_album_uuid, deterministic_system_tag_uuid, system_tags,
};
pub use fk_mapper::{convert_fk_to_uuid, map_sync_json_to_local, FKMapping};
pub use hlc::{HLCGenerator, HLC};
pub use peer_log::{ChangeType, PeerLog, PeerLogError, SharedChangeEntry};
pub use registry::{
	apply_shared_change, apply_state_change, compute_registry_sync_order, get_table_name,
	is_device_owned, ApplyError, SyncableModelRegistration,
};
pub use syncable::Syncable;
pub use transaction::{BulkOperation, BulkOperationMetadata, TransactionManager, TxError};
pub use transport::NetworkTransport;
