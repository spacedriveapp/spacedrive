//! Platform-specific event handling

use crate::infra::event::Event;
use crate::service::watcher::{WatchedLocation, WatcherEvent};
use anyhow::Result;
use std::collections::HashMap;
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
	) -> Result<Vec<Event>> {
		self.inner.process_event(event, watched_locations).await
	}

	/// Periodic tick for cleanup and debouncing
	pub async fn tick(&self) -> Result<()> {
		self.inner.tick().await
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
	) -> Result<Vec<Event>> {
		// Basic event processing without platform-specific optimizations
		if !event.should_process() {
			return Ok(vec![]);
		}

		let locations = watched_locations.read().await;
		let mut events = Vec::new();

		for location in locations.values() {
			if !location.enabled {
				continue;
			}

			for path in &event.paths {
				if path.starts_with(&location.path) {
					// Generate a placeholder entry ID for now
					// In a real implementation, this would look up or create an entry
					let entry_id = Uuid::new_v4();

					if let Some(core_event) = event.to_raw_event(location.library_id) {
						events.push(core_event);
					}
					break;
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
