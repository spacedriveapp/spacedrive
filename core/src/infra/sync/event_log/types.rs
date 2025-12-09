//! Sync event log types
//!
//! High-level event types for tracking sync lifecycle, data flow, and network operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A logged sync event
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncEventLog {
	pub id: Option<i64>,
	pub timestamp: DateTime<Utc>,
	pub device_id: Uuid,
	pub event_type: SyncEventType,
	pub category: EventCategory,
	pub severity: EventSeverity,
	pub summary: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub details: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub correlation_id: Option<Uuid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub peer_device_id: Option<Uuid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub model_types: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub record_count: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub duration_ms: Option<u64>,
}

impl SyncEventLog {
	/// Create a new event with common fields pre-filled
	pub fn new(device_id: Uuid, event_type: SyncEventType, summary: impl Into<String>) -> Self {
		let (category, severity) = event_type.default_category_and_severity();

		Self {
			id: None,
			timestamp: Utc::now(),
			device_id,
			event_type,
			category,
			severity,
			summary: summary.into(),
			details: None,
			correlation_id: None,
			peer_device_id: None,
			model_types: None,
			record_count: None,
			duration_ms: None,
		}
	}

	/// Builder: set details JSON
	pub fn with_details(mut self, details: serde_json::Value) -> Self {
		self.details = Some(details);
		self
	}

	/// Builder: set correlation ID (for session tracking)
	pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
		self.correlation_id = Some(correlation_id);
		self
	}

	/// Builder: set peer device ID
	pub fn with_peer(mut self, peer_device_id: Uuid) -> Self {
		self.peer_device_id = Some(peer_device_id);
		self
	}

	/// Builder: set model types
	pub fn with_model_types(mut self, model_types: Vec<String>) -> Self {
		self.model_types = Some(model_types);
		self
	}

	/// Builder: set record count
	pub fn with_record_count(mut self, count: u64) -> Self {
		self.record_count = Some(count);
		self
	}

	/// Builder: set duration in milliseconds
	pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
		self.duration_ms = Some(duration_ms);
		self
	}

	/// Builder: set category
	pub fn with_category(mut self, category: EventCategory) -> Self {
		self.category = category;
		self
	}

	/// Builder: set severity
	pub fn with_severity(mut self, severity: EventSeverity) -> Self {
		self.severity = severity;
		self
	}
}

/// High-level sync event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncEventType {
	/// State machine transition (Uninitialized → Backfilling → CatchingUp → Ready ⇄ Paused)
	StateTransition,

	/// Backfill session started
	BackfillSessionStarted,

	/// Backfill session completed successfully
	BackfillSessionCompleted,

	/// Backfill session failed
	BackfillSessionFailed,

	/// Catch-up session started (incremental sync)
	CatchUpSessionStarted,

	/// Catch-up session completed
	CatchUpSessionCompleted,

	/// Batch of records ingested (aggregated, not per-record)
	BatchIngestion,

	/// Sent backfill request to peer
	BackfillRequestSent,

	/// Received backfill request from peer
	BackfillRequestReceived,

	/// Sent backfill response to peer
	BackfillResponseSent,

	/// Peer device connected
	PeerConnected,

	/// Peer device disconnected
	PeerDisconnected,

	/// Sync error occurred
	SyncError,
}

impl SyncEventType {
	/// Get default category and severity for this event type
	pub fn default_category_and_severity(&self) -> (EventCategory, EventSeverity) {
		match self {
			Self::StateTransition => (EventCategory::Lifecycle, EventSeverity::Info),
			Self::BackfillSessionStarted => (EventCategory::Lifecycle, EventSeverity::Info),
			Self::BackfillSessionCompleted => (EventCategory::Lifecycle, EventSeverity::Info),
			Self::BackfillSessionFailed => (EventCategory::Lifecycle, EventSeverity::Error),
			Self::CatchUpSessionStarted => (EventCategory::Lifecycle, EventSeverity::Debug),
			Self::CatchUpSessionCompleted => (EventCategory::Lifecycle, EventSeverity::Debug),
			Self::BatchIngestion => (EventCategory::DataFlow, EventSeverity::Debug),
			Self::BackfillRequestSent => (EventCategory::Network, EventSeverity::Debug),
			Self::BackfillRequestReceived => (EventCategory::Network, EventSeverity::Debug),
			Self::BackfillResponseSent => (EventCategory::Network, EventSeverity::Debug),
			Self::PeerConnected => (EventCategory::Network, EventSeverity::Info),
			Self::PeerDisconnected => (EventCategory::Network, EventSeverity::Info),
			Self::SyncError => (EventCategory::Error, EventSeverity::Error),
		}
	}

	/// Convert to string for database storage
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::StateTransition => "state_transition",
			Self::BackfillSessionStarted => "backfill_session_started",
			Self::BackfillSessionCompleted => "backfill_session_completed",
			Self::BackfillSessionFailed => "backfill_session_failed",
			Self::CatchUpSessionStarted => "catch_up_session_started",
			Self::CatchUpSessionCompleted => "catch_up_session_completed",
			Self::BatchIngestion => "batch_ingestion",
			Self::BackfillRequestSent => "backfill_request_sent",
			Self::BackfillRequestReceived => "backfill_request_received",
			Self::BackfillResponseSent => "backfill_response_sent",
			Self::PeerConnected => "peer_connected",
			Self::PeerDisconnected => "peer_disconnected",
			Self::SyncError => "sync_error",
		}
	}

	/// Parse from string (for database retrieval)
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"state_transition" => Some(Self::StateTransition),
			"backfill_session_started" => Some(Self::BackfillSessionStarted),
			"backfill_session_completed" => Some(Self::BackfillSessionCompleted),
			"backfill_session_failed" => Some(Self::BackfillSessionFailed),
			"catch_up_session_started" => Some(Self::CatchUpSessionStarted),
			"catch_up_session_completed" => Some(Self::CatchUpSessionCompleted),
			"batch_ingestion" => Some(Self::BatchIngestion),
			"backfill_request_sent" => Some(Self::BackfillRequestSent),
			"backfill_request_received" => Some(Self::BackfillRequestReceived),
			"backfill_response_sent" => Some(Self::BackfillResponseSent),
			"peer_connected" => Some(Self::PeerConnected),
			"peer_disconnected" => Some(Self::PeerDisconnected),
			"sync_error" => Some(Self::SyncError),
			_ => None,
		}
	}
}

/// Event category for grouping related events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
	/// State machine lifecycle events
	Lifecycle,

	/// Data synchronization flow
	DataFlow,

	/// Network communication
	Network,

	/// Errors and failures
	Error,
}

impl EventCategory {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Lifecycle => "lifecycle",
			Self::DataFlow => "data_flow",
			Self::Network => "network",
			Self::Error => "error",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"lifecycle" => Some(Self::Lifecycle),
			"data_flow" => Some(Self::DataFlow),
			"network" => Some(Self::Network),
			"error" => Some(Self::Error),
			_ => None,
		}
	}
}

/// Event severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
	/// Debug-level information
	Debug,

	/// Informational event
	Info,

	/// Warning condition
	Warning,

	/// Error condition
	Error,
}

impl EventSeverity {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Debug => "debug",
			Self::Info => "info",
			Self::Warning => "warning",
			Self::Error => "error",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"debug" => Some(Self::Debug),
			"info" => Some(Self::Info),
			"warning" => Some(Self::Warning),
			"error" => Some(Self::Error),
			_ => None,
		}
	}
}
