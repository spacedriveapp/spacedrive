//! Update location service settings action

use crate::{
	context::CoreContext,
	domain::location::{LocationServiceSettings, StaleDetectorConfig, StaleDetectorSettings, SyncConfig, SyncSettings, WatcherConfig, WatcherSettings},
	infra::{
		action::{error::ActionError, LibraryAction, ValidationResult},
		db::entities::location_service_settings,
	},
	library::Library,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for updating location service settings
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLocationServiceSettingsInput {
	pub location_id: Uuid,
	pub watcher_enabled: Option<bool>,
	pub watcher_config: Option<WatcherConfig>,
	pub stale_detector_enabled: Option<bool>,
	pub stale_detector_config: Option<StaleDetectorConfig>,
	pub sync_enabled: Option<bool>,
	pub sync_config: Option<SyncConfig>,
}

/// Output indicating success
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLocationServiceSettingsOutput {
	pub success: bool,
	pub settings: LocationServiceSettings,
}

/// Action to update service settings for a location
#[derive(Debug, Clone)]
pub struct UpdateLocationServiceSettingsAction {
	pub input: UpdateLocationServiceSettingsInput,
}

impl LibraryAction for UpdateLocationServiceSettingsAction {
	type Input = UpdateLocationServiceSettingsInput;
	type Output = UpdateLocationServiceSettingsOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn validate(
		&self,
		_library: &Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		Ok(ValidationResult::Success)
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Get location database ID
		use crate::infra::db::entities::location;
		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(self.input.location_id))
			.one(db)
			.await?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.location_id))?;

		// Get or create service settings
		let existing = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(loc.id))
			.one(db)
			.await?;

		let now = chrono::Utc::now().into();

		let (watcher_enabled, watcher_config, stale_detector_enabled, stale_detector_config, sync_enabled, sync_config) = match &existing {
			Some(model) => {
				let we = self.input.watcher_enabled.unwrap_or(model.watcher_enabled);
				let wc = self.input.watcher_config.clone().unwrap_or_else(|| {
					model.watcher_config.as_ref()
						.and_then(|c| serde_json::from_str::<WatcherConfig>(c).ok())
						.unwrap_or_default()
				});
				let sde = self.input.stale_detector_enabled.unwrap_or(model.stale_detector_enabled);
				let sdc = self.input.stale_detector_config.clone().unwrap_or_else(|| {
					model.stale_detector_config.as_ref()
						.and_then(|c| serde_json::from_str::<StaleDetectorConfig>(c).ok())
						.unwrap_or_default()
				});
				let se = self.input.sync_enabled.unwrap_or(model.sync_enabled);
				let sc = self.input.sync_config.clone().unwrap_or_else(|| {
					model.sync_config.as_ref()
						.and_then(|c| serde_json::from_str::<SyncConfig>(c).ok())
						.unwrap_or_default()
				});
				(we, wc, sde, sdc, se, sc)
			}
			None => {
				let defaults = LocationServiceSettings::default_for_location(self.input.location_id);
				(
					self.input.watcher_enabled.unwrap_or(defaults.watcher.enabled),
					self.input.watcher_config.clone().unwrap_or(defaults.watcher.config),
					self.input.stale_detector_enabled.unwrap_or(defaults.stale_detector.enabled),
					self.input.stale_detector_config.clone().unwrap_or(defaults.stale_detector.config),
					self.input.sync_enabled.unwrap_or(defaults.sync.enabled),
					self.input.sync_config.clone().unwrap_or(defaults.sync.config),
				)
			}
		};

		let watcher_config_json = serde_json::to_string(&watcher_config)?;
		let stale_detector_config_json = serde_json::to_string(&stale_detector_config)?;
		let sync_config_json = serde_json::to_string(&sync_config)?;

		if let Some(model) = existing {
			// Update existing
			let mut active: location_service_settings::ActiveModel = model.into();
			active.watcher_enabled = Set(watcher_enabled);
			active.watcher_config = Set(Some(watcher_config_json));
			active.stale_detector_enabled = Set(stale_detector_enabled);
			active.stale_detector_config = Set(Some(stale_detector_config_json));
			active.sync_enabled = Set(sync_enabled);
			active.sync_config = Set(Some(sync_config_json));
			active.updated_at = Set(now);
			active.update(db).await?;
		} else {
			// Insert new
			let active = location_service_settings::ActiveModel {
				location_id: Set(loc.id),
				watcher_enabled: Set(watcher_enabled),
				watcher_config: Set(Some(watcher_config_json)),
				stale_detector_enabled: Set(stale_detector_enabled),
				stale_detector_config: Set(Some(stale_detector_config_json)),
				sync_enabled: Set(sync_enabled),
				sync_config: Set(Some(sync_config_json)),
				created_at: Set(now),
				updated_at: Set(now),
			};
			active.insert(db).await?;
		}

		let settings = LocationServiceSettings {
			location_id: self.input.location_id,
			watcher: WatcherSettings {
				enabled: watcher_enabled,
				config: watcher_config,
			},
			stale_detector: StaleDetectorSettings {
				enabled: stale_detector_enabled,
				config: stale_detector_config,
			},
			sync: SyncSettings {
				enabled: sync_enabled,
				config: sync_config,
			},
		};

		Ok(UpdateLocationServiceSettingsOutput {
			success: true,
			settings,
		})
	}

	fn action_kind(&self) -> &'static str {
		"locations.updateServiceSettings"
	}
}

crate::register_library_action!(UpdateLocationServiceSettingsAction, "locations.updateServiceSettings");
