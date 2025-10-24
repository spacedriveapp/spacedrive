//! Get sync metrics action

use crate::ops::{LibraryQuery, LibraryQueryContext};
use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use anyhow::Result;

use super::{GetSyncMetricsInput, GetSyncMetricsOutput};

/// Get sync metrics for the current library
pub struct GetSyncMetrics;

impl LibraryQuery for GetSyncMetrics {
	type Input = GetSyncMetricsInput;
	type Output = GetSyncMetricsOutput;

	async fn execute(input: Self::Input, ctx: LibraryQueryContext) -> Result<Self::Output> {
		// Get the sync service from the library
		let sync_service = ctx.library.sync_service().await?;
		let metrics = sync_service.metrics();
		
		// Create a snapshot of current metrics
		let mut snapshot = SyncMetricsSnapshot::from_metrics(metrics).await;
		
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