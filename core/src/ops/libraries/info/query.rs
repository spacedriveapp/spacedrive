//! Library information query implementation

use super::output::LibraryInfoOutput;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing;
use uuid::Uuid;

/// Input for library info query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryInfoQueryInput;

/// Query to get detailed information about a specific library
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryInfoQuery;

impl LibraryInfoQuery {
	/// Create a new library info query
	pub fn new(_library_id: uuid::Uuid) -> Self {
		Self
	}
}

impl LibraryQuery for LibraryInfoQuery {
	type Input = LibraryInfoQueryInput;
	type Output = LibraryInfoOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
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

		// Get library configuration which contains all the details
		let config = library.config().await;

		// Get library path
		let path = library.path().to_path_buf();

		// Check if cached statistics are empty/stale (never calculated or all zeros)
		let cached_stats = config.statistics.clone();
		let is_stale = cached_stats.total_files == 0
			&& cached_stats.source_count == 0
			&& cached_stats.tag_count == 0;

		let statistics = if is_stale {
			// First load or completely empty — return zeros immediately and
			// calculate in the background.  The synchronous path used to
			// block here, but on large libraries (e.g. NAS with millions of
			// files being indexed) the closure-table walk in
			// calculate_file_statistics can take minutes, locking up the RPC
			// endpoint and making the UI unresponsive.  The background task
			// emits a ResourceChanged event when done so the UI refreshes.
			tracing::info!(
				library_id = %library_id,
				library_name = %config.name,
				"Cached statistics are empty, returning zeros and calculating in background"
			);

			if let Err(e) = library.recalculate_statistics().await {
				tracing::warn!(
					library_id = %library_id,
					library_name = %config.name,
					error = %e,
					"Failed to trigger background statistics calculation"
				);
			}

			cached_stats
		} else {
			// Return cached statistics immediately (non-blocking)
			tracing::debug!(
				library_id = %library_id,
				library_name = %config.name,
				"Returning cached statistics and triggering background recalculation"
			);

			// Trigger background recalculation (non-blocking)
			// This will emit a ResourceChanged event when complete
			if let Err(e) = library.recalculate_statistics().await {
				tracing::warn!(
					library_id = %library_id,
					library_name = %config.name,
					error = %e,
					"Failed to trigger background statistics recalculation"
				);
			}

			cached_stats
		};

		// Fleet totals: each paired device reports its own statistics during
		// peer sync, and they are added at read time rather than persisted,
		// so every device shows the same numbers and a stale peer figure
		// never outlives the next sync.
		let mut statistics = statistics;
		crate::service::mounts::peer::add_device_summaries(&mut statistics, library.db().conn())
			.await;

		tracing::debug!(
			library_id = %config.id,
			library_name = %config.name,
			total_files = statistics.total_files,
			total_size = statistics.total_size,
			source_count = statistics.source_count,
			tag_count = statistics.tag_count,
			device_count = statistics.device_count,
			unique_content_count = statistics.unique_content_count,
			total_capacity = statistics.total_capacity,
			available_capacity = statistics.available_capacity,
			database_size = statistics.database_size,
			sidecar_count = statistics.sidecar_count,
			sidecar_size = statistics.sidecar_size,
			"Returning library info with cached statistics"
		);

		Ok(LibraryInfoOutput {
			id: config.id,
			name: config.name,
			description: config.description,
			path,
			created_at: config.created_at,
			updated_at: config.updated_at,
			settings: config.settings,
			statistics,
		})
	}
}

crate::register_library_query!(LibraryInfoQuery, "libraries.info");
