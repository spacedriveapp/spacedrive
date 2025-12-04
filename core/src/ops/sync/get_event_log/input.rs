//! Input types for get_sync_event_log query

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::infra::sync::{EventCategory, EventSeverity, SyncEventType};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetSyncEventLogInput {
	/// Time range filter (start)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_time: Option<DateTime<Utc>>,

	/// Time range filter (end)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_time: Option<DateTime<Utc>>,

	/// Filter by event types
	#[serde(skip_serializing_if = "Option::is_none")]
	pub event_types: Option<Vec<SyncEventType>>,

	/// Filter by categories
	#[serde(skip_serializing_if = "Option::is_none")]
	pub categories: Option<Vec<EventCategory>>,

	/// Filter by severity levels
	#[serde(skip_serializing_if = "Option::is_none")]
	pub severities: Option<Vec<EventSeverity>>,

	/// Filter by peer device
	#[serde(skip_serializing_if = "Option::is_none")]
	pub peer_id: Option<Uuid>,

	/// Filter by model type
	#[serde(skip_serializing_if = "Option::is_none")]
	pub model_type: Option<String>,

	/// Filter by correlation ID
	#[serde(skip_serializing_if = "Option::is_none")]
	pub correlation_id: Option<Uuid>,

	/// Maximum number of results
	#[serde(skip_serializing_if = "Option::is_none")]
	pub limit: Option<u32>,

	/// Offset for pagination
	#[serde(skip_serializing_if = "Option::is_none")]
	pub offset: Option<u32>,
}
