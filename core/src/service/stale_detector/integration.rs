//! Library and Location Integration Helpers
//!
//! Provides integration hooks for stale detection service with library lifecycle
//! and location management. These helpers are called from library initialization
//! and location operations.

use crate::{
	context::CoreContext,
	library::Library,
	service::{
		coordinator::ServiceCoordinator,
		stale_detector::{StaleDetectionService, StaleDetectionServiceConfig},
		watcher::{FsWatcherService, WatcherStateTracker},
		Service,
	},
};
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Initialize stale detection and related services for a library
///
/// This should be called during library initialization, after the database
/// is available but before the library is fully operational.
pub async fn init_stale_detection_services(
	library: Arc<Library>,
	context: Arc<CoreContext>,
	fs_watcher: Arc<FsWatcherService>,
) -> Result<(Arc<StaleDetectionService>, Arc<ServiceCoordinator>)> {
	info!(
		library_id = %library.id(),
		"Initializing stale detection services"
	);

	// Create watcher state tracker
	let watcher_tracker = WatcherStateTracker::new(library.clone());

	// Mark any interrupted watches from previous crash
	match watcher_tracker.mark_interrupted_on_startup().await {
		Ok(count) if count > 0 => {
			info!(
				count = count,
				"Detected {} interrupted watches from previous session",
				count
			);
		}
		Ok(_) => {}
		Err(e) => {
			warn!(error = %e, "Failed to check for interrupted watches");
		}
	}

	// Create stale detection service
	let stale_detector = Arc::new(StaleDetectionService::with_defaults(
		context.clone(),
		library.clone(),
	));

	// Create service coordinator
	let mut coordinator = ServiceCoordinator::new(library.clone(), context.clone(), fs_watcher);

	// Connect stale detector to coordinator
	coordinator.set_stale_detector(stale_detector.clone());

	let coordinator = Arc::new(coordinator);

	// Start stale detection service
	stale_detector.start().await?;

	info!(
		library_id = %library.id(),
		"Stale detection services initialized"
	);

	Ok((stale_detector, coordinator))
}

/// Called when a new location is added to initialize service settings
pub async fn on_location_added(
	library: Arc<Library>,
	location_id: Uuid,
	coordinator: Option<&ServiceCoordinator>,
) -> Result<()> {
	info!(
		library_id = %library.id(),
		location_id = %location_id,
		"Initializing services for new location"
	);

	// Initialize watcher state
	let watcher_tracker = WatcherStateTracker::new(library.clone());
	if let Err(e) = watcher_tracker.initialize_for_location(location_id).await {
		warn!(
			location_id = %location_id,
			error = %e,
			"Failed to initialize watcher state"
		);
	}

	// Initialize service settings
	if let Some(coordinator) = coordinator {
		if let Err(e) = coordinator.initialize_default_settings(location_id).await {
			warn!(
				location_id = %location_id,
				error = %e,
				"Failed to initialize service settings"
			);
		}
	}

	Ok(())
}

/// Called when a location is removed to clean up services
pub async fn on_location_removed(
	library: Arc<Library>,
	location_id: Uuid,
	coordinator: Option<&ServiceCoordinator>,
) -> Result<()> {
	info!(
		library_id = %library.id(),
		location_id = %location_id,
		"Cleaning up services for removed location"
	);

	// Stop location services
	if let Some(coordinator) = coordinator {
		if let Err(e) = coordinator.stop_location_services(location_id).await {
			warn!(
				location_id = %location_id,
				error = %e,
				"Failed to stop location services"
			);
		}
	}

	// Delete watcher state
	let watcher_tracker = WatcherStateTracker::new(library.clone());
	if let Err(e) = watcher_tracker.delete_for_location(location_id).await {
		warn!(
			location_id = %location_id,
			error = %e,
			"Failed to delete watcher state"
		);
	}

	Ok(())
}

/// Shutdown stale detection services for a library
pub async fn shutdown_stale_detection_services(
	stale_detector: &StaleDetectionService,
) -> Result<()> {
	info!("Shutting down stale detection services");
	stale_detector.stop().await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	// Integration tests require database setup
	// See core/tests/stale_detection_integration_test.rs
}
