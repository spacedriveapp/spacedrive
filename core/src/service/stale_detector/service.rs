//! Stale Detection Service Implementation
//!
//! Manages per-location stale detection workers that periodically check for
//! filesystem changes that occurred while Spacedrive was offline.

use crate::{
	context::CoreContext,
	domain::{
		addressing::SdPath,
		location::{IndexMode, StaleDetectionTrigger, StaleDetectorConfig},
	},
	infra::db::entities::{
		location, location_service_settings, location_watcher_state, stale_detection_runs,
	},
	infra::job::handle::JobHandle,
	library::Library,
	ops::indexing::job::{IndexPersistence, IndexScope, IndexerJob, IndexerJobConfig},
	service::Service,
};
use anyhow::Result;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use std::{
	collections::HashMap,
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};
use tokio::sync::{Mutex, Notify, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for the stale detection service
#[derive(Debug, Clone)]
pub struct StaleDetectionServiceConfig {
	/// Default check interval for locations without custom config (seconds)
	pub default_check_interval_secs: u64,
	/// Default offline threshold before triggering detection (seconds)
	pub default_offline_threshold_secs: u64,
	/// Maximum number of concurrent stale detection jobs
	pub max_concurrent_jobs: usize,
}

impl Default for StaleDetectionServiceConfig {
	fn default() -> Self {
		Self {
			default_check_interval_secs: 3600,      // 1 hour
			default_offline_threshold_secs: 1800,  // 30 minutes
			max_concurrent_jobs: 4,
		}
	}
}

/// Information about a location being monitored for staleness
#[derive(Debug, Clone)]
struct LocationInfo {
	id: Uuid,
	db_id: i32,
	path: PathBuf,
	index_mode: IndexMode,
	config: StaleDetectorConfig,
}

/// Per-location worker task handle
struct LocationWorker {
	location_id: Uuid,
	handle: tokio::task::JoinHandle<()>,
	shutdown: Arc<Notify>,
}

/// Stale Detection Service
///
/// Monitors locations for changes that occurred while offline by spawning
/// IndexerJob instances with `IndexMode::Stale` which uses mtime pruning.
pub struct StaleDetectionService {
	context: Arc<CoreContext>,
	config: StaleDetectionServiceConfig,
	library: Arc<Library>,

	/// Per-location worker tasks
	workers: Arc<RwLock<HashMap<Uuid, LocationWorker>>>,

	/// Currently running stale detection jobs
	running_jobs: Arc<Mutex<HashMap<Uuid, JobHandle>>>,

	/// Service running state
	is_running: AtomicBool,

	/// Global shutdown signal
	shutdown: Arc<Notify>,
}

impl StaleDetectionService {
	/// Create a new stale detection service for a library
	pub fn new(
		context: Arc<CoreContext>,
		library: Arc<Library>,
		config: StaleDetectionServiceConfig,
	) -> Self {
		Self {
			context,
			library,
			config,
			workers: Arc::new(RwLock::new(HashMap::new())),
			running_jobs: Arc::new(Mutex::new(HashMap::new())),
			is_running: AtomicBool::new(false),
			shutdown: Arc::new(Notify::new()),
		}
	}

	/// Create with default configuration
	pub fn with_defaults(context: Arc<CoreContext>, library: Arc<Library>) -> Self {
		Self::new(context, library, StaleDetectionServiceConfig::default())
	}

	/// Trigger stale detection for a specific location
	///
	/// Spawns an IndexerJob with `IndexMode::Stale` which wraps the location's
	/// configured index mode. The discovery phase will use mtime pruning to
	/// skip unchanged directory branches.
	pub async fn detect_stale(
		&self,
		location_id: Uuid,
		location_path: PathBuf,
		location_index_mode: IndexMode,
		trigger: StaleDetectionTrigger,
	) -> Result<String> {
		info!(
			location_id = %location_id,
			trigger = ?trigger,
			"Triggering stale detection"
		);

		// Create IndexerJob config with Stale mode wrapping the location's mode
		let config = IndexerJobConfig {
			location_id: Some(location_id),
			path: SdPath::local(location_path.clone()),
			mode: IndexMode::Stale(Box::new(location_index_mode.clone())),
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		};

		let job = IndexerJob::new(config);
		let handle = self
			.library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to dispatch stale detection job: {}", e))?;

		let job_id = handle.id().to_string();

		// Record the run in history
		if let Err(e) = self
			.record_detection_run(location_id, &job_id, trigger.clone())
			.await
		{
			warn!(
				location_id = %location_id,
				job_id = %job_id,
				error = %e,
				"Failed to record stale detection run"
			);
		}

		// Track the running job
		{
			let mut running = self.running_jobs.lock().await;
			running.insert(location_id, handle);
		}

		info!(
			location_id = %location_id,
			job_id = %job_id,
			"Stale detection job dispatched"
		);

		Ok(job_id)
	}

	/// Manually trigger stale detection for a location
	pub async fn trigger_manual(&self, location_id: Uuid) -> Result<String> {
		let location = self.get_location_info(location_id).await?;
		self.detect_stale(
			location_id,
			location.path,
			location.index_mode,
			StaleDetectionTrigger::Manual,
		)
		.await
	}

	/// Check if stale detection should run for a location
	async fn should_detect_stale(&self, location_id: Uuid) -> Result<bool> {
		let db = self.library.db().conn();

		// Get watcher state
		let watcher_state = location_watcher_state::Entity::find()
			.filter(
				location_watcher_state::Column::LocationId.eq(self.get_location_db_id(location_id).await?),
			)
			.one(db)
			.await?;

		let Some(state) = watcher_state else {
			// No watcher state means first run - full index needed, not stale detection
			return Ok(false);
		};

		// Always detect if watch was interrupted (crash recovery)
		if state.watch_interrupted {
			debug!(
				location_id = %location_id,
				"Watcher was interrupted, stale detection needed"
			);
			return Ok(true);
		}

		// Get settings to check threshold
		let settings = self.get_location_settings(location_id).await?;
		let threshold_secs = settings
			.as_ref()
			.and_then(|s| {
				s.stale_detector_config.as_ref().and_then(|c| {
					serde_json::from_str::<StaleDetectorConfig>(c).ok()
				})
			})
			.map(|c| c.offline_threshold_secs)
			.unwrap_or(self.config.default_offline_threshold_secs);

		// Check if offline duration exceeds threshold
		if state.should_detect_stale(threshold_secs as i64) {
			debug!(
				location_id = %location_id,
				threshold_secs = threshold_secs,
				"Offline duration exceeds threshold, stale detection needed"
			);
			return Ok(true);
		}

		Ok(false)
	}

	/// Check all locations for staleness on startup
	pub async fn check_stale_on_startup(&self) -> Result<usize> {
		let locations = self.get_enabled_locations().await?;
		let mut triggered = 0;

		for location in locations {
			if self.should_detect_stale(location.id).await? {
				info!(
					location_id = %location.id,
					"Running startup stale detection"
				);
				match self
					.detect_stale(
						location.id,
						location.path,
						location.index_mode,
						StaleDetectionTrigger::Startup,
					)
					.await
				{
					Ok(job_id) => {
						info!(
							location_id = %location.id,
							job_id = %job_id,
							"Startup stale detection triggered"
						);
						triggered += 1;
					}
					Err(e) => {
						warn!(
							location_id = %location.id,
							error = %e,
							"Failed to trigger startup stale detection"
						);
					}
				}
			}
		}

		Ok(triggered)
	}

	/// Enable stale detection for a location
	pub async fn enable_for_location(
		&self,
		location_id: Uuid,
		config: Option<StaleDetectorConfig>,
	) -> Result<()> {
		let location = self.get_location_info(location_id).await?;
		let cfg = config.unwrap_or_default();

		// Start worker for this location
		self.start_location_worker(location, cfg).await?;

		info!(
			location_id = %location_id,
			"Stale detection enabled"
		);
		Ok(())
	}

	/// Disable stale detection for a location
	pub async fn disable_for_location(&self, location_id: Uuid) -> Result<()> {
		let mut workers = self.workers.write().await;
		if let Some(worker) = workers.remove(&location_id) {
			worker.shutdown.notify_one();
			worker.handle.abort();
			info!(
				location_id = %location_id,
				"Stale detection disabled"
			);
		}
		Ok(())
	}

	/// Start a worker task for a location
	async fn start_location_worker(
		&self,
		location: LocationInfo,
		config: StaleDetectorConfig,
	) -> Result<()> {
		let shutdown = Arc::new(Notify::new());
		let location_id = location.id;
		let interval_secs = config.check_interval_secs;

		let service = self.clone_for_worker();
		let worker_shutdown = shutdown.clone();

		let handle = tokio::spawn(async move {
			Self::location_worker_loop(service, location, config, worker_shutdown).await
		});

		let mut workers = self.workers.write().await;
		workers.insert(
			location_id,
			LocationWorker {
				location_id,
				handle,
				shutdown,
			},
		);

		debug!(
			location_id = %location_id,
			interval_secs = interval_secs,
			"Location worker started"
		);
		Ok(())
	}

	/// Worker loop for periodic stale detection
	async fn location_worker_loop(
		service: Arc<StaleDetectionService>,
		location: LocationInfo,
		config: StaleDetectorConfig,
		shutdown: Arc<Notify>,
	) {
		let interval = Duration::from_secs(config.check_interval_secs);

		loop {
			tokio::select! {
				_ = shutdown.notified() => {
					debug!(
						location_id = %location.id,
						"Location worker shutting down"
					);
					break;
				}
				_ = tokio::time::sleep(interval) => {
					// Periodic check
					if let Ok(true) = service.should_detect_stale(location.id).await {
						if let Err(e) = service.detect_stale(
							location.id,
							location.path.clone(),
							location.index_mode.clone(),
							StaleDetectionTrigger::Periodic,
						).await {
							warn!(
								location_id = %location.id,
								error = %e,
								"Periodic stale detection failed"
							);
						}
					}
				}
			}
		}
	}

	/// Clone self for passing to worker tasks
	fn clone_for_worker(&self) -> Arc<StaleDetectionService> {
		Arc::new(StaleDetectionService {
			context: self.context.clone(),
			library: self.library.clone(),
			config: self.config.clone(),
			workers: self.workers.clone(),
			running_jobs: self.running_jobs.clone(),
			is_running: AtomicBool::new(self.is_running.load(Ordering::SeqCst)),
			shutdown: self.shutdown.clone(),
		})
	}

	/// Record a stale detection run in the history table
	async fn record_detection_run(
		&self,
		location_id: Uuid,
		job_id: &str,
		trigger: StaleDetectionTrigger,
	) -> Result<()> {
		let db = self.library.db().conn();
		let db_location_id = self.get_location_db_id(location_id).await?;

		let run = stale_detection_runs::ActiveModel {
			location_id: Set(db_location_id),
			job_id: Set(job_id.to_string()),
			triggered_by: Set(trigger.to_string()),
			started_at: Set(chrono::Utc::now().into()),
			status: Set("running".to_string()),
			..Default::default()
		};

		run.insert(db).await?;
		Ok(())
	}

	/// Update a stale detection run with completion stats
	pub async fn update_detection_run(
		&self,
		job_id: &str,
		directories_pruned: i32,
		directories_scanned: i32,
		changes_detected: i32,
		status: &str,
		error_message: Option<String>,
	) -> Result<()> {
		let db = self.library.db().conn();

		// Find the run by job_id
		let run = stale_detection_runs::Entity::find()
			.filter(stale_detection_runs::Column::JobId.eq(job_id))
			.one(db)
			.await?;

		if let Some(run) = run {
			let mut active: stale_detection_runs::ActiveModel = run.into();
			active.status = Set(status.to_string());
			active.completed_at = Set(Some(chrono::Utc::now().into()));
			active.directories_pruned = Set(directories_pruned);
			active.directories_scanned = Set(directories_scanned);
			active.changes_detected = Set(changes_detected);
			if let Some(msg) = error_message {
				active.error_message = Set(Some(msg));
			}
			active.update(db).await?;
		}

		Ok(())
	}

	/// Get locations with stale detection enabled
	async fn get_enabled_locations(&self) -> Result<Vec<LocationInfo>> {
		let db = self.library.db().conn();
		let current_device_uuid = crate::device::get_current_device_id();

		// Find current device in this library's database
		use crate::infra::db::entities::device;
		let current_device = device::Entity::find()
			.filter(device::Column::Uuid.eq(current_device_uuid))
			.one(db)
			.await?;

		let Some(current_device) = current_device else {
			debug!("Current device not found in library");
			return Ok(vec![]);
		};

		// Get locations owned by this device
		let locations = location::Entity::find()
			.filter(location::Column::DeviceId.eq(current_device.id))
			.all(db)
			.await?;

		let mut result = vec![];
		for loc in locations {
			// Check if stale detection is enabled for this location
			let settings = location_service_settings::Entity::find()
				.filter(location_service_settings::Column::LocationId.eq(loc.id))
				.one(db)
				.await?;

			let enabled = settings
				.as_ref()
				.map(|s| s.stale_detector_enabled)
				.unwrap_or(true); // Default to enabled

			if !enabled {
				continue;
			}

			// Get the filesystem path
			let Some(entry_id) = loc.entry_id else {
				continue;
			};

			let path = match crate::ops::indexing::PathResolver::get_full_path(db, entry_id).await {
				Ok(p) => p,
				Err(e) => {
					warn!(
						location_id = %loc.uuid,
						error = %e,
						"Failed to resolve location path"
					);
					continue;
				}
			};

			// Parse config
			let config = settings
				.as_ref()
				.and_then(|s| {
					s.stale_detector_config
						.as_ref()
						.and_then(|c| serde_json::from_str::<StaleDetectorConfig>(c).ok())
				})
				.unwrap_or_default();

			// Parse index mode
			let index_mode = IndexMode::from_str(&loc.index_mode);

			result.push(LocationInfo {
				id: loc.uuid,
				db_id: loc.id,
				path,
				index_mode,
				config,
			});
		}

		Ok(result)
	}

	/// Get location info by UUID
	async fn get_location_info(&self, location_id: Uuid) -> Result<LocationInfo> {
		let db = self.library.db().conn();

		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let entry_id = loc
			.entry_id
			.ok_or_else(|| anyhow::anyhow!("Location has no entry_id: {}", location_id))?;

		let path = crate::ops::indexing::PathResolver::get_full_path(db, entry_id).await?;

		let settings = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(loc.id))
			.one(db)
			.await?;

		let config = settings
			.and_then(|s| {
				s.stale_detector_config
					.and_then(|c| serde_json::from_str::<StaleDetectorConfig>(&c).ok())
			})
			.unwrap_or_default();

		let index_mode = IndexMode::from_str(&loc.index_mode);

		Ok(LocationInfo {
			id: loc.uuid,
			db_id: loc.id,
			path,
			index_mode,
			config,
		})
	}

	/// Get location database ID from UUID
	async fn get_location_db_id(&self, location_id: Uuid) -> Result<i32> {
		let db = self.library.db().conn();
		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;
		Ok(loc.id)
	}

	/// Get location service settings
	async fn get_location_settings(
		&self,
		location_id: Uuid,
	) -> Result<Option<location_service_settings::Model>> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id).await?;

		let settings = location_service_settings::Entity::find()
			.filter(location_service_settings::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		Ok(settings)
	}

	/// Stop all location workers
	async fn stop_all_workers(&self) {
		let mut workers = self.workers.write().await;
		for (location_id, worker) in workers.drain() {
			debug!(location_id = %location_id, "Stopping location worker");
			worker.shutdown.notify_one();
			worker.handle.abort();
		}
	}
}

