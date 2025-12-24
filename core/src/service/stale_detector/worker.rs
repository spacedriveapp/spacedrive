//! Per-location worker for stale detection

use crate::{
	domain::location::{IndexMode, StaleDetectionTrigger, StaleDetectorConfig},
	infra::job::manager::JobManager,
	library::Library,
	ops::indexing::{IndexerJob, IndexerJobConfig, IndexPersistence, IndexScope},
};
use sea_orm::DatabaseConnection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Per-location worker that periodically checks for staleness
pub struct LocationWorker {
	location_id: Uuid,
	config: StaleDetectorConfig,
	db: Arc<DatabaseConnection>,
	job_manager: Arc<JobManager>,
	library: Arc<Library>,
	shutdown: Arc<Notify>,
}

impl LocationWorker {
	pub fn new(
		location_id: Uuid,
		config: StaleDetectorConfig,
		db: Arc<DatabaseConnection>,
		job_manager: Arc<JobManager>,
		library: Arc<Library>,
		shutdown: Arc<Notify>,
	) -> Self {
		let worker = Self {
			location_id,
			config: config.clone(),
			db,
			job_manager,
			library,
			shutdown,
		};

		// Spawn background task
		let worker_clone = worker.clone();
		tokio::spawn(async move {
			worker_clone.run().await;
		});

		worker
	}

	async fn run(&self) {
		info!(
			"Starting stale detection worker for location {}",
			self.location_id
		);

		let check_interval = Duration::from_secs(self.config.check_interval_secs);

		loop {
			tokio::select! {
				_ = self.shutdown.notified() => {
					info!("Stale detection worker for location {} shutting down", self.location_id);
					break;
				}
				_ = tokio::time::sleep(check_interval) => {
					debug!("Checking staleness for location {}", self.location_id);

					if let Err(e) = self.check_and_trigger().await {
						error!(
							"Error checking staleness for location {}: {}",
							self.location_id, e
						);
					}
				}
			}
		}
	}

	async fn check_and_trigger(&self) -> Result<(), Box<dyn std::error::Error>> {
		// Check if stale detection is needed
		if !self.should_trigger().await? {
			return Ok(());
		}

		info!(
			"Triggering stale detection for location {} (periodic check)",
			self.location_id
		);

		// Get location info
		let location = self.get_location().await?;

		// Spawn IndexerJob with Stale mode
		let config = IndexerJobConfig {
			location_id: Some(self.location_id),
			path: location.sd_path.clone(),
			mode: IndexMode::Stale(Box::new(location.index_mode)),
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		};

		let job = IndexerJob::new(config);
		let _job_id = self.job_manager.dispatch(Box::new(job)).await?;

		Ok(())
	}

	async fn should_trigger(&self) -> Result<bool, Box<dyn std::error::Error>> {
		// Get watcher state
		let watcher_state = self.get_watcher_state().await?;

		// If watch was interrupted, always run
		if watcher_state.watch_interrupted {
			return Ok(true);
		}

		// Check offline duration
		if let Some(last_stop) = watcher_state.last_watch_stop {
			let offline_duration = chrono::Utc::now() - last_stop;
			let threshold =
				chrono::Duration::seconds(self.config.offline_threshold_secs as i64);

			return Ok(offline_duration > threshold);
		}

		// No watcher state, be conservative
		Ok(false)
	}

	async fn get_location(
		&self,
	) -> Result<crate::domain::location::Location, Box<dyn std::error::Error>> {
		use crate::domain::resource::Identifiable;
		let locations =
			crate::domain::location::Location::from_ids(self.db.as_ref(), &[self.location_id])
				.await?;
		locations.into_iter().next().ok_or("Location not found".into())
	}

	async fn get_watcher_state(&self) -> Result<WatcherState, Box<dyn std::error::Error>> {
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
}

impl Clone for LocationWorker {
	fn clone(&self) -> Self {
		Self {
			location_id: self.location_id,
			config: self.config.clone(),
			db: Arc::clone(&self.db),
			job_manager: Arc::clone(&self.job_manager),
			library: Arc::clone(&self.library),
			shutdown: Arc::clone(&self.shutdown),
		}
	}
}

struct WatcherState {
	last_watch_start: Option<chrono::DateTime<chrono::Utc>>,
	last_watch_stop: Option<chrono::DateTime<chrono::Utc>>,
	last_successful_event: Option<chrono::DateTime<chrono::Utc>>,
	watch_interrupted: bool,
}
