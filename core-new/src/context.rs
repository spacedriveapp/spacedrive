//! Shared context providing access to core application components.

//! Shared context providing access to core application components.

use crate::{
	device::DeviceManager, infrastructure::events::EventBus,
	keys::library_key_manager::LibraryKeyManager, library::LibraryManager,
	services::networking::NetworkingService, volume::VolumeManager,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared context providing access to core application components.
#[derive(Clone)]
pub struct CoreContext {
	pub events: Arc<EventBus>,
	pub device_manager: Arc<DeviceManager>,
	pub library_manager: Arc<LibraryManager>,
	pub volume_manager: Arc<VolumeManager>,
	pub library_key_manager: Arc<LibraryKeyManager>,
	// This is wrapped in an RwLock to allow it to be set after initialization
	pub networking: Arc<RwLock<Option<Arc<NetworkingService>>>>,
}

impl CoreContext {
	/// Create a new context with the given components
	pub fn new(
		events: Arc<EventBus>,
		device_manager: Arc<DeviceManager>,
		library_manager: Arc<LibraryManager>,
		volume_manager: Arc<VolumeManager>,
		library_key_manager: Arc<LibraryKeyManager>,
	) -> Self {
		Self {
			events,
			device_manager,
			library_manager,
			volume_manager,
			library_key_manager,
			networking: Arc::new(RwLock::new(None)),
		}
	}

	/// Helper method for services to get the networking service
	pub async fn get_networking(&self) -> Option<Arc<NetworkingService>> {
		self.networking.read().await.clone()
	}

	/// Method for Core to set networking after it's initialized
	pub async fn set_networking(&self, networking: Arc<NetworkingService>) {
		*self.networking.write().await = Some(networking);
	}
}
