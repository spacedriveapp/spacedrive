//! Service Coordinator
//!
//! Coordinates service lifecycle based on location settings. Manages per-location
//! configuration for watcher, stale detector, and sync services.

use crate::{
	domain::location::LocationServiceSettings,
	infra::db::entities::{location, location_service_settings},
};
use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Coordinates service lifecycle based on location settings
pub struct ServiceCoordinator {
	db: Arc<sea_orm::DatabaseConnection>,
	watcher_service: Option<Arc<crate::service::watcher::FsWatcherService>>,
	stale_detector_service: Option<Arc<crate::service::stale_detector::StaleDetectionService>>,
	sync_service: Option<Arc<()>>, // Placeholder for future sync service
}

impl ServiceCoordinator {
	/// Create a new service coordinator
	pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
		Self {
			db,
			watcher_service: None,
			stale_detector_service: None,
			sync_service: None,
		}
	}

	/// Set the watcher service
	pub fn set_watcher_service(&mut self, service: Arc<crate::service::watcher::FsWatcherService>) {
		self.watcher_service = Some(service);
	}

	/// Set the stale detector service
	pub fn set_stale_detector_service(
		&mut self,
		service: Arc<crate::service::stale_detector::StaleDetectionService>,
	) {
		self.stale_detector_service = Some(service);
	}

	/// Apply service settings to a location
	pub async fn apply_location_settings(
		&self,
		location_id: Uuid,
		settings: LocationServiceSettings,
	) -> Result<()> {
		// Get location's database ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		// Save settings to database
		self.save_location_settings(location_model.id, &settings).await?;

		// Apply watcher settings
		if let Some(watcher) = &self.watcher_service {
			if settings.watcher.enabled {
				// Watcher would be enabled via watch_location call
				info!("Watcher enabled for location {}", location_id);
			} else {
				// Unwatch location
				info!("Watcher disabled for location {}", location_id);
			}
		}

		// Apply stale detector settings
		if let Some(stale_detector) = &self.stale_detector_service {
			if settings.stale_detector.enabled {
				info!("Stale detector enabled for location {}", location_id);
				// Worker would be started by stale detector service
			} else {
				info!("Stale detector disabled for location {}", location_id);
				// Worker would be stopped by stale detector service
			}
		}

		// Apply sync settings (placeholder)
		if let Some(_sync) = &self.sync_service {
			if settings.sync.enabled {
				info!("Sync enabled for location {}", location_id);
			} else {
				info!("Sync disabled for location {}", location_id);
			}
		}

		Ok(())
	}

	/// Get current settings for a location
	pub async fn get_location_settings(
		&self,
		location_id: Uuid,
	) -> Result<LocationServiceSettings> {
		// Get location's database ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let settings_model = location_service_settings::Entity::find_by_id(location_model.id)
			.one(&*self.db)
			.await
			.context("Failed to query service settings")?;

		if let Some(model) = settings_model {
			Ok(LocationServiceSettings {
				location_id,
				watcher: crate::domain::location::WatcherSettings {
					enabled: model.watcher_enabled,
					config: model
						.watcher_config
						.as_ref()
						.and_then(|json| serde_json::from_str(json).ok())
						.unwrap_or_default(),
				},
				stale_detector: crate::domain::location::StaleDetectorSettings {
					enabled: model.stale_detector_enabled,
					config: model
						.stale_detector_config
						.as_ref()
						.and_then(|json| serde_json::from_str(json).ok())
						.unwrap_or_default(),
				},
				sync: crate::domain::location::SyncSettings {
					enabled: model.sync_enabled,
					config: model
						.sync_config
						.as_ref()
						.and_then(|json| serde_json::from_str(json).ok())
						.unwrap_or_default(),
				},
			})
		} else {
			// Return defaults if not configured
			Ok(LocationServiceSettings {
				location_id,
				watcher: crate::domain::location::WatcherSettings {
					enabled: true,
					config: Default::default(),
				},
				stale_detector: crate::domain::location::StaleDetectorSettings {
					enabled: true,
					config: Default::default(),
				},
				sync: crate::domain::location::SyncSettings {
					enabled: false,
					config: Default::default(),
				},
			})
		}
	}

	/// Initialize default settings when location is created
	pub async fn initialize_default_settings(&self, location_id: Uuid) -> Result<()> {
		// Get location's database ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let default_settings = LocationServiceSettings {
			location_id,
			watcher: crate::domain::location::WatcherSettings {
				enabled: true,
				config: Default::default(),
			},
			stale_detector: crate::domain::location::StaleDetectorSettings {
				enabled: true,
				config: Default::default(),
			},
			sync: crate::domain::location::SyncSettings {
				enabled: false,
				config: Default::default(),
			},
		};

		self.save_location_settings(location_model.id, &default_settings)
			.await?;

		Ok(())
	}

	/// Save location settings to database
	async fn save_location_settings(
		&self,
		location_db_id: i32,
		settings: &LocationServiceSettings,
	) -> Result<()> {
		use location_service_settings::ActiveModel;

		let watcher_config_json = serde_json::to_string(&settings.watcher.config)
			.context("Failed to serialize watcher config")?;
		let stale_detector_config_json = serde_json::to_string(&settings.stale_detector.config)
			.context("Failed to serialize stale detector config")?;
		let sync_config_json = serde_json::to_string(&settings.sync.config)
			.context("Failed to serialize sync config")?;

		let active_model = ActiveModel {
			location_id: Set(location_db_id),
			watcher_enabled: Set(settings.watcher.enabled),
			watcher_config: Set(Some(watcher_config_json)),
			stale_detector_enabled: Set(settings.stale_detector.enabled),
			stale_detector_config: Set(Some(stale_detector_config_json)),
			sync_enabled: Set(settings.sync.enabled),
			sync_config: Set(Some(sync_config_json)),
			updated_at: Set(chrono::Utc::now()),
			..Default::default()
		};

		location_service_settings::Entity::insert(active_model)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(location_service_settings::Column::LocationId)
					.update_columns([
						location_service_settings::Column::WatcherEnabled,
						location_service_settings::Column::WatcherConfig,
						location_service_settings::Column::StaleDetectorEnabled,
						location_service_settings::Column::StaleDetectorConfig,
						location_service_settings::Column::SyncEnabled,
						location_service_settings::Column::SyncConfig,
						location_service_settings::Column::UpdatedAt,
					])
					.to_owned(),
			)
			.exec(&*self.db)
			.await
			.context("Failed to save location settings")?;

		Ok(())
	}

	/// Stop all services for a location
	pub async fn stop_location_services(&self, _location_id: Uuid) -> Result<()> {
		// This would stop watcher, stale detector workers, etc.
		// Implementation depends on service APIs
		Ok(())
	}
}