#[async_trait::async_trait]
impl Service for StaleDetectionService {
	async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("Stale detection service is already running");
			return Ok(());
		}

		info!("Starting stale detection service");

		// Load locations with stale detection enabled
		let locations = self.get_enabled_locations().await?;
		info!(
			"Found {} locations with stale detection enabled",
			locations.len()
		);

		// Start workers for each location
		for location in locations {
			let config = location.config.clone();
			if let Err(e) = self.start_location_worker(location, config).await {
				warn!(error = %e, "Failed to start location worker");
			}
		}

		// Check for stale on startup
		match self.check_stale_on_startup().await {
			Ok(count) => {
				if count > 0 {
					info!("Triggered {} startup stale detection jobs", count);
				}
			}
			Err(e) => {
				warn!(error = %e, "Failed to check stale on startup");
			}
		}

		info!("Stale detection service started");
		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.swap(false, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping stale detection service");

		// Signal shutdown to all workers
		self.shutdown.notify_waiters();

		// Stop all workers
		self.stop_all_workers().await;

		info!("Stale detection service stopped");
		Ok(())
	}

	fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	fn name(&self) -> &'static str {
		"stale_detector"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_default() {
		let config = StaleDetectionServiceConfig::default();
		assert_eq!(config.default_check_interval_secs, 3600);
		assert_eq!(config.default_offline_threshold_secs, 1800);
		assert_eq!(config.max_concurrent_jobs, 4);
	}
}
