//! Service Coordinator
//!
//! Coordinates service lifecycle based on location settings. Manages per-location
//! configuration for watcher, stale detector, and sync services.
//!
//! ## Responsibility
//!
//! The ServiceCoordinator acts as the central point for managing background services
//! on a per-location basis. It handles:
//! - Loading/saving service settings from database
//! - Applying settings changes to running services
//! - Initializing default settings for new locations
//! - Stopping services when locations are removed

use crate::{
	context::CoreContext,
	domain::location::{
		LocationServiceSettings, StaleDetectorConfig, StaleDetectorSettings, SyncConfig,
		SyncSettings, WatcherConfig, WatcherSettings,
	},
	infra::db::entities::location_service_settings,
	library::Library,
	service::{stale_detector::StaleDetectionService, watcher::FsWatcherService},
};
use anyhow::Result;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Coordinates service lifecycle based on location settings
pub struct ServiceCoordinator {
	library: Arc<Library>,
	context: Arc<CoreContext>,
	watcher_service: Arc<FsWatcherService>,
	stale_detector_service: Option<Arc<StaleDetectionService>>,
}

impl ServiceCoordinator {
	/// Create a new service coordinator
	pub fn new(
		library: Arc<Library>,
		context: Arc<CoreContext>,
		watcher_service: Arc<FsWatcherService>,
	) -> Self {
		Self {
			library,
			context,
			watcher_service,
			stale_detector_service: None,
		}
	}

	/// Set the stale detector service (called after initialization)
	pub fn set_stale_detector(&mut self, service: Arc<StaleDetectionService>) {
		self.stale_detector_service = Some(service);
	}

	/// Apply service settings to a location
	///
	/// Updates the database and configures running services to match the new settings.
	pub async fn apply_location_settings(
		&self,
		location_id: Uuid,
		settings: LocationServiceSettings,
	) -> Result<()> {
		info!(
			location_id = %location_id,
			"Applying service settings"
		);

		// Save settings to database
		self.save_location_settings(location_id, &settings).await?;

		// Get the database ID for this location
		let db_location_id = self.get_location_db_id(location_id).await?;

		// Apply watcher settings
		if settings.watcher.enabled {
			// Watcher service manages its own location watching
			debug!(
				location_id = %location_id,
				"Watcher enabled for location"
			);
		} else {
			// Stop watching this location
			if let Err(e) = self.watcher_service.unwatch_location(location_id).await {
				warn!(
					location_id = %location_id,
					error = %e,
					"Failed to unwatch location"
				);
			}
		}

		// Apply stale detector settings
		if let Some(stale_detector) = &self.stale_detector_service {
			if settings.stale_detector.enabled {
				if let Err(e) = stale_detector
					.enable_for_location(location_id, Some(settings.stale_detector.config.clone()))
					.await
				{
					warn!(
						location_id = %location_id,
						error = %e,
						"Failed to enable stale detection"
					);
				}
			} else {
				if let Err(e) = stale_detector.disable_for_location(location_id).await {
					warn!(
						location_id = %location_id,
						error = %e,
						"Failed to disable stale detection"
					);
				}
			}
		}

		// Sync service settings will be applied when sync service is available
		if settings.sync.enabled {
			debug!(
				location_id = %location_id,
				"Sync enabled for location (pending sync service implementation)"
			);
		}

		Ok(())
	}

	/// Get current settings for a location
	pub async fn get_location_settings(
		&self,
		location_id: Uuid,
	) -> Result<LocationServiceSettings> {
		let db = self.library.db().conn();
		let db_location_id = self.get_location_db_id(location_id).await?;

		let settings = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(db_location_id))
			.one(db)
			.await?;

