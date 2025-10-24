//! Point-in-time snapshots of sync metrics

use crate::service::sync::state::DeviceSyncState;
use crate::service::sync::metrics::types::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Point-in-time snapshot of all sync metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMetricsSnapshot {
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,
    
    /// State metrics
    pub state: SyncStateSnapshot,
    
    /// Operation metrics
    pub operations: OperationSnapshot,
    
    /// Data volume metrics
    pub data_volume: DataVolumeSnapshot,
    
    /// Performance metrics
    pub performance: PerformanceSnapshot,
    
    /// Error metrics
    pub errors: ErrorSnapshot,
}

/// State metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncStateSnapshot {
    pub current_state: DeviceSyncState,
    pub state_entered_at: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub state_history: Vec<StateTransition>,
    pub total_time_in_state: HashMap<DeviceSyncState, u64>, // milliseconds
    pub transition_count: HashMap<(DeviceSyncState, DeviceSyncState), u64>,
}

/// Operation metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationSnapshot {
    // Broadcasts
    pub broadcasts_sent: u64,
    pub state_changes_broadcast: u64,
    pub shared_changes_broadcast: u64,
    pub broadcast_batches_sent: u64,
    pub failed_broadcasts: u64,
    
    // Receives
    pub changes_received: u64,
    pub changes_applied: u64,
    pub changes_rejected: u64,
    pub buffer_queue_depth: u64,
    
    // Backfill
    pub active_backfill_sessions: u64,
    pub backfill_sessions_completed: u64,
    pub backfill_pagination_rounds: u64,
    
    // Retries
    pub retry_queue_depth: u64,
    pub retry_attempts: u64,
    pub retry_successes: u64,
}

/// Data volume metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataVolumeSnapshot {
    pub entries_synced: HashMap<String, u64>,
    pub entries_by_device: HashMap<Uuid, DeviceMetricsSnapshot>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub last_sync_per_peer: HashMap<Uuid, DateTime<Utc>>,
    pub last_sync_per_model: HashMap<String, DateTime<Utc>>,
}

/// Device metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetricsSnapshot {
    pub device_id: Uuid,
    pub device_name: String,
    pub entries_received: u64,
    pub last_seen: DateTime<Utc>,
    pub is_online: bool,
}

/// Performance metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub broadcast_latency: LatencySnapshot,
    pub apply_latency: LatencySnapshot,
    pub backfill_request_latency: LatencySnapshot,
    pub state_watermark: DateTime<Utc>,
    pub shared_watermark: String,
    pub watermark_lag_ms: HashMap<Uuid, u64>,
    pub hlc_physical_drift_ms: i64,
    pub hlc_counter_max: u64,
    pub db_query_duration: LatencySnapshot,
    pub db_query_count: u64,
}

/// Latency metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencySnapshot {
    pub count: u64,
    pub avg_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
}

/// Error metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorSnapshot {
    pub total_errors: u64,
    pub network_errors: u64,
    pub database_errors: u64,
    pub apply_errors: u64,
    pub validation_errors: u64,
    pub recent_errors: Vec<ErrorEvent>,
    pub conflicts_detected: u64,
    pub conflicts_resolved_by_hlc: u64,
}

