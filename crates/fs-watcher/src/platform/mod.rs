//! Platform-specific event handlers
//!
//! Each platform has different filesystem event semantics. Platform handlers
//! translate raw OS events into normalized `FsEvent` types.
//!
//! Key responsibilities:
//! - Rename detection (especially on macOS where renames come as separate create/delete events)
//! - Event buffering and debouncing
//! - Platform-specific quirk handling
//!
//! Platform handlers are storage-agnostic - they return raw events without
//! any knowledge of locations, libraries, or databases.

use crate::event::{FsEvent, RawNotifyEvent};
use crate::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::LinuxHandler;
#[cfg(target_os = "macos")]
pub use macos::MacOsHandler;
#[cfg(target_os = "windows")]
pub use windows::WindowsHandler;

/// Trait for platform-specific event processing
///
/// Platform handlers receive raw notify events and return normalized `FsEvent` values.
/// They may buffer events internally for rename detection or debouncing.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
	/// Process a raw event and return normalized events
	///
	/// Platform handlers may:
	/// - Buffer events internally (e.g., for rename detection)
	/// - Return empty vec if event is still being processed
	/// - Return multiple events if buffered events are ready
	async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>>;

	/// Periodic tick for evicting buffered events
	///
	/// Returns events that have been buffered and are now ready to emit.
	/// For example, files that didn't match rename patterns after a timeout.
	async fn tick(&self) -> Result<Vec<FsEvent>>;

	/// Reset internal state (e.g., clear buffers)
	async fn reset(&self);
}

/// Platform handler wrapper that selects the appropriate implementation
pub struct PlatformHandler {
	#[cfg(target_os = "macos")]
	inner: MacOsHandler,
	#[cfg(target_os = "linux")]
	inner: LinuxHandler,
	#[cfg(target_os = "windows")]
	inner: WindowsHandler,
	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	inner: DefaultHandler,
}

impl PlatformHandler {
	/// Create a new platform handler for the current platform
	pub fn new() -> Self {
		Self {
			#[cfg(target_os = "macos")]
			inner: MacOsHandler::new(),
			#[cfg(target_os = "linux")]
			inner: LinuxHandler::new(),
			#[cfg(target_os = "windows")]
			inner: WindowsHandler::new(),
			#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
			inner: DefaultHandler::new(),
		}
	}

	/// Process a raw event
	pub async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
		self.inner.process(event).await
	}

	/// Periodic tick for buffered event eviction
	pub async fn tick(&self) -> Result<Vec<FsEvent>> {
		self.inner.tick().await
	}

	/// Reset internal state
	pub async fn reset(&self) {
		self.inner.reset().await
	}
}

impl Default for PlatformHandler {
	fn default() -> Self {
		Self::new()
	}
}

/// Default handler for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub struct DefaultHandler;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
impl DefaultHandler {
	pub fn new() -> Self {
		Self
	}
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
#[async_trait::async_trait]
impl EventHandler for DefaultHandler {
	async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
		use crate::event::RawEventKind;

		let Some(path) = event.primary_path().cloned() else {
			return Ok(vec![]);
		};

		let fs_event = match event.kind {
			RawEventKind::Create => FsEvent::create(path),
			RawEventKind::Modify => FsEvent::modify(path),
			RawEventKind::Remove => FsEvent::remove(path),
			RawEventKind::Rename => {
				// Without platform-specific handling, treat rename as modify
				FsEvent::modify(path)
			}
			RawEventKind::Other(_) => return Ok(vec![]),
		};

		Ok(vec![fs_event])
	}

	async fn tick(&self) -> Result<Vec<FsEvent>> {
		Ok(vec![])
	}

	async fn reset(&self) {}
}