		match settings {
			Some(model) => Ok(self.model_to_domain(location_id, model)),
			None => {
				// Return defaults if no settings exist
				Ok(LocationServiceSettings::default_for_location(location_id))
			}
		}
	}

	/// Initialize default settings when location is created
	pub async fn initialize_default_settings(&self, location_id: Uuid) -> Result<()> {
		info!(
			location_id = %location_id,
			"Initializing default service settings"
		);

		let settings = LocationServiceSettings::default_for_location(location_id);
		self.apply_location_settings(location_id, settings).await
	}

	/// Stop all services for a location
	pub async fn stop_location_services(&self, location_id: Uuid) -> Result<()> {
		info!(
			location_id = %location_id,
			"Stopping all services for location"
		);

		// Stop watcher
		if let Err(e) = self.watcher_service.unwatch_location(location_id).await {
			warn!(
				location_id = %location_id,
				error = %e,
				"Failed to stop watcher for location"
			);
		}

		// Stop stale detector
		if let Some(stale_detector) = &self.stale_detector_service {
			if let Err(e) = stale_detector.disable_for_location(location_id).await {
				warn!(
					location_id = %location_id,
					error = %e,
					"Failed to stop stale detector for location"
				);
			}
		}

		// Delete settings from database
		self.delete_location_settings(location_id).await?;

		Ok(())
	}

	/// Save location settings to database
	async fn save_location_settings(
		&self,
		location_id: Uuid,
		settings: &LocationServiceSettings,
	) -> Result<()> {
		let db = self.library.db().conn();
		let db_location_id = self.get_location_db_id(location_id).await?;

		// Serialize configs to JSON
		let watcher_config = serde_json::to_string(&settings.watcher.config)?;
		let stale_detector_config = serde_json::to_string(&settings.stale_detector.config)?;
		let sync_config = serde_json::to_string(&settings.sync.config)?;

		// Check if settings exist
		let existing = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(db_location_id))
			.one(db)
			.await?;

		if let Some(existing) = existing {
			// Update existing
			let mut active: location_service_settings::ActiveModel = existing.into();
			active.watcher_enabled = Set(settings.watcher.enabled);
			active.watcher_config = Set(Some(watcher_config));
			active.stale_detector_enabled = Set(settings.stale_detector.enabled);
			active.stale_detector_config = Set(Some(stale_detector_config));
			active.sync_enabled = Set(settings.sync.enabled);
			active.sync_config = Set(Some(sync_config));
			active.updated_at = Set(chrono::Utc::now().into());
			active.update(db).await?;
		} else {
			// Insert new
			let active = location_service_settings::ActiveModel {
				location_id: Set(db_location_id),
				watcher_enabled: Set(settings.watcher.enabled),
				watcher_config: Set(Some(watcher_config)),
				stale_detector_enabled: Set(settings.stale_detector.enabled),
				stale_detector_config: Set(Some(stale_detector_config)),
				sync_enabled: Set(settings.sync.enabled),
				sync_config: Set(Some(sync_config)),
				created_at: Set(chrono::Utc::now().into()),
				updated_at: Set(chrono::Utc::now().into()),
			};
			active.insert(db).await?;
		}

		debug!(
			location_id = %location_id,
			"Saved service settings"
		);
		Ok(())
	}

	/// Delete location settings from database
	async fn delete_location_settings(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_location_id = self.get_location_db_id(location_id).await?;

		location_service_settings::Entity::delete_by_id(db_location_id)
			.exec(db)
			.await?;

		debug!(
			location_id = %location_id,
			"Deleted service settings"
		);
		Ok(())
	}

	/// Get location database ID from UUID
	async fn get_location_db_id(&self, location_id: Uuid) -> Result<i32> {
		use crate::infra::db::entities::location;

		let db = self.library.db().conn();
		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;
		Ok(loc.id)
	}

	/// Convert database model to domain model
	fn model_to_domain(
		&self,
		location_id: Uuid,
		model: location_service_settings::Model,
	) -> LocationServiceSettings {
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
			location_id,
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

	/// Ensure settings exist for a location, creating defaults if needed
	pub async fn ensure_settings_exist(&self, location_id: Uuid) -> Result<LocationServiceSettings> {
		let db = self.library.db().conn();
		let db_location_id = self.get_location_db_id(location_id).await?;

		let existing = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(db_location_id))
			.one(db)
			.await?;

		if let Some(model) = existing {
			Ok(self.model_to_domain(location_id, model))
		} else {
			let defaults = LocationServiceSettings::default_for_location(location_id);
			self.save_location_settings(location_id, &defaults).await?;
			Ok(defaults)
		}
	}

	/// Update watcher settings for a location
	pub async fn update_watcher_settings(
		&self,
		location_id: Uuid,
		settings: WatcherSettings,
	) -> Result<()> {
		let mut current = self.get_location_settings(location_id).await?;
		current.watcher = settings;
		self.apply_location_settings(location_id, current).await
	}

	/// Update stale detector settings for a location
	pub async fn update_stale_detector_settings(
		&self,
		location_id: Uuid,
		settings: StaleDetectorSettings,
	) -> Result<()> {
		let mut current = self.get_location_settings(location_id).await?;
		current.stale_detector = settings;
		self.apply_location_settings(location_id, current).await
	}

	/// Update sync settings for a location
	pub async fn update_sync_settings(
		&self,
		location_id: Uuid,
		settings: SyncSettings,
	) -> Result<()> {
		let mut current = self.get_location_settings(location_id).await?;
		current.sync = settings;
		self.apply_location_settings(location_id, current).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_settings() {
		let location_id = Uuid::new_v4();
		let settings = LocationServiceSettings::default_for_location(location_id);

		assert_eq!(settings.location_id, location_id);
		assert!(settings.watcher.enabled);
		assert!(settings.stale_detector.enabled);
		assert!(!settings.sync.enabled);
	}
}
