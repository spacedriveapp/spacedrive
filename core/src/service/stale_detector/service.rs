//! Stale Detection Service implementation

use crate::{
	domain::location::{IndexMode, Location, StaleDetectionTrigger, StaleDetectorSettings},
	infra::{
		db::entities,
		job::manager::JobManager,
	},
	library::Library,
	ops::indexing::{IndexerJob, IndexerJobConfig, IndexPersistence, IndexScope},
};
use sea_orm::{
	ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{Notify, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use super::worker::LocationWorker;

pub struct StaleDetectionService {
	db: Arc<DatabaseConnection>,
	job_manager: Arc<JobManager>,
	library: Arc<Library>,

	/// Per-location worker tasks
	location_workers: Arc<RwLock<HashMap<Uuid, LocationWorker>>>,

	/// Shutdown signal
	shutdown: Arc<Notify>,
}

impl StaleDetectionService {
	/// Create a new stale detection service
	pub fn new(
		db: Arc<DatabaseConnection>,
		job_manager: Arc<JobManager>,
		library: Arc<Library>,
	) -> Self {
		Self {
			db,
			job_manager,
			library,
			location_workers: Arc::new(RwLock::new(HashMap::new())),
			shutdown: Arc::new(Notify::new()),
		}
	}

	/// Start the service and initialize workers for all enabled locations
	pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
		info!("Starting stale detection service");

		// Load all locations with stale detection enabled
		let locations = self.get_enabled_locations().await?;

		for location in locations {
			self.enable_for_location(location.id, location.settings)
				.await?;
		}

		Ok(())
	}

	/// Stop the service and all workers
	pub async fn stop(&self) {
		info!("Stopping stale detection service");
		self.shutdown.notify_waiters();

		let mut workers = self.location_workers.write().await;
		workers.clear();
	}

	/// Trigger stale detection for a location manually
	pub async fn detect_stale(
		&self,
		location_id: Uuid,
		location_path: PathBuf,
		trigger: StaleDetectionTrigger,
	) -> Result<String, Box<dyn std::error::Error>> {
		info!(
			"Triggering stale detection for location {} (trigger: {})",
			location_id, trigger
		);

		// Get location's configured index mode
		let location = self.get_location(location_id).await?;

		// Spawn IndexerJob with Stale mode wrapping location's mode
		let config = IndexerJobConfig {
			location_id: Some(location_id),
			path: location.sd_path.clone(),
			mode: IndexMode::Stale(Box::new(location.index_mode)),
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		};

		let job = IndexerJob::new(config);
		let job_id = self.job_manager.dispatch(Box::new(job)).await?;

		// Record run in history
		self.record_detection_run(location_id, &job_id, trigger)
			.await?;

		Ok(job_id)
	}

	/// Check if a location needs stale detection
	pub async fn should_detect_stale(
		&self,
		location_id: Uuid,
	) -> Result<bool, Box<dyn std::error::Error>> {
		// Get watcher state
		let watcher_state = self.get_watcher_state(location_id).await?;

		// Get location settings
		let settings = self.get_location_settings(location_id).await?;

		// If watch was interrupted, always run
		if watcher_state.watch_interrupted {
			return Ok(true);
		}

		// Check offline duration
		if let Some(last_stop) = watcher_state.last_watch_stop {
			let offline_duration = chrono::Utc::now() - last_stop;
			let threshold =
				chrono::Duration::seconds(settings.config.offline_threshold_secs as i64);

			return Ok(offline_duration > threshold);
		}

		// No watcher state, assume we should check
		Ok(true)
	}

	/// Enable stale detection for a location
	pub async fn enable_for_location(
		&self,
		location_id: Uuid,
		settings: StaleDetectorSettings,
	) -> Result<(), Box<dyn std::error::Error>> {
		info!("Enabling stale detection for location {}", location_id);

		let worker = LocationWorker::new(
			location_id,
			settings.config.clone(),
			self.db.clone(),
			self.job_manager.clone(),
			self.library.clone(),
			self.shutdown.clone(),
		);

		let mut workers = self.location_workers.write().await;
		workers.insert(location_id, worker);

		Ok(())
	}

	/// Disable stale detection for a location
	pub async fn disable_for_location(
		&self,
		location_id: &Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		info!("Disabling stale detection for location {}", location_id);

		let mut workers = self.location_workers.write().await;
		workers.remove(location_id);

		Ok(())
	}

	/// Get location by ID
	async fn get_location(
		&self,
		location_id: Uuid,
	) -> Result<Location, Box<dyn std::error::Error>> {
		// Query location from database
		let location_model = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(location_id))
			.one(self.db.as_ref())
			.await?
			.ok_or("Location not found")?;

		// Get entry for path resolution
		let entry = entities::entry::Entity::find_by_id(
			location_model.entry_id.ok_or("Location has no entry")?,
		)
		.one(self.db.as_ref())
		.await?
		.ok_or("Entry not found")?;

		let dir_path = entities::directory_paths::Entity::find_by_id(entry.id)
			.one(self.db.as_ref())
			.await?
			.ok_or("Directory path not found")?;

		// Get device for path construction
		let device = entities::device::Entity::find_by_id(location_model.device_id)
			.one(self.db.as_ref())
			.await?
			.ok_or("Device not found")?;

		let sd_path = crate::domain::addressing::SdPath::Physical {
			device_slug: device.slug.clone(),
			path: dir_path.path.into(),
		};

		Ok(Location::from_db_model(
			&location_model,
			self.library.id(),
			sd_path,
		))
	}

	/// Get stale detection settings for a location
	async fn get_location_settings(
		&self,
		location_id: Uuid,
	) -> Result<StaleDetectorSettings, Box<dyn std::error::Error>> {
		// Query from location_service_settings table
		// For now, return default settings
		// TODO: Implement actual database query
		Ok(StaleDetectorSettings {
			enabled: true,
			config: Default::default(),
		})
	}

	/// Get watcher state for a location
	async fn get_watcher_state(
		&self,
		_location_id: Uuid,
	) -> Result<WatcherState, Box<dyn std::error::Error>> {
		// Query from location_watcher_state table
		// For now, return empty state
		// TODO: Implement actual database query
		Ok(WatcherState {
			last_watch_start: None,
			last_watch_stop: None,
			last_successful_event: None,
			watch_interrupted: false,
		})
	}

	/// Get locations with stale detection enabled
	async fn get_enabled_locations(
		&self,
	) -> Result<Vec<LocationWithSettings>, Box<dyn std::error::Error>> {
		// For now, return empty list
		// TODO: Query location_service_settings table for enabled locations
		Ok(vec![])
	}

	/// Record a stale detection run in history
	async fn record_detection_run(
		&self,
		location_id: Uuid,
		job_id: &str,
		trigger: StaleDetectionTrigger,
	) -> Result<(), Box<dyn std::error::Error>> {
		// Get location's database ID
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(location_id))
			.one(self.db.as_ref())
			.await?
			.ok_or("Location not found")?;

		let run = entities::stale_detection_runs::ActiveModel {
			id: ActiveValue::NotSet,
			location_id: ActiveValue::Set(location.id),
			job_id: ActiveValue::Set(job_id.to_string()),
			triggered_by: ActiveValue::Set(trigger.to_string()),
			started_at: ActiveValue::Set(chrono::Utc::now().into()),
			completed_at: ActiveValue::NotSet,
			status: ActiveValue::Set("running".to_string()),
			directories_pruned: ActiveValue::Set(0),
			directories_scanned: ActiveValue::Set(0),
			changes_detected: ActiveValue::Set(0),
			error_message: ActiveValue::NotSet,
		};

		run.insert(self.db.as_ref()).await?;

		Ok(())
	}
}

struct WatcherState {
	last_watch_start: Option<chrono::DateTime<chrono::Utc>>,
	last_watch_stop: Option<chrono::DateTime<chrono::Utc>>,
	last_successful_event: Option<chrono::DateTime<chrono::Utc>>,
	watch_interrupted: bool,
}

struct LocationWithSettings {
	id: Uuid,
	settings: StaleDetectorSettings,
}
