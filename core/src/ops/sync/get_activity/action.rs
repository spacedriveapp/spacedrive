//! Get sync activity action

use crate::context::CoreContext;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use std::sync::Arc;

use super::{GetSyncActivityInput, GetSyncActivityOutput};
use super::output::PeerActivity;

/// Get sync activity summary for the current library
pub struct GetSyncActivity {
	pub input: GetSyncActivityInput,
}

impl LibraryQuery for GetSyncActivity {
	type Input = GetSyncActivityInput;
	type Output = GetSyncActivityOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
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

		let sync_service = library
			.sync_service()
			.ok_or_else(|| QueryError::Internal("Sync service not available".to_string()))?;
		let metrics = sync_service.metrics();

		// Create a snapshot of current metrics
		let snapshot = SyncMetricsSnapshot::from_metrics(metrics.metrics()).await;

		// Transform into activity summary
		let peers: Vec<PeerActivity> = snapshot
			.data_volume
			.entries_by_device
			.into_iter()
			.map(|(device_id, device_metrics)| PeerActivity {
				device_id,
				device_name: device_metrics.device_name,
				is_online: device_metrics.is_online,
				last_seen: device_metrics.last_seen,
				entries_received: device_metrics.entries_received,
				bytes_received: snapshot.data_volume.bytes_received,
				bytes_sent: snapshot.data_volume.bytes_sent,
				watermark_lag_ms: snapshot.performance.watermark_lag_ms.get(&device_id).copied(),
			})
			.collect();

		Ok(GetSyncActivityOutput {
			current_state: snapshot.state.current_state,
			peers,
			error_count: snapshot.errors.total_errors,
		})
	}
}

// Register the query
crate::register_library_query!(GetSyncActivity, "sync.activity");
