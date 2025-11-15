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
			&& cached_stats.location_count == 0
			&& cached_stats.tag_count == 0;

		let statistics = if is_stale {
			// First load or completely empty - calculate synchronously
			tracing::debug!(
				library_id = %library_id,
				library_name = %config.name,
				"Cached statistics are empty, calculating synchronously for first load"
			);

			let stats = library
				.calculate_statistics_for_query()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to calculate statistics: {}", e)))?;

			// Also trigger background save and event emission
			// (non-blocking, happens after we return the stats to the user)
			if let Err(e) = library.recalculate_statistics().await {
				tracing::warn!(
					library_id = %library_id,
					library_name = %config.name,
					error = %e,
					"Failed to trigger background statistics save after sync calculation"
				);
			}

			stats
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

		tracing::debug!(
			library_id = %config.id,
			library_name = %config.name,
			total_files = statistics.total_files,
			total_size = statistics.total_size,
			location_count = statistics.location_count,
			tag_count = statistics.tag_count,
			device_count = statistics.device_count,
			total_capacity = statistics.total_capacity,
			available_capacity = statistics.available_capacity,
			database_size = statistics.database_size,
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
