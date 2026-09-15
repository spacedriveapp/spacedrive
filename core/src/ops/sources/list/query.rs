//! Source listing query implementation

use super::output::SourceInfo;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
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

		// One table, whatever fills a source. `data_type` is what forks them,
		// matching `_schema.data_type_id` in the source's own store.
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

		let rows = registry::all(library.db().conn())
			.await
			.map_err(|e| QueryError::Internal(format!("Failed to list sources: {e}")))?;

		let mut sources: Vec<SourceInfo> = rows
			.into_iter()
			.filter(|row| {
				self.input
					.data_type
					.as_ref()
					.is_none_or(|filter| &row.data_type == filter)
			})
			.map(|row| {
				let mount = row
					.volume_uuid
					.and_then(|uuid| attached_mounts.get(&uuid).cloned());
				SourceInfo::from_row(row, mount.as_deref())
			})
			.collect();

		// Paired devices' sources, already replicated by the peer-mount
		// plane. They live in memory rather than the library database, so
		// they are appended here instead of joined above.
		if self
			.input
			.data_type
			.as_ref()
			.is_none_or(|filter| filter == "filesystem")
		{
			for share in crate::service::mounts::peer::remote_shares().await {
				sources.push(SourceInfo::from_remote_share(&share));
			}
		}

		Ok(sources)
	}
}

crate::register_library_query!(ListSourcesQuery, "sources.list");
