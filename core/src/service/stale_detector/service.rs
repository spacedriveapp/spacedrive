//! Stale Detection Service Implementation
//!
//! This service periodically checks locations for stale changes and triggers indexing
//! with modified-time pruning to efficiently detect what changed while offline.

use crate::{
	context::CoreContext,
	domain::location::{IndexMode, StaleDetectionTrigger},
	infra::{
		db::entities::{location, location_service_settings, location_watcher_state, stale_detection_runs},
		job::prelude::JobManager,
	},
	ops::indexing::{IndexerJob, IndexerJobConfig, IndexPersistence, IndexScope},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::service::Service;

/// Configuration for stale detection service
#[derive(Debug, Clone)]
pub struct StaleDetectorServiceConfig {
	/// Default check interval in seconds
	pub default_check_interval_secs: u64,
}

impl Default for StaleDetectorServiceConfig {
	fn default() -> Self {
		Self {
			default_check_interval_secs: 3600, // 1 hour
		}
	}
}

/// Per-location worker state
struct LocationWorker {
	location_id: Uuid,
	task: tokio::task::JoinHandle<()>,
}

/// Stale Detection Service
///
/// Periodically checks locations for stale changes and triggers indexing with
/// modified-time pruning. Uses per-location workers with custom intervals based
/// on location configuration.
pub struct StaleDetectionService {
	db: Arc<sea_orm::DatabaseConnection>,
	job_manager: Arc<JobManager>,
	context: Arc<CoreContext>,
	config: StaleDetectorServiceConfig,

	/// Per-location worker tasks
	location_workers: Arc<RwLock<HashMap<Uuid, LocationWorker>>>,

	/// Shutdown signal
	shutdown: Arc<Notify>,
	is_running: AtomicBool,
}

impl StaleDetectionService {
	/// Create a new stale detection service
	pub fn new(
		db: Arc<sea_orm::DatabaseConnection>,
		job_manager: Arc<JobManager>,
		context: Arc<CoreContext>,
		config: StaleDetectorServiceConfig,
	) -> Self {
		Self {
			db,
			job_manager,
			context,
			config,
			location_workers: Arc::new(RwLock::new(HashMap::new())),
			shutdown: Arc::new(Notify::new()),
			is_running: AtomicBool::new(false),
		}
	}

	/// Trigger stale detection for a location
	pub async fn detect_stale(
		&self,
		location_id: Uuid,
		location_path: PathBuf,
		trigger: StaleDetectionTrigger,
	) -> Result<String> {
		info!("Triggering stale detection for location {}", location_id);

		// Get location's configured index mode
		let location = self.get_location(location_id).await?;

		// Spawn IndexerJob with Stale mode (wraps location's mode)
		let config = IndexerJobConfig {
			location_id: Some(location_id),
			path: crate::domain::addressing::SdPath::local(location_path),
			mode: IndexMode::Stale(Box::new(location.index_mode)),
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		};

		let job_id = self
			.job_manager
			.dispatch(IndexerJob::new(config))
			.await
			.context("Failed to dispatch IndexerJob")?;

		// Record run in history
		self.record_detection_run(location_id, &job_id, trigger).await?;

		Ok(job_id)
	}

	/// Check if location needs stale detection
	async fn should_detect_stale(&self, location_id: Uuid) -> Result<bool> {
		// Get watcher state
		let watcher_state = self.get_watcher_state(location_id).await?;

		// Get location settings
		let settings = self.get_location_settings(location_id).await?;

		// Decision logic
		if watcher_state.watch_interrupted {
			return Ok(true);
		}

		if let Some(last_stop) = watcher_state.last_watch_stop {
			let offline_duration = Utc::now() - last_stop;
			let threshold = Duration::seconds(settings.stale_detector.config.offline_threshold_secs as i64);

			if offline_duration > threshold {
				return Ok(true);
			}
		}

		Ok(false)
	}

	/// Get location from database
	async fn get_location(&self, location_id: Uuid) -> Result<crate::domain::location::Location> {
		use crate::infra::db::entities::location as location_entity;

		let location_model = location_entity::Entity::find()
			.filter(location_entity::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		// Convert to domain model (simplified - would need full conversion)
		// For now, return a basic location
		Ok(crate::domain::location::Location::new(
			Uuid::nil(), // library_id - would need to be passed or queried
			location_model.name.unwrap_or_else(|| "Unknown".to_string()),
			crate::domain::addressing::SdPath::local("/"), // Would need to resolve actual path
			match location_model.index_mode.as_str() {
				"none" => IndexMode::None,
				"shallow" => IndexMode::Shallow,
				"content" => IndexMode::Content,
				"deep" => IndexMode::Deep,
				_ => IndexMode::Deep,
			},
		))
	}

	/// Get watcher state for a location
	async fn get_watcher_state(
		&self,
		location_id: Uuid,
	) -> Result<WatcherState> {
		// Get location's database ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let watcher_state_model = location_watcher_state::Entity::find_by_id(location_model.id)
			.one(&*self.db)
			.await
			.context("Failed to query watcher state")?;

		Ok(watcher_state_model.map(|m| WatcherState {
			last_watch_start: m.last_watch_start,
			last_watch_stop: m.last_watch_stop,
			last_successful_event: m.last_successful_event,
			watch_interrupted: m.watch_interrupted,
		}).unwrap_or_default())
	}

	/// Get location service settings
	async fn get_location_settings(
		&self,
		location_id: Uuid,
	) -> Result<crate::domain::location::StaleDetectorSettings> {
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
			let config: crate::domain::location::StaleDetectorConfig = model
				.stale_detector_config
				.as_ref()
				.and_then(|json| serde_json::from_str(json).ok())
				.unwrap_or_default();

			Ok(crate::domain::location::StaleDetectorSettings {
				enabled: model.stale_detector_enabled,
				config,
			})
		} else {
			// Return defaults if not configured
			Ok(crate::domain::location::StaleDetectorSettings {
				enabled: true,
				config: Default::default(),
			})
		}
	}

	/// Record a stale detection run in the database
	async fn record_detection_run(
		&self,
		location_id: Uuid,
		job_id: &str,
		trigger: StaleDetectionTrigger,
	) -> Result<()> {
		// Get location's database ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(&*self.db)
			.await
			.context("Failed to query location")?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let run = stale_detection_runs::ActiveModel {
			location_id: Set(location_model.id),
			job_id: Set(job_id.to_string()),
			triggered_by: Set(trigger.to_string()),
			started_at: Set(Utc::now()),
			completed_at: Set(None),
			status: Set("running".to_string()),
			directories_pruned: Set(0),
			directories_scanned: Set(0),
			changes_detected: Set(0),
			error_message: Set(None),
			..Default::default()
		};

		stale_detection_runs::Entity::insert(run)
			.exec(&*self.db)
			.await
			.context("Failed to insert stale detection run")?;

		Ok(())
	}

	/// Load locations with stale detection enabled and start workers
	async fn load_and_start_workers(&self) -> Result<()> {
		// Query all locations with stale detection enabled
		let locations = location::Entity::find()
			.all(&*self.db)
			.await
			.context("Failed to query locations")?;

		for location_model in locations {
			let location_id = location_model.uuid;
			let settings = self.get_location_settings(location_id).await?;

			if !settings.enabled {
				continue;
			}

			// Start worker for this location
			self.start_location_worker(location_id, settings.config.check_interval_secs)
				.await?;
		}

		Ok(())
	}

	/// Start a worker task for a location
	async fn start_location_worker(
		&self,
		location_id: Uuid,
		check_interval_secs: u64,
	) -> Result<()> {
		let db = Arc::clone(&self.db);
		let job_manager = Arc::clone(&self.job_manager);
		let context = Arc::clone(&self.context);
		let shutdown = Arc::clone(&self.shutdown);
		let location_workers = Arc::clone(&self.location_workers);

		let task = tokio::spawn(async move {
			let mut interval = interval(TokioDuration::from_secs(check_interval_secs));
			interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				tokio::select! {
					_ = interval.tick() => {
						// Check if stale detection needed
						let service = StaleDetectionService {
							db: Arc::clone(&db),
							job_manager: Arc::clone(&job_manager),
							context: Arc::clone(&context),
							config: Default::default(),
							location_workers: Arc::clone(&location_workers),
							shutdown: Arc::clone(&shutdown),
							is_running: AtomicBool::new(true),
						};

						match service.should_detect_stale(location_id).await {
							Ok(true) => {
								info!("Stale detection needed for location {}", location_id);
								// Get location path (simplified - would need proper resolution)
								if let Err(e) = service.detect_stale(
									location_id,
									PathBuf::from("/"), // Would need actual path resolution
									StaleDetectionTrigger::Periodic,
								).await {
									warn!("Failed to trigger stale detection: {}", e);
								}
							}
							Ok(false) => {
								debug!("No stale detection needed for location {}", location_id);
							}
							Err(e) => {
								warn!("Error checking stale detection for location {}: {}", location_id, e);
							}
						}
					}
					_ = shutdown.notified() => {
						debug!("Location worker {} shutting down", location_id);
						break;
					}
				}
			}
		});

		let mut workers = location_workers.write().await;
		workers.insert(location_id, LocationWorker { location_id, task });

		Ok(())
	}
}

#[derive(Default)]
struct WatcherState {
	last_watch_start: Option<DateTime<Utc>>,
	last_watch_stop: Option<DateTime<Utc>>,
	last_successful_event: Option<DateTime<Utc>>,
	watch_interrupted: bool,
}

#[async_trait::async_trait]
impl Service for StaleDetectionService {
	async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("Stale detection service already running");
			return Ok(());
		}

		info!("Starting stale detection service");
		self.load_and_start_workers().await?;
		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.swap(false, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping stale detection service");

		// Signal shutdown
		self.shutdown.notify_waiters();

		// Wait for all workers to finish
		let workers = self.location_workers.read().await;
		for (location_id, worker) in workers.iter() {
			debug!("Waiting for location worker {} to finish", location_id);
			let _ = worker.task.await;
		}

		Ok(())
	}

	fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	fn name(&self) -> &'static str {
		"StaleDetectionService"
	}
}
