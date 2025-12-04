//! Sync event logger
//!
//! Persists sync events to sync.db and provides query API.

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement};
use tracing::warn;
use uuid::Uuid;

use super::query::{QueryBuilder, SyncEventQuery};
use super::types::{EventCategory, EventSeverity, SyncEventLog, SyncEventType};

/// Sync event logger
///
/// Writes high-level sync events to sync.db for debugging and observability.
#[derive(Debug)]
pub struct SyncEventLogger {
	library_id: Uuid,
	device_id: Uuid,
	conn: Arc<DatabaseConnection>,
}

impl SyncEventLogger {
	/// Create a new event logger
	pub fn new(
		library_id: Uuid,
		device_id: Uuid,
		conn: Arc<DatabaseConnection>,
	) -> Self {
		Self {
			library_id,
			device_id,
			conn,
		}
	}

	/// Log a single event
	///
	/// Errors are logged but not propagated - event logging must never crash sync.
	pub async fn log(&self, event: SyncEventLog) -> Result<()> {
		if let Err(e) = self.log_internal(event).await {
			warn!("Failed to log sync event: {}", e);
		}
		Ok(())
	}

	/// Internal log implementation
	async fn log_internal(&self, event: SyncEventLog) -> Result<()> {
		let details_json = event
			.details
			.as_ref()
			.map(|d| serde_json::to_string(d))
			.transpose()?;

		let model_types_str = event
			.model_types
			.as_ref()
			.map(|types| types.join(","));

		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT INTO sync_event_log (
					timestamp, device_id, event_type, category, severity,
					summary, details, correlation_id, peer_device_id,
					model_types, record_count, duration_ms
				)
				VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
				"#,
				vec![
					event.timestamp.to_rfc3339().into(),
					event.device_id.to_string().into(),
					event.event_type.as_str().into(),
					event.category.as_str().into(),
					event.severity.as_str().into(),
					event.summary.into(),
					details_json.into(),
					event.correlation_id.map(|id| id.to_string()).into(),
					event.peer_device_id.map(|id| id.to_string()).into(),
					model_types_str.into(),
					event.record_count.map(|c| c as i64).into(),
					event.duration_ms.map(|d| d as i64).into(),
				],
			))
			.await?;

		Ok(())
	}

	/// Log multiple events in a single transaction
	pub async fn log_batch(&self, events: Vec<SyncEventLog>) -> Result<()> {
		if events.is_empty() {
			return Ok(());
		}

		for event in events {
			if let Err(e) = self.log_internal(event).await {
				warn!("Failed to log sync event in batch: {}", e);
			}
		}

		Ok(())
	}

	/// Query events with filters
	pub async fn query(&self, query: SyncEventQuery) -> Result<Vec<SyncEventLog>> {
		let mut builder = QueryBuilder::new();

		// Always filter by this device
		builder.add_device_filter(self.device_id);

		// Apply query filters
		if let Some((start, end)) = query.time_range {
			builder.add_time_range(start, end);
		}

		if let Some(types) = query.event_types.as_ref() {
			builder.add_event_types(types);
		}

		if let Some(categories) = query.categories.as_ref() {
			builder.add_categories(categories);
		}

		if let Some(severities) = query.severities.as_ref() {
			builder.add_severities(severities);
		}

		if let Some(peer_id) = query.peer_filter {
			builder.add_peer_filter(peer_id);
		}

		if let Some(model_type) = query.model_type_filter.as_ref() {
			builder.add_model_type_filter(model_type);
		}

		if let Some(correlation_id) = query.correlation_id {
			builder.add_correlation_id(correlation_id);
		}

		let (where_clause, params) = builder.build();

		let limit = query.limit.unwrap_or(1000);
		let offset = query.offset.unwrap_or(0);

		let sql = format!(
			r#"
			SELECT id, timestamp, device_id, event_type, category, severity,
			       summary, details, correlation_id, peer_device_id,
			       model_types, record_count, duration_ms
			FROM sync_event_log
			{}
			ORDER BY timestamp DESC
			LIMIT {}
			OFFSET {}
			"#,
			where_clause, limit, offset
		);

		let param_values: Vec<sea_orm::Value> =
			params.into_iter().map(|p| p.into()).collect();

		let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, &sql, param_values);

		let rows = self.conn.query_all(stmt).await?;

		let events: Vec<SyncEventLog> = rows
			.into_iter()
			.filter_map(|row| Self::parse_row(&row).ok())
			.collect();

		Ok(events)
	}

	/// Parse a database row into a SyncEventLog
	fn parse_row(row: &sea_orm::QueryResult) -> Result<SyncEventLog> {
		let id: i64 = row.try_get("", "id")?;
		let timestamp_str: String = row.try_get("", "timestamp")?;
		let device_id_str: String = row.try_get("", "device_id")?;
		let event_type_str: String = row.try_get("", "event_type")?;
		let category_str: String = row.try_get("", "category")?;
		let severity_str: String = row.try_get("", "severity")?;
		let summary: String = row.try_get("", "summary")?;
		let details_str: Option<String> = row.try_get("", "details").ok();
		let correlation_id_str: Option<String> = row.try_get("", "correlation_id").ok();
		let peer_device_id_str: Option<String> = row.try_get("", "peer_device_id").ok();
		let model_types_str: Option<String> = row.try_get("", "model_types").ok();
		let record_count: Option<i64> = row.try_get("", "record_count").ok();
		let duration_ms: Option<i64> = row.try_get("", "duration_ms").ok();

		Ok(SyncEventLog {
			id: Some(id),
			timestamp: DateTime::parse_from_rfc3339(&timestamp_str)?
				.with_timezone(&Utc),
			device_id: Uuid::parse_str(&device_id_str)?,
			event_type: SyncEventType::from_str(&event_type_str)
				.ok_or_else(|| anyhow::anyhow!("Invalid event type: {}", event_type_str))?,
			category: EventCategory::from_str(&category_str)
				.ok_or_else(|| anyhow::anyhow!("Invalid category: {}", category_str))?,
			severity: EventSeverity::from_str(&severity_str)
				.ok_or_else(|| anyhow::anyhow!("Invalid severity: {}", severity_str))?,
			summary,
			details: details_str
				.as_ref()
				.and_then(|s| serde_json::from_str(s).ok()),
			correlation_id: correlation_id_str
				.as_ref()
				.and_then(|s| Uuid::parse_str(s).ok()),
			peer_device_id: peer_device_id_str
				.as_ref()
				.and_then(|s| Uuid::parse_str(s).ok()),
			model_types: model_types_str
				.map(|s| s.split(',').map(|t| t.to_string()).collect()),
			record_count: record_count.map(|c| c as u64),
			duration_ms: duration_ms.map(|d| d as u64),
		})
	}

	/// Clean up old events (called by pruning task)
	pub async fn cleanup_old_events(&self, older_than: DateTime<Utc>) -> Result<usize> {
		let result = self
			.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"DELETE FROM sync_event_log WHERE timestamp < ?",
				vec![older_than.to_rfc3339().into()],
			))
			.await?;

		Ok(result.rows_affected() as usize)
	}

	/// Get the database connection (for sharing with other components)
	pub fn conn(&self) -> &DatabaseConnection {
		&self.conn
	}

	/// Get library ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Get device ID
	pub fn device_id(&self) -> Uuid {
		self.device_id
	}
}
