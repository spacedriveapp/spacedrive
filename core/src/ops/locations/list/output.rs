use crate::domain::{location::JobPolicies, resource::Identifiable, SdPath};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationInfo {
	pub id: Uuid,
	pub path: PathBuf,
	pub name: Option<String>,
	pub sd_path: SdPath,
	#[serde(default)]
	pub job_policies: JobPolicies,
	pub index_mode: String,
	pub scan_state: String,
	pub last_scan_at: Option<DateTimeUtc>,
	pub error_message: Option<String>,
	pub total_file_count: i64,
	pub total_byte_size: i64,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

impl Identifiable for LocationInfo {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"location"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::domain::addressing::SdPath;
		use crate::infra::db::entities::{device, directory_paths, entry, location};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let locations_with_entries = location::Entity::find()
			.filter(location::Column::Uuid.is_in(ids.to_vec()))
			.find_also_related(entry::Entity)
			.all(db)
			.await
			.map_err(|e| crate::common::errors::CoreError::Database(e.to_string()))?;

		let mut results = Vec::new();

		for (loc, entry_opt) in locations_with_entries {
			let Some(entry) = entry_opt else {
				tracing::warn!("Location {} has no root entry, skipping", loc.uuid);
				continue;
			};

			let Some(dir_path) = directory_paths::Entity::find_by_id(entry.id)
				.one(db)
				.await
				.map_err(|e| crate::common::errors::CoreError::Database(e.to_string()))?
			else {
				tracing::warn!(
					"No directory path for location {} entry {}",
					loc.uuid,
					entry.id
				);
				continue;
			};

			let Some(device_model) = device::Entity::find_by_id(loc.device_id)
				.one(db)
				.await
				.map_err(|e| crate::common::errors::CoreError::Database(e.to_string()))?
			else {
				tracing::warn!("Device not found for location {}", loc.uuid);
				continue;
			};

			let sd_path = SdPath::Physical {
				device_slug: device_model.slug.clone(),
				path: dir_path.path.clone().into(),
			};

			let job_policies = loc
				.job_policies
				.as_ref()
				.and_then(|json| serde_json::from_str(json).ok())
				.unwrap_or_default();

			results.push(LocationInfo {
				id: loc.uuid,
				path: dir_path.path.into(),
				name: loc.name.clone(),
				sd_path,
				job_policies,
				index_mode: loc.index_mode.clone(),
				scan_state: loc.scan_state.clone(),
				last_scan_at: loc.last_scan_at,
				error_message: loc.error_message.clone(),
				total_file_count: loc.total_file_count,
				total_byte_size: loc.total_byte_size,
				created_at: loc.created_at,
				updated_at: loc.updated_at,
			});
		}

		Ok(results)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListOutput {
	pub locations: Vec<LocationInfo>,
}
