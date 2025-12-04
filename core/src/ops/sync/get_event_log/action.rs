//! Get sync event log action

use crate::context::CoreContext;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::infra::sync::SyncEventQuery;
use anyhow::Result;
use std::sync::Arc;

use super::{GetSyncEventLogInput, GetSyncEventLogOutput};

/// Get sync event log for the current library
pub struct GetSyncEventLog {
	pub input: GetSyncEventLogInput,
}

impl LibraryQuery for GetSyncEventLog {
	type Input = GetSyncEventLogInput;
	type Output = GetSyncEventLogOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::LibraryNotFound(library_id))?;

		let sync_service = library
			.sync_service()
			.ok_or_else(|| QueryError::Internal("Sync service not available".to_string()))?;

		// Build query from input
		let mut query = SyncEventQuery::new(library_id);

		if let (Some(start), Some(end)) = (self.input.start_time, self.input.end_time) {
			query = query.with_time_range(start, end);
		}

		if let Some(types) = self.input.event_types {
			query = query.with_event_types(types);
		}

		if let Some(categories) = self.input.categories {
			query = query.with_categories(categories);
		}

		if let Some(severities) = self.input.severities {
			query = query.with_severities(severities);
		}

		if let Some(peer_id) = self.input.peer_id {
			query = query.with_peer(peer_id);
		}

		if let Some(model_type) = self.input.model_type {
			query = query.with_model_type(model_type);
		}

		if let Some(correlation_id) = self.input.correlation_id {
			query = query.with_correlation_id(correlation_id);
		}

		if let Some(limit) = self.input.limit {
			query = query.with_limit(limit);
		}

		if let Some(offset) = self.input.offset {
			query = query.with_offset(offset);
		}

		// Query events
		let events = sync_service
			.event_logger()
			.query(query)
			.await
			.map_err(|e| QueryError::Internal(format!("Failed to query events: {}", e)))?;

		Ok(GetSyncEventLogOutput { events })
	}
}

// Register the query
crate::register_library_query!(GetSyncEventLog, "sync.eventLog");
