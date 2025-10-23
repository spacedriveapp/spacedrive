//! Unified configuration for library sync behavior
//!
//! Controls all timing, batching, and retention parameters across
//! both device-owned (state-based) and shared (log-based) sync.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unified configuration for library sync behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
	/// Batching and performance settings
	pub batching: BatchingConfig,

	/// Retention and pruning settings
	pub retention: RetentionConfig,

	/// Network and timing settings
	pub network: NetworkConfig,

	/// Monitoring and health check settings
	pub monitoring: MonitoringConfig,
}

impl Default for SyncConfig {
	fn default() -> Self {
		Self {
			batching: BatchingConfig::default(),
			retention: RetentionConfig::default(),
			network: NetworkConfig::default(),
			monitoring: MonitoringConfig::default(),
		}
	}
}

impl SyncConfig {
	/// Aggressive sync for fast local networks
	///
	/// Optimized for:
	/// - Fast LAN connections
	/// - Always-online devices
	/// - Minimal storage overhead
	/// - Low latency
	pub fn aggressive() -> Self {
		Self {
			batching: BatchingConfig {
				backfill_batch_size: 5_000,
				state_broadcast_batch_size: 500,
				shared_broadcast_batch_size: 50,
				max_snapshot_size: 50_000,
			},
			retention: RetentionConfig {
				strategy: PruningStrategy::AcknowledgmentBased,
				tombstone_max_retention_days: 3,
				peer_log_max_retention_days: 3,
				force_full_sync_threshold_days: 2,
			},
			network: NetworkConfig {
				message_timeout_secs: 15,
				backfill_request_timeout_secs: 30,
				sync_loop_interval_secs: 2,
				connection_check_interval_secs: 5,
			},
			monitoring: MonitoringConfig {
				pruning_interval_secs: 1800,
				enable_metrics: true,
				metrics_log_interval_secs: 60,
			},
		}
	}

	/// Conservative sync for unreliable networks
	///
	/// Optimized for:
	/// - Unreliable network connections
	/// - Frequently offline devices
	/// - Large batch efficiency
	/// - Extended retention
	pub fn conservative() -> Self {
		Self {
			batching: BatchingConfig {
				backfill_batch_size: 25_000,
				state_broadcast_batch_size: 2_000,
				shared_broadcast_batch_size: 200,
				max_snapshot_size: 200_000,
			},
			retention: RetentionConfig {
				strategy: PruningStrategy::Conservative {
					min_retention_days: 7,
				},
				tombstone_max_retention_days: 30,
				peer_log_max_retention_days: 30,
				force_full_sync_threshold_days: 25,
			},
			network: NetworkConfig {
				message_timeout_secs: 60,
				backfill_request_timeout_secs: 120,
				sync_loop_interval_secs: 10,
				connection_check_interval_secs: 30,
			},
			monitoring: MonitoringConfig {
				pruning_interval_secs: 7200,
				enable_metrics: true,
				metrics_log_interval_secs: 600,
			},
		}
	}

	/// Mobile-optimized sync
	///
	/// Optimized for:
	/// - Battery life
	/// - Bandwidth conservation
	/// - Background operation
	/// - Less frequent sync checks
	pub fn mobile() -> Self {
		Self {
			batching: BatchingConfig {
				backfill_batch_size: 5_000,
				state_broadcast_batch_size: 500,
				shared_broadcast_batch_size: 50,
				max_snapshot_size: 50_000,
			},
			retention: RetentionConfig {
				strategy: PruningStrategy::TimeBased { retention_days: 14 },
				tombstone_max_retention_days: 14,
				peer_log_max_retention_days: 14,
				force_full_sync_threshold_days: 10,
			},
			network: NetworkConfig {
				message_timeout_secs: 45,
				backfill_request_timeout_secs: 90,
				sync_loop_interval_secs: 30,
				connection_check_interval_secs: 60,
			},
			monitoring: MonitoringConfig {
				pruning_interval_secs: 14400,
				enable_metrics: false,
				metrics_log_interval_secs: 1800,
			},
		}
	}
}

/// Batching configuration for sync operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
	/// Records per batch for backfill requests
	///
	/// Used for: StateRequest batch_size, SharedChangeRequest limit
	/// Default: 10,000
	pub backfill_batch_size: usize,

	/// Records per batch for state broadcast
	///
	/// Used for: StateBatch messages during indexing
	/// Default: 1,000
	pub state_broadcast_batch_size: usize,

	/// Records per batch for shared resource broadcast
	///
	/// Used for: SharedChangeBatch messages
	/// Default: 100
	pub shared_broadcast_batch_size: usize,

	/// Maximum snapshot size for current state
	///
	/// Used for: SharedChangeResponse.current_state limit
	/// Default: 100,000
	pub max_snapshot_size: usize,
}

impl Default for BatchingConfig {
	fn default() -> Self {
		Self {
			backfill_batch_size: 10_000,
			state_broadcast_batch_size: 1_000,
			shared_broadcast_batch_size: 100,
			max_snapshot_size: 100_000,
		}
	}
}

