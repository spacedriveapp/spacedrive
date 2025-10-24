//! Metric types and data structures for sync observability

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::service::sync::state::DeviceSyncState;

/// Central metrics collector for sync operations
#[derive(Debug)]
pub struct SyncMetrics {
    /// State transition tracking
    pub state: SyncStateMetrics,
    
    /// Operation counters
    pub operations: OperationMetrics,
    
    /// Data volume tracking
    pub data_volume: DataVolumeMetrics,
    
    /// Performance metrics
    pub performance: PerformanceMetrics,
    
    /// Error tracking
    pub errors: ErrorMetrics,
}

impl Default for SyncMetrics {
    fn default() -> Self {
        Self {
            state: SyncStateMetrics::default(),
            operations: OperationMetrics::default(),
            data_volume: DataVolumeMetrics::default(),
            performance: PerformanceMetrics::default(),
            errors: ErrorMetrics::default(),
        }
    }
}

/// Sync state transition tracking
#[derive(Debug)]
pub struct SyncStateMetrics {
    /// Current sync state
    pub current_state: Arc<RwLock<DeviceSyncState>>,
    
    /// When current state was entered
    pub state_entered_at: Arc<RwLock<DateTime<Utc>>>,
    
    /// State transition history (last N transitions)
    pub state_history: Arc<RwLock<VecDeque<StateTransition>>>,
    
    /// Total time spent in each state
    pub total_time_in_state: Arc<RwLock<HashMap<DeviceSyncState, std::time::Duration>>>,
    
    /// Transition counts between states
    pub transition_count: Arc<RwLock<HashMap<(DeviceSyncState, DeviceSyncState), u64>>>,
}

