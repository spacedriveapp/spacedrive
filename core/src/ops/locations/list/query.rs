use super::output::{LocationInfo, LocationsListOutput};
use crate::domain::addressing::SdPath;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQuery;

impl LibraryQuery for LocationsListQuery {
	type Input = LocationsListQueryInput;
	type Output = LocationsListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db().conn();

		let rows = crate::infra::db::entities::location::Entity::find()
			.find_also_related(crate::infra::db::entities::entry::Entity)
			.all(db)
			.await?;

		let mut out = Vec::new();

		for (location, entry_opt) in rows {
			let entry = match entry_opt {
				Some(e) => e,
				None => {
					tracing::warn!(
						location_id = %location.uuid,
						"Location has no root entry, skipping"
					);
					continue;
				}
			};

			let directory_path = crate::infra::db::entities::directory_paths::Entity::find_by_id(entry.id)
				.one(db)
				.await?
				.ok_or_else(|| {
					QueryError::Internal(format!(
						"No directory path found for location {} entry {}",
						location.uuid, entry.id
					))
				})?;

			let device = crate::infra::db::entities::device::Entity::find_by_id(location.device_id)
				.one(db)
				.await?
				.ok_or_else(|| {
					QueryError::Internal(format!(
						"Device not found for location {}",
						location.uuid
					))
				})?;

			let sd_path = SdPath::Physical {
				device_slug: device.slug.clone(),
				path: directory_path.path.clone().into(),
			};

			let job_policies = location
				.job_policies
				.as_ref()
				.and_then(|json| serde_json::from_str(json).ok())
				.unwrap_or_default();

			out.push(LocationInfo {
				id: location.uuid,
				path: directory_path.path.clone().into(),
				name: location.name.clone(),
				sd_path,
				job_policies,
				index_mode: location.index_mode.clone(),
				scan_state: location.scan_state.clone(),
				last_scan_at: location.last_scan_at,
				error_message: location.error_message.clone(),
				total_file_count: location.total_file_count,
				total_byte_size: location.total_byte_size,
				created_at: location.created_at,
				updated_at: location.updated_at,
			});
		}

		Ok(LocationsListOutput { locations: out })
	}
}

crate::register_library_query!(LocationsListQuery, "locations.list");
