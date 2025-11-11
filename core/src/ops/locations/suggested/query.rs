use super::output::{SuggestedLocation, SuggestedLocationsOutput};
use crate::domain::addressing::SdPath;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocationsQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocationsQuery;

impl LibraryQuery for SuggestedLocationsQuery {
	type Input = SuggestedLocationsQueryInput;
	type Output = SuggestedLocationsOutput;

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

		// Get current device from session
		let current_device_uuid = session.auth.device_id;

		// Find the device record in the database
		let device = crate::infra::db::entities::device::Entity::find()
			.filter(crate::infra::db::entities::device::Column::Uuid.eq(current_device_uuid))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal("Current device not found in library".to_string()))?;

		// Get all existing locations for this device with their paths
		let existing_locations = crate::infra::db::entities::location::Entity::find()
			.filter(crate::infra::db::entities::location::Column::DeviceId.eq(device.id))
			.find_also_related(crate::infra::db::entities::entry::Entity)
			.all(db)
			.await?;

		// Collect existing location paths
		let mut existing_paths = std::collections::HashSet::new();
		for (location, entry_opt) in existing_locations {
			if let Some(entry) = entry_opt {
				if let Ok(Some(dir_path)) =
					crate::infra::db::entities::directory_paths::Entity::find_by_id(entry.id)
						.one(db)
						.await
				{
					existing_paths.insert(PathBuf::from(dir_path.path));
				}
			}
		}

		// Get suggested locations based on OS
		let suggestions = get_suggested_locations_for_os();

		// Filter to only those that exist and aren't already locations
		let mut result = Vec::new();
		for (name, path) in suggestions {
			// Check if exists on filesystem
			if !path.exists() {
				continue;
			}

			// Check if already a location
			if existing_paths.contains(&path) {
				continue;
			}

			let sd_path = SdPath::Physical {
				device_slug: device.slug.clone(),
				path: path.clone(),
			};

			result.push(SuggestedLocation {
				name,
				path,
				sd_path,
			});
		}

		Ok(SuggestedLocationsOutput { locations: result })
	}
}

fn get_suggested_locations_for_os() -> Vec<(String, PathBuf)> {
	let mut suggestions = Vec::new();

	// Get home directory
	let home = match dirs::home_dir() {
		Some(h) => h,
		None => return suggestions,
	};

	if cfg!(target_os = "macos") {
		suggestions.push(("Desktop".to_string(), home.join("Desktop")));
		suggestions.push(("Documents".to_string(), home.join("Documents")));
		suggestions.push(("Downloads".to_string(), home.join("Downloads")));
		suggestions.push(("Pictures".to_string(), home.join("Pictures")));
		suggestions.push(("Music".to_string(), home.join("Music")));
		suggestions.push(("Movies".to_string(), home.join("Movies")));
	} else if cfg!(target_os = "linux") {
		suggestions.push(("Desktop".to_string(), home.join("Desktop")));
		suggestions.push(("Documents".to_string(), home.join("Documents")));
		suggestions.push(("Downloads".to_string(), home.join("Downloads")));
		suggestions.push(("Pictures".to_string(), home.join("Pictures")));
		suggestions.push(("Music".to_string(), home.join("Music")));
		suggestions.push(("Videos".to_string(), home.join("Videos")));
	} else if cfg!(target_os = "windows") {
		suggestions.push(("Desktop".to_string(), home.join("Desktop")));
		suggestions.push(("Documents".to_string(), home.join("Documents")));
		suggestions.push(("Downloads".to_string(), home.join("Downloads")));
		suggestions.push(("Pictures".to_string(), home.join("Pictures")));
		suggestions.push(("Music".to_string(), home.join("Music")));
		suggestions.push(("Videos".to_string(), home.join("Videos")));
	}

	suggestions
}

crate::register_library_query!(SuggestedLocationsQuery, "locations.suggested");