impl Default for SyncStateMetrics {
    fn default() -> Self {
        Self {
            current_state: Arc::new(RwLock::new(DeviceSyncState::Uninitialized)),
            state_entered_at: Arc::new(RwLock::new(Utc::now())),
            state_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            total_time_in_state: Arc::new(RwLock::new(HashMap::new())),
            transition_count: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// State transition event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: DeviceSyncState,
    pub to: DeviceSyncState,
    pub timestamp: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Operation counters for sync activities
#[derive(Debug)]
pub struct OperationMetrics {
    // Broadcasts
    pub broadcasts_sent: AtomicU64,
    pub state_changes_broadcast: AtomicU64,
    pub shared_changes_broadcast: AtomicU64,
    pub broadcast_batches_sent: AtomicU64,
    pub failed_broadcasts: AtomicU64,
    
    // Receives
    pub changes_received: AtomicU64,
    pub changes_applied: AtomicU64,
    pub changes_rejected: AtomicU64,
    pub buffer_queue_depth: AtomicU64,
    
    // Backfill
    pub active_backfill_sessions: AtomicU64,
    pub backfill_sessions_completed: AtomicU64,
    pub backfill_pagination_rounds: AtomicU64,
    
    // Retries
    pub retry_queue_depth: AtomicU64,
    pub retry_attempts: AtomicU64,
    pub retry_successes: AtomicU64,
}

impl Default for OperationMetrics {
    fn default() -> Self {
        Self {
            broadcasts_sent: AtomicU64::new(0),
            state_changes_broadcast: AtomicU64::new(0),
            shared_changes_broadcast: AtomicU64::new(0),
            broadcast_batches_sent: AtomicU64::new(0),
            failed_broadcasts: AtomicU64::new(0),
            changes_received: AtomicU64::new(0),
            changes_applied: AtomicU64::new(0),
            changes_rejected: AtomicU64::new(0),
            buffer_queue_depth: AtomicU64::new(0),
            active_backfill_sessions: AtomicU64::new(0),
            backfill_sessions_completed: AtomicU64::new(0),
            backfill_pagination_rounds: AtomicU64::new(0),
            retry_queue_depth: AtomicU64::new(0),
            retry_attempts: AtomicU64::new(0),
            retry_successes: AtomicU64::new(0),
        }
    }
}

/// Data volume tracking metrics
#[derive(Debug)]
pub struct DataVolumeMetrics {
    /// Per-model type counters
    pub entries_synced: Arc<RwLock<HashMap<String, AtomicU64>>>,
    
    /// Per-device metrics
    pub entries_by_device: Arc<RwLock<HashMap<Uuid, DeviceMetrics>>>,
    
    /// Bytes transferred
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    
    /// Last sync timestamps
    pub last_sync_per_peer: Arc<RwLock<HashMap<Uuid, DateTime<Utc>>>>,
    pub last_sync_per_model: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl Default for DataVolumeMetrics {
    fn default() -> Self {
        Self {
            entries_synced: Arc::new(RwLock::new(HashMap::new())),
            entries_by_device: Arc::new(RwLock::new(HashMap::new())),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            last_sync_per_peer: Arc::new(RwLock::new(HashMap::new())),
            last_sync_per_model: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// Per-device metrics
#[derive(Debug)]
pub struct DeviceMetrics {
    pub device_id: Uuid,
    pub device_name: String,
    pub entries_received: AtomicU64,
    pub last_seen: AtomicU64, // Unix timestamp
    pub is_online: AtomicBool,
}

impl DeviceMetrics {
    pub fn new(device_id: Uuid, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
            entries_received: AtomicU64::new(0),
            last_seen: AtomicU64::new(Utc::now().timestamp() as u64),
            is_online: AtomicBool::new(true),
        }
    }
}

/// Performance metrics with latency tracking
#[derive(Debug)]
pub struct PerformanceMetrics {
    /// Latency histograms
    pub broadcast_latency_ms: HistogramMetric,
    pub apply_latency_ms: HistogramMetric,
    pub backfill_request_latency_ms: HistogramMetric,
    
    /// Watermark tracking
    pub state_watermark: AtomicU64, // Unix timestamp
    pub shared_watermark: Arc<RwLock<String>>, // HLC string
    pub watermark_lag_ms: Arc<RwLock<HashMap<Uuid, AtomicU64>>>, // Per-peer lag
    
    /// HLC drift tracking
    pub hlc_physical_drift_ms: AtomicI64,
    pub hlc_counter_max: AtomicU64,
    
    /// Database performance
    pub db_query_duration_ms: HistogramMetric,
    pub db_query_count: AtomicU64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            broadcast_latency_ms: HistogramMetric::new(),
            apply_latency_ms: HistogramMetric::new(),
            backfill_request_latency_ms: HistogramMetric::new(),
            state_watermark: AtomicU64::new(Utc::now().timestamp() as u64),
            shared_watermark: Arc::new(RwLock::new(String::new())),
            watermark_lag_ms: Arc::new(RwLock::new(HashMap::new())),
            hlc_physical_drift_ms: AtomicI64::new(0),
            hlc_counter_max: AtomicU64::new(0),
            db_query_duration_ms: HistogramMetric::new(),
            db_query_count: AtomicU64::new(0),
        }
    }
}

/// Histogram metric for tracking latency distributions
#[derive(Debug)]
pub struct HistogramMetric {
    pub count: AtomicU64,
    pub sum: AtomicU64,
    pub min: AtomicU64,
    pub max: AtomicU64,
}

impl HistogramMetric {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
        }
    }
    
    pub fn record(&self, value_ms: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value_ms, Ordering::Relaxed);
        
        // Update min
        loop {
            let current_min = self.min.load(Ordering::Relaxed);
            if value_ms >= current_min {
                break;
            }
            if self.min.compare_exchange_weak(current_min, value_ms, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
                break;
            }
        }
        
        // Update max
        loop {
            let current_max = self.max.load(Ordering::Relaxed);
            if value_ms <= current_max {
                break;
            }
            if self.max.compare_exchange_weak(current_max, value_ms, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
                break;
            }
        }
    }
    
    pub fn avg(&self) -> f64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            self.sum.load(Ordering::Relaxed) as f64 / count as f64
        }
    }
    
    pub fn min(&self) -> u64 {
        let min = self.min.load(Ordering::Relaxed);
        if min == u64::MAX { 0 } else { min }
    }
    
    pub fn max(&self) -> u64 {
        self.max.load(Ordering::Relaxed)
    }
    
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// Error tracking metrics
#[derive(Debug)]
pub struct ErrorMetrics {
    /// Error counts by type
    pub total_errors: AtomicU64,
    pub network_errors: AtomicU64,
    pub database_errors: AtomicU64,
    pub apply_errors: AtomicU64,
    pub validation_errors: AtomicU64,
    
    /// Recent errors (ring buffer)
    pub recent_errors: Arc<RwLock<VecDeque<ErrorEvent>>>,
    
    /// Conflict resolution
    pub conflicts_detected: AtomicU64,
    pub conflicts_resolved_by_hlc: AtomicU64,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_errors: AtomicU64::new(0),
            network_errors: AtomicU64::new(0),
            database_errors: AtomicU64::new(0),
            apply_errors: AtomicU64::new(0),
            validation_errors: AtomicU64::new(0),
            recent_errors: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            conflicts_detected: AtomicU64::new(0),
            conflicts_resolved_by_hlc: AtomicU64::new(0),
        }
    }
}

/// Error event for tracking recent errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub timestamp: DateTime<Utc>,
    pub error_type: String,
    pub message: String,
    pub model_type: Option<String>,
    pub device_id: Option<Uuid>,
}

impl ErrorEvent {
    pub fn new(error_type: String, message: String) -> Self {
        Self {
            timestamp: Utc::now(),
            error_type,
            message,
            model_type: None,
            device_id: None,
        }
    }
    
    pub fn with_model_type(mut self, model_type: String) -> Self {
        self.model_type = Some(model_type);
        self
    }
    
    pub fn with_device_id(mut self, device_id: Uuid) -> Self {
        self.device_id = Some(device_id);
        self
    }
}