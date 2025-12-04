//! Sync infrastructure (Leaderless Hybrid Architecture)
//!
//! Core sync components for peer-to-peer synchronization:
//! - HLC for distributed ordering
//! - Per-peer logs for shared resource changes
//! - Syncable trait for model registration
//! - Transaction manager for atomic commits
//! - Unified configuration for all sync behavior
//! - Per-resource watermarks for incremental sync
//! - Checkpoint persistence for resumable backfill
//!

pub mod checkpoints;
pub mod config;
pub mod dependency_graph;
pub mod deterministic;
pub mod event_bus;
pub mod event_log;
pub mod fk_mapper;
pub mod hlc;
pub mod peer_log;
pub mod peer_watermarks;
pub mod registry;
pub mod syncable;
pub mod transaction;
pub mod transport;
pub mod watermarks;

pub use checkpoints::{BackfillCheckpoint, BackfillCheckpointStore, CheckpointError};
pub use config::{
	BatchingConfig, MonitoringConfig, NetworkConfig, PruningStrategy, RetentionConfig, SyncConfig,
};
pub use dependency_graph::{compute_sync_order, DependencyError};
pub use deterministic::{
	deterministic_library_default_uuid, deterministic_system_album_uuid,
	deterministic_system_tag_uuid, system_tags,
};
pub use event_bus::{SyncEvent, SyncEventBus};
pub use event_log::{
	BatchAggregator, BatchAggregatorConfig, EventCategory, EventSeverity, SyncEventLog,
	SyncEventLogger, SyncEventQuery, SyncEventType,
};
pub use fk_mapper::{
	batch_map_sync_json_to_local, convert_fk_to_uuid, map_sync_json_to_local, BatchFkMapResult,
	FKMapping,
};
pub use hlc::{HLCGenerator, HLC};
pub use peer_log::{ChangeType, PeerLog, PeerLogError, SharedChangeEntry};
pub use peer_watermarks::PeerWatermarkStore;
pub use registry::{
	apply_shared_change, apply_state_change, compute_registry_sync_order, get_fk_mappings,
	get_table_name, is_device_owned, ApplyError, SyncableInventoryEntry, SyncableModelRegistration,
};
pub use syncable::Syncable;
pub use transaction::{BulkOperation, BulkOperationMetadata, TransactionManager, TxError};
pub use transport::NetworkTransport;
pub use watermarks::{ResourceWatermarkStore, WatermarkError};
