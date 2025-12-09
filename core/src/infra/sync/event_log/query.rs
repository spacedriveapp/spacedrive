//! Sync event log query types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::types::{EventCategory, EventSeverity, SyncEventType};

/// Query parameters for filtering sync events
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncEventQuery {
	/// Library to query
	pub library_id: Uuid,

	/// Time range filter (start, end)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,

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
	pub peer_filter: Option<Uuid>,

	/// Filter by model type
	#[serde(skip_serializing_if = "Option::is_none")]
	pub model_type_filter: Option<String>,

	/// Filter by correlation ID (show all events in a session)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub correlation_id: Option<Uuid>,

	/// Maximum number of results
	#[serde(skip_serializing_if = "Option::is_none")]
	pub limit: Option<u32>,

	/// Offset for pagination
	#[serde(skip_serializing_if = "Option::is_none")]
	pub offset: Option<u32>,
}

impl SyncEventQuery {
	/// Create a new query for a library
	pub fn new(library_id: Uuid) -> Self {
		Self {
			library_id,
			time_range: None,
			event_types: None,
			categories: None,
			severities: None,
			peer_filter: None,
			model_type_filter: None,
			correlation_id: None,
			limit: Some(1000), // Default limit
			offset: None,
		}
	}

	/// Builder: set time range
	pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
		self.time_range = Some((start, end));
		self
	}

	/// Builder: filter by event types
	pub fn with_event_types(mut self, types: Vec<SyncEventType>) -> Self {
		self.event_types = Some(types);
		self
	}

	/// Builder: filter by categories
	pub fn with_categories(mut self, categories: Vec<EventCategory>) -> Self {
		self.categories = Some(categories);
		self
	}

	/// Builder: filter by severities
	pub fn with_severities(mut self, severities: Vec<EventSeverity>) -> Self {
		self.severities = Some(severities);
		self
	}

	/// Builder: filter by peer
	pub fn with_peer(mut self, peer_id: Uuid) -> Self {
		self.peer_filter = Some(peer_id);
		self
	}

	/// Builder: filter by model type
	pub fn with_model_type(mut self, model_type: String) -> Self {
		self.model_type_filter = Some(model_type);
		self
	}

	/// Builder: filter by correlation ID
	pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
		self.correlation_id = Some(correlation_id);
		self
	}

	/// Builder: set limit
	pub fn with_limit(mut self, limit: u32) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Builder: set offset
	pub fn with_offset(mut self, offset: u32) -> Self {
		self.offset = Some(offset);
		self
	}
}

/// SQL query builder helper
pub struct QueryBuilder {
	where_clauses: Vec<String>,
	params: Vec<String>,
}

impl QueryBuilder {
	pub fn new() -> Self {
		Self {
			where_clauses: Vec::new(),
			params: Vec::new(),
		}
	}

	pub fn add_time_range(&mut self, start: DateTime<Utc>, end: DateTime<Utc>) {
		self.where_clauses
			.push("timestamp >= ? AND timestamp <= ?".to_string());
		self.params.push(start.to_rfc3339());
		self.params.push(end.to_rfc3339());
	}

	pub fn add_event_types(&mut self, types: &[SyncEventType]) {
		if !types.is_empty() {
			let placeholders = vec!["?"; types.len()].join(",");
			self.where_clauses
				.push(format!("event_type IN ({})", placeholders));
			for t in types {
				self.params.push(t.as_str().to_string());
			}
		}
	}

	pub fn add_categories(&mut self, categories: &[EventCategory]) {
		if !categories.is_empty() {
			let placeholders = vec!["?"; categories.len()].join(",");
			self.where_clauses
				.push(format!("category IN ({})", placeholders));
			for c in categories {
				self.params.push(c.as_str().to_string());
			}
		}
	}

	pub fn add_severities(&mut self, severities: &[EventSeverity]) {
		if !severities.is_empty() {
			let placeholders = vec!["?"; severities.len()].join(",");
			self.where_clauses
				.push(format!("severity IN ({})", placeholders));
			for s in severities {
				self.params.push(s.as_str().to_string());
			}
		}
	}

	pub fn add_peer_filter(&mut self, peer_id: Uuid) {
		self.where_clauses.push("peer_device_id = ?".to_string());
		self.params.push(peer_id.to_string());
	}

	pub fn add_model_type_filter(&mut self, model_type: &str) {
		self.where_clauses.push("model_types LIKE ?".to_string());
		self.params.push(format!("%{}%", model_type));
	}

	pub fn add_correlation_id(&mut self, correlation_id: Uuid) {
		self.where_clauses.push("correlation_id = ?".to_string());
		self.params.push(correlation_id.to_string());
	}

	pub fn add_device_filter(&mut self, device_id: Uuid) {
		self.where_clauses.push("device_id = ?".to_string());
		self.params.push(device_id.to_string());
	}

	pub fn build(&self) -> (String, Vec<String>) {
		let where_clause = if self.where_clauses.is_empty() {
			String::new()
		} else {
			format!("WHERE {}", self.where_clauses.join(" AND "))
		};

		(where_clause, self.params.clone())
	}
}

impl Default for QueryBuilder {
	fn default() -> Self {
		Self::new()
	}
}
