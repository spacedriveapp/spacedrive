//! Spacedrive Core v2
//! 
//! A unified, simplified architecture for cross-platform file management.

pub mod config;
pub mod device;
pub mod domain;
pub mod file_type;
pub mod infrastructure;
pub mod library;
pub mod location;
pub mod operations;
pub mod services;
pub mod shared;
pub mod volume;

use crate::config::AppConfig;
use crate::device::DeviceManager;
use crate::infrastructure::events::{Event, EventBus};
use crate::library::LibraryManager;
use crate::services::Services;
use crate::volume::{VolumeManager, VolumeDetectionConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

/// The main context for all core operations
pub struct Core {
    /// Application configuration
    config: Arc<RwLock<AppConfig>>,
    
    /// Device manager
    pub device: Arc<DeviceManager>,
    
    /// Library manager
    pub libraries: Arc<LibraryManager>,
    
    /// Volume manager
    pub volumes: Arc<VolumeManager>,
    
    /// Event bus for state changes
    pub events: Arc<EventBus>,
    
    /// Background services
    services: Services,
}

impl Core {
    /// Initialize a new Core instance with default data directory
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = crate::config::default_data_dir()?;
        Self::new_with_config(data_dir).await
    }
    
    /// Initialize a new Core instance with custom data directory
    pub async fn new_with_config(data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing Spacedrive Core at {:?}", data_dir);
        
        // 1. Load or create app config
        let config = AppConfig::load_or_create(&data_dir)?;
        config.ensure_directories()?;
        let config = Arc::new(RwLock::new(config));
        
        // 2. Initialize device manager
        let device = Arc::new(DeviceManager::init_with_path(&data_dir)?);        
        // Set the global device ID for legacy compatibility
        shared::types::set_current_device_id(device.device_id()?);
        
        // 3. Create event bus
        let events = Arc::new(EventBus::default());
        
        // 4. Initialize volume manager
        let volume_config = VolumeDetectionConfig::default();
        let volumes = Arc::new(VolumeManager::new(volume_config, events.clone()));
        
        // 5. Initialize volume detection
        info!("Initializing volume detection...");
        match volumes.initialize().await {
            Ok(()) => info!("Volume manager initialized"),
            Err(e) => error!("Failed to initialize volume manager: {}", e),
        }
        
        // 6. Initialize library manager with libraries directory
        let libraries_dir = config.read().await.libraries_dir();
        let libraries = Arc::new(LibraryManager::new_with_dir(libraries_dir, events.clone()));
        
        // 7. Auto-load all libraries
        info!("Loading existing libraries...");
        match libraries.load_all().await {
            Ok(count) => info!("Loaded {} libraries", count),
            Err(e) => error!("Failed to load libraries: {}", e),
        }
        
        // 8. Initialize and start services
        let services = Services::new(events.clone());
        
        info!("Starting background services...");
        match services.start_all().await {
            Ok(()) => info!("Background services started"),
            Err(e) => error!("Failed to start services: {}", e),
        }
        
        // 9. Emit startup event
        events.emit(Event::CoreStarted);
        
        Ok(Self {
            config,
            device,
            libraries,
            volumes,
            events,
            services,
        })
    }
    
    /// Get the application configuration
    pub fn config(&self) -> Arc<RwLock<AppConfig>> {
        self.config.clone()
    }
    
    /// Add a location to the file system watcher
    pub async fn add_watched_location(
        &self,
        location_id: uuid::Uuid,
        library_id: uuid::Uuid,
        path: std::path::PathBuf,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::services::location_watcher::WatchedLocation;
        
        let watched_location = WatchedLocation {
            id: location_id,
            library_id,
            path,
            enabled,
        };
        
        self.services.location_watcher.add_location(watched_location).await?;
        Ok(())
    }
    
    /// Remove a location from the file system watcher
    pub async fn remove_watched_location(
        &self,
        location_id: uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.services.location_watcher.remove_location(location_id).await?;
        Ok(())
    }
    
    /// Update file watching settings for a location
    pub async fn update_watched_location(
        &self,
        location_id: uuid::Uuid,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.services.location_watcher.update_location(location_id, enabled).await?;
        Ok(())
    }
    
    /// Get all currently watched locations
    pub async fn get_watched_locations(&self) -> Vec<crate::services::location_watcher::WatchedLocation> {
        self.services.location_watcher.get_watched_locations().await
    }
    
    /// Shutdown the core gracefully
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Shutting down Spacedrive Core...");
        
        // Stop all services
        self.services.stop_all().await?;
        
        // Stop volume monitoring
        self.volumes.stop_monitoring().await;
        
        // Close all libraries
        self.libraries.close_all().await?;
        
        // Save configuration
        self.config.write().await.save()?;
        
        // Emit shutdown event
        self.events.emit(Event::CoreShutdown);
        
        info!("Spacedrive Core shutdown complete");
        Ok(())
    }
}