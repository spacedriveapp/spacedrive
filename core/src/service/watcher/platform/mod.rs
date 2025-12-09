//! Platform-specific event handling

use crate::infra::event::Event;
use crate::service::watcher::{EphemeralWatch, WatchedLocation, WatcherEvent};
use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Platform-specific event handler
pub struct PlatformHandler {
	#[cfg(target_os = "linux")]
	pub inner: linux::LinuxHandler,
	#[cfg(target_os = "macos")]
	pub inner: macos::MacOSHandler,
	#[cfg(target_os = "windows")]
	pub inner: windows::WindowsHandler,
	#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
	pub inner: DefaultHandler,
}

impl PlatformHandler {
	/// Create a new platform handler
	pub fn new() -> Self {
		Self {
			#[cfg(target_os = "linux")]
			inner: linux::LinuxHandler::new(),
			#[cfg(target_os = "macos")]
			inner: macos::MacOSHandler::new(),
			#[cfg(target_os = "windows")]
			inner: windows::WindowsHandler::new(),
			#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
			inner: DefaultHandler::new(),
		}
	}

	/// Process a file system event
	pub async fn process_event(
		&self,
		event: WatcherEvent,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>> {
		self.inner
			.process_event(event, watched_locations, ephemeral_watches)
			.await
	}

	/// Periodic tick for cleanup and debouncing
	pub async fn tick(&self) -> Result<()> {
		self.inner.tick().await
	}

	/// Register a database connection for a location (needed for rename detection)
	#[cfg(target_os = "macos")]
	pub async fn register_location_db(&self, location_id: Uuid, db: DatabaseConnection) {
		self.inner.register_location_db(location_id, db).await;
	}

	#[cfg(not(target_os = "macos"))]
	pub async fn register_location_db(&self, _location_id: Uuid, _db: DatabaseConnection) {
		// Not needed on other platforms yet
	}

	/// Unregister a database connection for a location
	#[cfg(target_os = "macos")]
	pub async fn unregister_location_db(&self, location_id: Uuid) {
		self.inner.unregister_location_db(location_id).await;
	}

	#[cfg(not(target_os = "macos"))]
	pub async fn unregister_location_db(&self, _location_id: Uuid) {
		// Not needed on other platforms yet
	}
}

/// Trait for platform-specific handlers
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
	/// Process a file system event and return core events
	async fn process_event(
		&self,
		event: WatcherEvent,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>>;

	/// Periodic cleanup and processing
	async fn tick(&self) -> Result<()>;
}

/// Default handler for unsupported platforms
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub struct DefaultHandler;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl DefaultHandler {
	pub fn new() -> Self {
		Self
	}
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[async_trait::async_trait]
impl EventHandler for DefaultHandler {
	async fn process_event(
		&self,
		event: WatcherEvent,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>> {
		// Basic event processing without platform-specific optimizations
		if !event.should_process() {
			return Ok(vec![]);
		}

		let locations = watched_locations.read().await;
		let ephemeral = ephemeral_watches.read().await;
		let mut events = Vec::new();

		for path in &event.paths {
			// Check if this matches a location
			let mut matched_location = false;
			for location in locations.values() {
				if location.enabled && path.starts_with(&location.path) {
					if let Some(core_event) = event.to_raw_event(location.library_id) {
						events.push(core_event);
					}
					matched_location = true;
					break;
				}
			}

			// If not matched by location, check ephemeral watches
			// For ephemeral, we use a dummy library_id since it's not library-specific
			if !matched_location {
				if let Some(parent) = path.parent() {
					if ephemeral.contains_key(parent) {
						// Use a zero UUID for ephemeral events (they're not library-specific)
						if let Some(core_event) = event.to_raw_event(Uuid::nil()) {
							events.push(core_event);
						}
						break;
					}
				}
			}
		}

		Ok(events)
	}

	async fn tick(&self) -> Result<()> {
		// Nothing to do for default handler
		Ok(())
	}
}
