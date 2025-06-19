//! Spacedrive Core v2
//! 
//! A unified, simplified architecture for cross-platform file management.

pub mod config;
pub mod device;
pub mod domain;
pub mod file_type;
pub mod infrastructure;
pub mod library;
pub mod operations;
pub mod services;
pub mod shared;

use crate::config::AppConfig;
use crate::device::DeviceManager;
use crate::infrastructure::events::{Event, EventBus};
use crate::library::LibraryManager;
use crate::services::Services;
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
        
        // 4. Initialize library manager with libraries directory
        let libraries_dir = config.read().await.libraries_dir();
        let libraries = Arc::new(LibraryManager::new_with_dir(libraries_dir, events.clone()));
        
        // 5. Auto-load all libraries
        info!("Loading existing libraries...");
        match libraries.load_all().await {
            Ok(count) => info!("Loaded {} libraries", count),
            Err(e) => error!("Failed to load libraries: {}", e),
        }
        
        // 6. Initialize services (placeholder for now)
        let services = Services::new();
        
        // 7. Emit startup event
        events.emit(Event::CoreStarted);
        
        Ok(Self {
            config,
            device,
            libraries,
            events,
            services,
        })
    }
    
    /// Get the application configuration
    pub fn config(&self) -> Arc<RwLock<AppConfig>> {
        self.config.clone()
    }
    
    /// Shutdown the core gracefully
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Shutting down Spacedrive Core...");
        
        // Stop all services
        self.services.stop_all().await?;
        
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