impl SyncMetricsSnapshot {
    /// Create a snapshot from current metrics
    pub async fn from_metrics(metrics: &Arc<SyncMetrics>) -> Self {
        let now = Utc::now();
        
        // State snapshot
        let current_state = *metrics.state.current_state.read().await;
        let state_entered_at = *metrics.state.state_entered_at.read().await;
        let uptime_seconds = now.signed_duration_since(state_entered_at).num_seconds().max(0) as u64;
        
        let state_history = metrics.state.state_history.read().await.clone().into();
        let total_time_in_state = metrics.state.total_time_in_state.read().await
            .iter()
            .map(|(k, v)| (*k, v.as_millis() as u64))
            .collect();
        let transition_count = metrics.state.transition_count.read().await.clone();
        
        let state = SyncStateSnapshot {
            current_state,
            state_entered_at,
            uptime_seconds,
            state_history,
            total_time_in_state,
            transition_count,
        };
        
        // Operation snapshot
        let operations = OperationSnapshot {
            broadcasts_sent: metrics.operations.broadcasts_sent.load(std::sync::atomic::Ordering::Relaxed),
            state_changes_broadcast: metrics.operations.state_changes_broadcast.load(std::sync::atomic::Ordering::Relaxed),
            shared_changes_broadcast: metrics.operations.shared_changes_broadcast.load(std::sync::atomic::Ordering::Relaxed),
            broadcast_batches_sent: metrics.operations.broadcast_batches_sent.load(std::sync::atomic::Ordering::Relaxed),
            failed_broadcasts: metrics.operations.failed_broadcasts.load(std::sync::atomic::Ordering::Relaxed),
            changes_received: metrics.operations.changes_received.load(std::sync::atomic::Ordering::Relaxed),
            changes_applied: metrics.operations.changes_applied.load(std::sync::atomic::Ordering::Relaxed),
            changes_rejected: metrics.operations.changes_rejected.load(std::sync::atomic::Ordering::Relaxed),
            buffer_queue_depth: metrics.operations.buffer_queue_depth.load(std::sync::atomic::Ordering::Relaxed),
            active_backfill_sessions: metrics.operations.active_backfill_sessions.load(std::sync::atomic::Ordering::Relaxed),
            backfill_sessions_completed: metrics.operations.backfill_sessions_completed.load(std::sync::atomic::Ordering::Relaxed),
            backfill_pagination_rounds: metrics.operations.backfill_pagination_rounds.load(std::sync::atomic::Ordering::Relaxed),
            retry_queue_depth: metrics.operations.retry_queue_depth.load(std::sync::atomic::Ordering::Relaxed),
            retry_attempts: metrics.operations.retry_attempts.load(std::sync::atomic::Ordering::Relaxed),
            retry_successes: metrics.operations.retry_successes.load(std::sync::atomic::Ordering::Relaxed),
        };
        
        // Data volume snapshot
        let entries_synced = metrics.data_volume.entries_synced.read().await
            .iter()
            .map(|(k, v)| (k.clone(), v.load(std::sync::atomic::Ordering::Relaxed)))
            .collect();
        
        let entries_by_device = metrics.data_volume.entries_by_device.read().await
            .iter()
            .map(|(device_id, device_metrics)| {
                (*device_id, DeviceMetricsSnapshot {
                    device_id: device_metrics.device_id,
                    device_name: device_metrics.device_name.clone(),
                    entries_received: device_metrics.entries_received.load(std::sync::atomic::Ordering::Relaxed),
                    last_seen: DateTime::from_timestamp(device_metrics.last_seen.load(std::sync::atomic::Ordering::Relaxed) as i64, 0)
                        .unwrap_or_else(|| Utc::now()),
                    is_online: device_metrics.is_online.load(std::sync::atomic::Ordering::Relaxed),
                })
            })
            .collect();
        
        let last_sync_per_peer = metrics.data_volume.last_sync_per_peer.read().await.clone();
        let last_sync_per_model = metrics.data_volume.last_sync_per_model.read().await.clone();
        
        let data_volume = DataVolumeSnapshot {
            entries_synced,
            entries_by_device,
            bytes_sent: metrics.data_volume.bytes_sent.load(std::sync::atomic::Ordering::Relaxed),
            bytes_received: metrics.data_volume.bytes_received.load(std::sync::atomic::Ordering::Relaxed),
            last_sync_per_peer,
            last_sync_per_model,
        };
        
        // Performance snapshot
        let state_watermark = DateTime::from_timestamp(
            metrics.performance.state_watermark.load(std::sync::atomic::Ordering::Relaxed) as i64,
            0
        ).unwrap_or_else(|| Utc::now());
        
        let shared_watermark = metrics.performance.shared_watermark.read().await.clone();
        let watermark_lag_ms = metrics.performance.watermark_lag_ms.read().await
            .iter()
            .map(|(k, v)| (*k, v.load(std::sync::atomic::Ordering::Relaxed)))
            .collect();
        
        let performance = PerformanceSnapshot {
            broadcast_latency: LatencySnapshot::from_histogram(&metrics.performance.broadcast_latency_ms),
            apply_latency: LatencySnapshot::from_histogram(&metrics.performance.apply_latency_ms),
            backfill_request_latency: LatencySnapshot::from_histogram(&metrics.performance.backfill_request_latency_ms),
            state_watermark,
            shared_watermark,
            watermark_lag_ms,
            hlc_physical_drift_ms: metrics.performance.hlc_physical_drift_ms.load(std::sync::atomic::Ordering::Relaxed),
            hlc_counter_max: metrics.performance.hlc_counter_max.load(std::sync::atomic::Ordering::Relaxed),
            db_query_duration: LatencySnapshot::from_histogram(&metrics.performance.db_query_duration_ms),
            db_query_count: metrics.performance.db_query_count.load(std::sync::atomic::Ordering::Relaxed),
        };
        
        // Error snapshot
        let recent_errors = metrics.errors.recent_errors.read().await.clone().into();
        
        let errors = ErrorSnapshot {
            total_errors: metrics.errors.total_errors.load(std::sync::atomic::Ordering::Relaxed),
            network_errors: metrics.errors.network_errors.load(std::sync::atomic::Ordering::Relaxed),
            database_errors: metrics.errors.database_errors.load(std::sync::atomic::Ordering::Relaxed),
            apply_errors: metrics.errors.apply_errors.load(std::sync::atomic::Ordering::Relaxed),
            validation_errors: metrics.errors.validation_errors.load(std::sync::atomic::Ordering::Relaxed),
            recent_errors,
            conflicts_detected: metrics.errors.conflicts_detected.load(std::sync::atomic::Ordering::Relaxed),
            conflicts_resolved_by_hlc: metrics.errors.conflicts_resolved_by_hlc.load(std::sync::atomic::Ordering::Relaxed),
        };
        
        Self {
            timestamp: now,
            state,
            operations,
            data_volume,
            performance,
            errors,
        }
    }
    