/// Retention and pruning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
	/// Pruning strategy for sync coordination data
	pub strategy: PruningStrategy,

	/// Maximum retention for tombstones (days)
	///
	/// Prevents offline devices from blocking pruning forever.
	/// Default: 7 days
	pub tombstone_max_retention_days: u32,

	/// Maximum retention for peer log entries (days)
	///
	/// Prevents offline devices from blocking pruning forever.
	/// Default: 7 days
	pub peer_log_max_retention_days: u32,

	/// Force full sync if watermark older than this (days)
	///
	/// If device watermark is older than this threshold, skip incremental
	/// sync and do full backfill to ensure consistency.
	/// Default: 25 days
	pub force_full_sync_threshold_days: u32,
}

impl Default for RetentionConfig {
	fn default() -> Self {
		Self {
			strategy: PruningStrategy::AcknowledgmentBased,
			tombstone_max_retention_days: 7,
			peer_log_max_retention_days: 7,
			force_full_sync_threshold_days: 25,
		}
	}
}

/// Pruning strategy for sync coordination data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PruningStrategy {
	/// Prune as soon as all devices acknowledge (minimal storage)
	AcknowledgmentBased,

	/// Keep for minimum duration even if acknowledged (safety buffer)
	Conservative { min_retention_days: u32 },

	/// Always keep for fixed duration (ignore acknowledgments)
	TimeBased { retention_days: u32 },
}

/// Network timing and timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
	/// Timeout for sync message responses (seconds)
	///
	/// Used for: StateRequest/Response, SharedChangeRequest/Response
	/// Default: 30 seconds
	pub message_timeout_secs: u64,

	/// Timeout for backfill requests (seconds)
	///
	/// Longer than message timeout for large batches.
	/// Default: 60 seconds
	pub backfill_request_timeout_secs: u64,

	/// Interval between sync loop iterations (seconds)
	///
	/// Checks for reconnections, triggers catch-up.
	/// Default: 5 seconds
	pub sync_loop_interval_secs: u64,

	/// Interval for connection health checks (seconds)
	///
	/// Updates devices.is_online and devices.last_seen_at.
	/// Default: 10 seconds
	pub connection_check_interval_secs: u64,
}

impl Default for NetworkConfig {
	fn default() -> Self {
		Self {
			message_timeout_secs: 30,
			backfill_request_timeout_secs: 60,
			sync_loop_interval_secs: 5,
			connection_check_interval_secs: 10,
		}
	}
}

/// Monitoring and maintenance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
	/// Interval for pruning sync coordination data (seconds)
	///
	/// Runs unified pruning for both peer log and tombstones.
	/// Default: 3600 seconds (1 hour)
	pub pruning_interval_secs: u64,

	/// Enable detailed sync metrics and logging
	///
	/// Default: true
	pub enable_metrics: bool,

	/// Log sync statistics at this interval (seconds)
	///
	/// Default: 300 seconds (5 minutes)
	pub metrics_log_interval_secs: u64,
}

impl Default for MonitoringConfig {
	fn default() -> Self {
		Self {
			pruning_interval_secs: 3600,
			enable_metrics: true,
			metrics_log_interval_secs: 300,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = SyncConfig::default();

		assert_eq!(config.batching.backfill_batch_size, 10_000);
		assert_eq!(config.retention.tombstone_max_retention_days, 7);
		assert_eq!(config.network.message_timeout_secs, 30);
		assert_eq!(config.monitoring.pruning_interval_secs, 3600);
	}

	#[test]
	fn test_aggressive_preset() {
		let config = SyncConfig::aggressive();

		assert_eq!(config.batching.backfill_batch_size, 5_000);
		assert_eq!(config.retention.tombstone_max_retention_days, 3);
		assert_eq!(config.network.sync_loop_interval_secs, 2);
		assert!(config.monitoring.enable_metrics);
	}

	#[test]
	fn test_conservative_preset() {
		let config = SyncConfig::conservative();

		assert_eq!(config.batching.backfill_batch_size, 25_000);
		assert_eq!(config.retention.tombstone_max_retention_days, 30);
		assert_eq!(config.network.sync_loop_interval_secs, 10);
	}

	#[test]
	fn test_mobile_preset() {
		let config = SyncConfig::mobile();

		assert_eq!(config.retention.tombstone_max_retention_days, 14);
		assert_eq!(config.network.sync_loop_interval_secs, 30);
		assert!(!config.monitoring.enable_metrics); // Battery saving
	}

	#[test]
	fn test_serialization() {
		let config = SyncConfig::default();

		let json = serde_json::to_string(&config).unwrap();
		let deserialized: SyncConfig = serde_json::from_str(&json).unwrap();

		assert_eq!(
			deserialized.batching.backfill_batch_size,
			config.batching.backfill_batch_size
		);
	}
}
