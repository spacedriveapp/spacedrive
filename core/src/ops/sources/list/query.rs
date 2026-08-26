//! Source listing query implementation

use super::output::SourceInfo;
use crate::{
	context::CoreContext,
	infra::db::entities::source,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::indexing::ephemeral::SourceRecord,
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourcesInput {
	/// Filter by data type
	pub data_type: Option<String>,
}

/// Query to list all sources in the active library
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourcesQuery {
	pub input: ListSourcesInput,
}

impl ListSourcesQuery {
	pub fn all() -> Self {
		Self {
			input: ListSourcesInput { data_type: None },
		}
	}
}

impl LibraryQuery for ListSourcesQuery {
	type Input = ListSourcesInput;
	type Output = Vec<SourceInfo>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Get the active library from session
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		// Get or initialize the source manager
		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		// Filesystem sources come from the `sources` table; adapter sources
		// still come from `registry.db`. The union is transitional: folding the
		// adapter half into the same table is the rest of convergence P3, and
		// it changes where these rows are read rather than what they are.
		let mut result = Vec::new();

		let attached_mounts: std::collections::HashMap<uuid::Uuid, std::path::PathBuf> =
			crate::infra::db::entities::volume::Entity::find()
				.all(library.db().conn())
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to list volumes: {e}")))?
				.into_iter()
				.filter(|volume| volume.is_online)
				.filter_map(|volume| {
					Some((
						volume.uuid,
						std::path::PathBuf::from(volume.mount_point.as_ref()?),
					))
				})
				.collect();

		let rows = source::Entity::find()
			.all(library.db().conn())
			.await
			.map_err(|e| QueryError::Internal(format!("Failed to list sources: {e}")))?;

		for row in rows {
			if self
				.input
				.data_type
				.as_ref()
				.is_some_and(|filter| &row.data_type != filter)
			{
				continue;
			}

			let status = row.status.clone();
			let last_seen_at = Some(row.last_seen_at.to_rfc3339());
			let last_synced = row.last_indexed_at.map(|at| at.to_rfc3339());
			let adapter_id = row.adapter_id.clone();
			let data_type = row.data_type.clone();
			let mount = row
				.volume_uuid
				.and_then(|uuid| attached_mounts.get(&uuid).cloned());
			let record = SourceRecord::from_row(row, mount.as_deref());

			result.push(SourceInfo {
				id: record.id,
				name: record.name,
				data_type,
				adapter_id,
				item_count: record.record_count.unwrap_or(0) as i64,
				last_synced,
				status,
				attached: record.root.exists(),
				root: Some(record.root.to_string_lossy().into_owned()),
				volume_uuid: record.volume_uuid,
				total_bytes: record.total_bytes.map(|bytes| bytes as i64),
				last_seen_at,
			});
		}

		let sources = source_manager
			.list_sources()
			.await
			.map_err(|e| QueryError::Internal(format!("Failed to list sources: {e}")))?;

		for source in sources {
			// Apply data type filter if specified
			if let Some(ref filter) = self.input.data_type {
				if &source.data_type != filter {
					continue;
				}
			}

			let id = Uuid::parse_str(&source.id)
				.map_err(|e| QueryError::Internal(format!("Invalid source ID: {e}")))?;

			result.push(SourceInfo::adapter(
				id,
				source.name,
				source.data_type,
				source.adapter_id,
				source.item_count,
				source.last_synced,
				source.status,
			));
		}

		Ok(result)
	}
}

// Register library-scoped query
crate::register_library_query!(ListSourcesQuery, "sources.list");