    /// Filter snapshot to only include data since a specific time
    pub fn filter_since(&mut self, since: DateTime<Utc>) {
        // Filter state history
        self.state.state_history.retain(|transition| transition.timestamp >= since);
        
        // Filter recent errors
        self.errors.recent_errors.retain(|error| error.timestamp >= since);
        
        // Note: Other metrics are cumulative, so we don't filter them
    }
    
    /// Filter snapshot to only include data for a specific peer
    pub fn filter_by_peer(&mut self, peer_id: Uuid) {
        // Filter device metrics
        self.data_volume.entries_by_device.retain(|device_id, _| *device_id == peer_id);
        self.data_volume.last_sync_per_peer.retain(|device_id, _| *device_id == peer_id);
        self.performance.watermark_lag_ms.retain(|device_id, _| *device_id == peer_id);
        
        // Filter recent errors
        self.errors.recent_errors.retain(|error| error.device_id == Some(peer_id));
    }
    
    /// Filter snapshot to only include data for a specific model type
    pub fn filter_by_model(&mut self, model_type: &str) {
        // Filter entries synced
        self.data_volume.entries_synced.retain(|model, _| model == model_type);
        self.data_volume.last_sync_per_model.retain(|model, _| model == model_type);
        
		// Filter recent errors
		self.errors.recent_errors.retain(|error| error.model_type.as_ref() == Some(model_type));
    }
}

impl LatencySnapshot {
    fn from_histogram(histogram: &HistogramMetric) -> Self {
        Self {
            count: histogram.count(),
            avg_ms: histogram.avg(),
            min_ms: histogram.min(),
            max_ms: histogram.max(),
        }
    }
}