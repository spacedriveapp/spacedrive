use super::output::LocationsListOutput;
use crate::domain::{addressing::SdPath, Location};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::EntityTrait;
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

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
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

		for (location_model, entry_opt) in rows {
			let entry = match entry_opt {
				Some(e) => e,
				None => {
					tracing::warn!(
						location_id = %location_model.uuid,
						"Location has no root entry, skipping"
					);
					continue;
				}
			};

			let directory_path =
				crate::infra::db::entities::directory_paths::Entity::find_by_id(entry.id)
					.one(db)
					.await?
					.ok_or_else(|| {
						QueryError::Internal(format!(
							"No directory path found for location {} entry {}",
							location_model.uuid, entry.id
						))
					})?;

			let device =
				crate::infra::db::entities::device::Entity::find_by_id(location_model.device_id)
					.one(db)
					.await?
					.ok_or_else(|| {
						QueryError::Internal(format!(
							"Device not found for location {}",
							location_model.uuid
						))
					})?;

			let sd_path = SdPath::Physical {
				device_slug: device.slug.clone(),
				path: directory_path.path.clone().into(),
			};

			out.push(Location::from_db_model(
				&location_model,
				library_id,
				sd_path,
			));
		}

		Ok(LocationsListOutput { locations: out })
	}
}

crate::register_library_query!(LocationsListQuery, "locations.list");
