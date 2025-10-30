//! Get sync metrics action

use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;

use super::{GetSyncMetricsInput, GetSyncMetricsOutput};

/// Get sync metrics for the current library
pub struct GetSyncMetrics {
	pub input: GetSyncMetricsInput,
}

impl LibraryQuery for GetSyncMetrics {
	type Input = GetSyncMetricsInput;
	type Output = GetSyncMetricsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Use the input from the query
		let input = self.input;
		// Get the specific library from the library manager
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::LibraryNotFound(library_id))?;
		
		let sync_service = library.sync_service()
			.ok_or_else(|| QueryError::Internal("Sync service not available".to_string()))?;
		let metrics = sync_service.metrics();
		
		// Create a snapshot of current metrics
		let mut snapshot = SyncMetricsSnapshot::from_metrics(metrics.metrics()).await;
		
		// Apply filters
		if let Some(since) = input.since {
			snapshot.filter_since(since);
		}
		
		if let Some(peer_id) = input.peer_id {
			snapshot.filter_by_peer(peer_id);
		}
		
		if let Some(model_type) = input.model_type {
			snapshot.filter_by_model(&model_type);
		}
		
		// Apply category filters
		if input.state_only.unwrap_or(false) {
			// Keep only state metrics, clear others
			snapshot.operations = Default::default();
			snapshot.data_volume = Default::default();
			snapshot.performance = Default::default();
			snapshot.errors = Default::default();
		}
		
		if input.operations_only.unwrap_or(false) {
			// Keep only operation metrics, clear others
			snapshot.state = Default::default();
			snapshot.data_volume = Default::default();
			snapshot.performance = Default::default();
			snapshot.errors = Default::default();
		}
		
		if input.errors_only.unwrap_or(false) {
			// Keep only error metrics, clear others
			snapshot.state = Default::default();
			snapshot.operations = Default::default();
			snapshot.data_volume = Default::default();
			snapshot.performance = Default::default();
		}
		
		Ok(GetSyncMetricsOutput { metrics: snapshot })
	}
}

// Register the query
crate::register_library_query!(GetSyncMetrics, "sync.metrics");