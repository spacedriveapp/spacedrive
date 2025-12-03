//! Get sync activity action

use crate::context::CoreContext;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use std::sync::Arc;

use super::output::PeerActivity;
use super::{GetSyncActivityInput, GetSyncActivityOutput};

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

		// Get actual current state from peer sync (not metrics, which might lag)
		let current_state = sync_service.peer_sync().state().await;

		let metrics = sync_service.metrics();

		// Create a snapshot of current metrics
		let snapshot = SyncMetricsSnapshot::from_metrics(metrics.metrics()).await;

		// Get paired/connected devices from network layer
		let network = context.get_networking().await;
		let (paired_devices, connected_device_ids) = if let Some(net) = network.as_ref() {
			// Get paired devices (need to keep Arc alive and clone the result)
			let paired = {
				let registry_arc = net.device_registry();
				let registry = registry_arc.read().await;
				registry.get_paired_devices()
			};

			// Get connected devices
			let connected = net.get_connected_devices().await;
			let connected_ids: std::collections::HashSet<_> =
				connected.into_iter().map(|d| d.device_id).collect();
			(paired, connected_ids)
		} else {
			(Vec::new(), std::collections::HashSet::new())
		};

		// Build peer list from paired devices, enriched with metrics data and connection status
		let peers: Vec<PeerActivity> = paired_devices
			.into_iter()
			.map(|device_info| {
				// Try to get metrics for this device
				let device_metrics = snapshot
					.data_volume
					.entries_by_device
					.get(&device_info.device_id);

				// Check if device is actually connected at network level
				let is_online = connected_device_ids.contains(&device_info.device_id);

				PeerActivity {
					device_id: device_info.device_id,
					device_name: device_info.device_name.clone(),
					is_online,
					last_seen: device_metrics
						.map(|m| m.last_seen)
						.unwrap_or_else(|| chrono::Utc::now()),
					entries_received: device_metrics.map(|m| m.entries_received).unwrap_or(0),
					bytes_received: device_metrics
						.map(|_| snapshot.data_volume.bytes_received)
						.unwrap_or(0),
					bytes_sent: device_metrics
						.map(|_| snapshot.data_volume.bytes_sent)
						.unwrap_or(0),
					watermark_lag_ms: snapshot
						.performance
						.watermark_lag_ms
						.get(&device_info.device_id)
						.copied(),
				}
			})
			.collect();

		Ok(GetSyncActivityOutput {
			current_state,
			peers,
			error_count: snapshot.errors.total_errors,
		})
	}
}

// Register the query
crate::register_library_query!(GetSyncActivity, "sync.activity");
