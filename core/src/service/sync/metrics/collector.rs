//! Central metrics collector for sync operations

use crate::service::sync::state::DeviceSyncState;
use crate::service::sync::metrics::types::*;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

/// Central collector for all sync metrics
#[derive(Debug)]
pub struct SyncMetricsCollector {
    metrics: Arc<SyncMetrics>,
    max_history_size: usize,
}

impl SyncMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(SyncMetrics::default()),
            max_history_size: 100,
        }
    }
    
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.max_history_size = size;
        self
    }

    /// Get the underlying metrics
    pub fn metrics(&self) -> &Arc<SyncMetrics> {
        &self.metrics
    }
    
    /// Get reference to metrics
    pub fn metrics(&self) -> &Arc<SyncMetrics> {
        &self.metrics
    }
    
    /// Record a state transition
    pub async fn record_state_transition(
        &self,
        from: DeviceSyncState,
        to: DeviceSyncState,
        reason: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();
        
        // Update current state
        {
            let mut current_state = self.metrics.state.current_state.write().await;
            *current_state = to;
        }
        
        // Update state entered time
        {
            let mut state_entered_at = self.metrics.state.state_entered_at.write().await;
            *state_entered_at = now;
        }
        
		// Record transition
		let transition = StateTransition {
			from,
			to,
			timestamp: now,
			reason: reason.clone(),
		};
        
        // Add to history
        {
            let mut history = self.metrics.state.state_history.write().await;
            history.push_back(transition.clone());
            
            // Trim to max size
            while history.len() > self.max_history_size {
                history.pop_front();
            }
        }
        
        // Update transition count
        {
            let mut transition_count = self.metrics.state.transition_count.write().await;
            *transition_count.entry((from, to)).or_insert(0) += 1;
        }
        
        // Update time in previous state
        {
            let mut state_entered_at = self.metrics.state.state_entered_at.write().await;
            let mut total_time = self.metrics.state.total_time_in_state.write().await;
            
            // Calculate duration BEFORE updating the entry time
            let duration = now.signed_duration_since(*state_entered_at);
            *total_time.entry(from).or_insert(std::time::Duration::ZERO) += 
                std::time::Duration::from_millis(duration.num_milliseconds().max(0) as u64);
            
            // Update entry time for new state AFTER calculating duration
            *state_entered_at = now;
        }
        
        debug!(
            from = ?from,
            to = ?to,
            reason = ?reason,
            "Recorded sync state transition"
        );
        
        Ok(())
    }
    
    /// Record a broadcast operation
    pub fn record_broadcast(&self, is_state_change: bool, batch_size: Option<u64>) {
        self.metrics.operations.broadcasts_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        if is_state_change {
            self.metrics.operations.state_changes_broadcast.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.metrics.operations.shared_changes_broadcast.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        
        if let Some(size) = batch_size {
            self.metrics.operations.broadcast_batches_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    
    /// Record a failed broadcast
    pub fn record_failed_broadcast(&self) {
        self.metrics.operations.failed_broadcasts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record received changes
    pub fn record_changes_received(&self, count: u64) {
        self.metrics.operations.changes_received.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record applied changes
    pub fn record_changes_applied(&self, count: u64) {
        self.metrics.operations.changes_applied.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record rejected changes
    pub fn record_changes_rejected(&self, count: u64) {
        self.metrics.operations.changes_rejected.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record buffer queue depth
    pub fn record_buffer_queue_depth(&self, depth: u64) {
        self.metrics.operations.buffer_queue_depth.store(depth, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record backfill session start
    pub fn record_backfill_session_start(&self) {
        self.metrics.operations.active_backfill_sessions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record backfill session completion
    pub fn record_backfill_session_complete(&self) {
        self.metrics.operations.active_backfill_sessions.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.operations.backfill_sessions_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record backfill pagination round
    pub fn record_backfill_pagination_round(&self) {
        self.metrics.operations.backfill_pagination_rounds.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record retry queue depth
    pub fn record_retry_queue_depth(&self, depth: u64) {
        self.metrics.operations.retry_queue_depth.store(depth, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record retry attempt
    pub fn record_retry_attempt(&self) {
        self.metrics.operations.retry_attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record retry success
    pub fn record_retry_success(&self) {
        self.metrics.operations.retry_successes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record data volume by model type
    pub async fn record_entries_synced(&self, model_type: &str, count: u64) {
        let mut entries_synced = self.metrics.data_volume.entries_synced.write().await;
        entries_synced
            .entry(model_type.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record data volume by device
    pub async fn record_device_entries(&self, device_id: Uuid, device_name: &str, count: u64) {
        let mut entries_by_device = self.metrics.data_volume.entries_by_device.write().await;
        
        let device_metrics = entries_by_device
            .entry(device_id)
            .or_insert_with(|| DeviceMetrics::new(device_id, device_name.to_string()));
        
        device_metrics.entries_received.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
        device_metrics.last_seen.store(Utc::now().timestamp() as u64, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record bytes transferred
    pub fn record_bytes_sent(&self, bytes: u64) {
        self.metrics.data_volume.bytes_sent.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record bytes received
    pub fn record_bytes_received(&self, bytes: u64) {
        self.metrics.data_volume.bytes_received.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record last sync time for peer
    pub async fn record_last_sync_peer(&self, peer_id: Uuid) {
        let mut last_sync_per_peer = self.metrics.data_volume.last_sync_per_peer.write().await;
        last_sync_per_peer.insert(peer_id, Utc::now());
    }
    
    /// Record last sync time for model
    pub async fn record_last_sync_model(&self, model_type: &str) {
        let mut last_sync_per_model = self.metrics.data_volume.last_sync_per_model.write().await;
        last_sync_per_model.insert(model_type.to_string(), Utc::now());
    }
    
    /// Record latency for broadcast operations
    pub fn record_broadcast_latency(&self, latency_ms: u64) {
        self.metrics.performance.broadcast_latency_ms.record(latency_ms);
    }
    
    /// Record latency for apply operations
    pub fn record_apply_latency(&self, latency_ms: u64) {
        self.metrics.performance.apply_latency_ms.record(latency_ms);
    }
    
    /// Record latency for backfill requests
    pub fn record_backfill_request_latency(&self, latency_ms: u64) {
        self.metrics.performance.backfill_request_latency_ms.record(latency_ms);
    }
    
    /// Record database query duration
    pub fn record_db_query_duration(&self, duration_ms: u64) {
        self.metrics.performance.db_query_duration_ms.record(duration_ms);
        self.metrics.performance.db_query_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Update state watermark
    pub fn update_state_watermark(&self, timestamp: u64) {
        self.metrics.performance.state_watermark.store(timestamp, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Update shared watermark
    pub async fn update_shared_watermark(&self, hlc: &str) {
        let mut shared_watermark = self.metrics.performance.shared_watermark.write().await;
        *shared_watermark = hlc.to_string();
    }
    
    /// Update watermark lag for peer
    pub async fn update_watermark_lag(&self, peer_id: Uuid, lag_ms: u64) {
        let mut watermark_lag = self.metrics.performance.watermark_lag_ms.write().await;
        watermark_lag
            .entry(peer_id)
            .or_insert_with(|| AtomicU64::new(0))
            .store(lag_ms, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Update HLC drift
    pub fn update_hlc_drift(&self, drift_ms: i64) {
        self.metrics.performance.hlc_physical_drift_ms.store(drift_ms, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Update HLC counter max
    pub fn update_hlc_counter_max(&self, max: u64) {
        self.metrics.performance.hlc_counter_max.store(max, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record an error event
    pub async fn record_error(&self, error: ErrorEvent) {
        // Update error counts
        self.metrics.errors.total_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
		match error.error_type.as_str() {
			"network" => {
				self.metrics.errors.network_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			}
			"database" => {
				self.metrics.errors.database_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			}
			"apply" => {
				self.metrics.errors.apply_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			}
			"validation" => {
				self.metrics.errors.validation_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			}
			_ => {}
		}
        
		// Add to recent errors
		{
			let mut recent_errors = self.metrics.errors.recent_errors.write().await;
			recent_errors.push_back(error.clone());
			
			// Trim to max size
			while recent_errors.len() > self.max_history_size {
				recent_errors.pop_front();
			}
		}
		
		warn!(
			error_type = %error.error_type,
			message = %error.message,
			"Recorded sync error"
		);
    }
    
    /// Record conflict detection
    pub fn record_conflict_detected(&self) {
        self.metrics.errors.conflicts_detected.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record conflict resolution by HLC
    pub fn record_conflict_resolved_by_hlc(&self) {
        self.metrics.errors.conflicts_resolved_by_hlc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Set device online status
    pub async fn set_device_online(&self, device_id: Uuid, is_online: bool) {
        let mut entries_by_device = self.metrics.data_volume.entries_by_device.write().await;
        if let Some(device_metrics) = entries_by_device.get_mut(&device_id) {
            device_metrics.is_online.store(is_online, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl Default for SyncMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}