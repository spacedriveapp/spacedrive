//! Get location service settings query

use crate::{
	context::CoreContext,
	domain::location::{LocationServiceSettings, StaleDetectorConfig, StaleDetectorSettings, SyncConfig, SyncSettings, WatcherConfig, WatcherSettings},
	infra::{
		db::entities::location_service_settings,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for getting location service settings
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetLocationServiceSettingsInput {
	pub location_id: Uuid,
}

/// Output containing service settings
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetLocationServiceSettingsOutput {
	pub settings: LocationServiceSettings,
}

/// Query to get service settings for a location
#[derive(Debug, Clone)]
pub struct GetLocationServiceSettingsQuery {
	pub location_id: Uuid,
}

impl LibraryQuery for GetLocationServiceSettingsQuery {
	type Input = GetLocationServiceSettingsInput;
	type Output = GetLocationServiceSettingsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			location_id: input.location_id,
		})
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

		// Get location database ID
		use crate::infra::db::entities::location;
		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(self.location_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::LocationNotFound(self.location_id))?;

		// Get service settings
		let settings = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(loc.id))
			.one(db)
			.await?;

		let settings = match settings {
			Some(model) => {
				let watcher_config = model
					.watcher_config
					.as_ref()
					.and_then(|c| serde_json::from_str::<WatcherConfig>(c).ok())
					.unwrap_or_default();

				let stale_detector_config = model
					.stale_detector_config
					.as_ref()
					.and_then(|c| serde_json::from_str::<StaleDetectorConfig>(c).ok())
					.unwrap_or_default();

				let sync_config = model
					.sync_config
					.as_ref()
					.and_then(|c| serde_json::from_str::<SyncConfig>(c).ok())
					.unwrap_or_default();

				LocationServiceSettings {
					location_id: self.location_id,
					watcher: WatcherSettings {
						enabled: model.watcher_enabled,
						config: watcher_config,
					},
					stale_detector: StaleDetectorSettings {
						enabled: model.stale_detector_enabled,
						config: stale_detector_config,
					},
					sync: SyncSettings {
						enabled: model.sync_enabled,
						config: sync_config,
					},
				}
			}
			None => LocationServiceSettings::default_for_location(self.location_id),
		};

		Ok(GetLocationServiceSettingsOutput { settings })
	}
}

crate::register_library_query!(GetLocationServiceSettingsQuery, "locations.getServiceSettings");
