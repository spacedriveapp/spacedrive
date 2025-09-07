# Core Lifecycle Design

## Overview

The Spacedrive core manages the complete lifecycle of the application, including configuration, library management, and service coordination.

## Directory Structure

```
$DATA_DIR/
├── spacedrive.json         # Main application config
├── libraries/              # All library data
│   ├── {uuid}/
│   │   ├── library.json   # Library metadata
│   │   ├── database.db    # SQLite database
│   │   ├── thumbnails/    # Thumbnail cache
│   │   ├── previews/      # Preview cache
│   │   ├── indexes/       # Search indexes
│   │   └── exports/       # Export temp files
│   └── {uuid}/...
├── logs/                   # Application logs
│   ├── spacedrive.log     # Current log
│   └── spacedrive.{n}.log # Rotated logs
└── device.json            # Device-specific config
```

## Core Initialization Flow

```rust
// 1. Load or create app config
let config = AppConfig::load_or_create(&data_dir)?;

// 2. Initialize device manager
let device_manager = DeviceManager::new(&data_dir)?;

// 3. Create event bus
let events = EventBus::new();

// 4. Initialize library manager
let libraries = LibraryManager::new(&data_dir.join("libraries"), events.clone())?;

// 5. Auto-load all libraries
libraries.load_all().await?;

// 6. Start background services
let location_watcher = LocationWatcher::new();
let job_manager = JobManager::new();
let thumbnail_service = ThumbnailService::new();

// 7. Create core instance
let core = Core {
    config,
    device: device_manager,
    libraries,
    events,
    services: Services {
        locations: location_watcher,
        jobs: job_manager,
        thumbnails: thumbnail_service,
    },
};
```

## Configuration System

### Application Config (`spacedrive.json`)
```json
{
  "version": 1,
  "data_dir": "/Users/jamie/Library/Application Support/spacedrive",
  "log_level": "info",
  "telemetry_enabled": true,
  "p2p": {
    "enabled": true,
    "discovery": "local"
  },
  "preferences": {
    "theme": "dark",
    "language": "en"
  }
}
```

### Device Config (`device.json`)
```json
{
  "version": 1,
  "id": "3e19b8fd-ab4a-4094-8502-4db233d5e955",
  "name": "Jamie's MacBook Pro",
  "created_at": "2024-01-01T00:00:00Z",
  "p2p_identity": "base64_encoded_key"
}
```

### Library Config (`{uuid}/library.json`)
```json
{
  "version": 1,
  "id": "fc06414a-683c-41e1-94a7-28e00e1ab880",
  "name": "Main Library",
  "description": "My primary Spacedrive library",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "cloud_sync": {
    "enabled": false,
    "provider": null
  }
}
```

## Core Struct

```rust
pub struct Core {
    /// Application configuration
    config: Arc<RwLock<AppConfig>>,
    
    /// Device management
    pub device: Arc<DeviceManager>,
    
    /// Library management
    pub libraries: Arc<LibraryManager>,
    
    /// Event broadcasting
    pub events: Arc<EventBus>,
    
    /// Background services
    services: Services,
}

struct Services {
    locations: Arc<LocationWatcher>,
    jobs: Arc<JobManager>,
    thumbnails: Arc<ThumbnailService>,
}
```

## Key Methods

```rust
impl Core {
    /// Initialize core with default data directory
    pub async fn new() -> Result<Self> {
        let data_dir = AppConfig::default_data_dir()?;
        Self::new_with_config(data_dir).await
    }
    
    /// Initialize core with custom data directory
    pub async fn new_with_config(data_dir: PathBuf) -> Result<Self> {
        // ... initialization flow ...
    }
    
    /// Shutdown core gracefully
    pub async fn shutdown(&self) -> Result<()> {
        // Stop all services
        self.services.locations.stop().await?;
        self.services.jobs.stop().await?;
        self.services.thumbnails.stop().await?;
        
        // Close all libraries
        self.libraries.close_all().await?;
        
        // Save config
        self.config.write().await.save()?;
        
        Ok(())
    }
}
```

## Library Lifecycle

```rust
impl LibraryManager {
    /// Load all libraries from disk
    pub async fn load_all(&self) -> Result<()> {
        let entries = fs::read_dir(&self.libraries_dir)?;
        
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                match self.load_library(&path).await {
                    Ok(library) => {
                        info!("Loaded library: {}", library.name());
                    }
                    Err(e) => {
                        error!("Failed to load library at {:?}: {}", path, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Create a new library
    pub async fn create_library(&self, name: &str) -> Result<Arc<Library>> {
        let id = Uuid::new_v4();
        let library_dir = self.libraries_dir.join(id.to_string());
        
        // Create directory structure
        fs::create_dir_all(&library_dir)?;
        fs::create_dir(&library_dir.join("thumbnails"))?;
        fs::create_dir(&library_dir.join("previews"))?;
        fs::create_dir(&library_dir.join("indexes"))?;
        fs::create_dir(&library_dir.join("exports"))?;
        
        // Create config
        let config = LibraryConfig {
            version: 1,
            id,
            name: name.to_string(),
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cloud_sync: CloudSync::default(),
        };
        
        // Save config
        let config_path = library_dir.join("library.json");
        let json = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, json)?;
        
        // Create database
        let db_path = library_dir.join("database.db");
        let db = DatabaseConnection::create(&db_path).await?;
        
        // Create library instance
        let library = Arc::new(Library::new(config, db, library_dir));
        
        // Register in active libraries
        self.libraries.write().await.insert(id, library.clone());
        
        // Emit event
        self.events.emit(Event::LibraryCreated { id, name: name.to_string() });
        
        Ok(library)
    }
}
```

## Event System

```rust
#[derive(Clone, Debug)]
pub enum Event {
    // Library events
    LibraryCreated { id: Uuid, name: String },
    LibraryLoaded { id: Uuid },
    LibraryDeleted { id: Uuid },
    
    // Location events
    LocationAdded { library_id: Uuid, location_id: Uuid },
    LocationScanning { library_id: Uuid, location_id: Uuid },
    LocationIndexed { library_id: Uuid, location_id: Uuid, file_count: usize },
    
    // Entry events
    EntryDiscovered { library_id: Uuid, entry_id: Uuid },
    EntryModified { library_id: Uuid, entry_id: Uuid },
    EntryDeleted { library_id: Uuid, entry_id: Uuid },
}

pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }
    
    pub fn emit(&self, event: Event) {
        let _ = self.sender.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}
```

## Migration System

```rust
pub trait Migrate {
    fn current_version(&self) -> u32;
    fn target_version() -> u32;
    fn migrate(&mut self) -> Result<()>;
}

impl Migrate for AppConfig {
    fn current_version(&self) -> u32 {
        self.version
    }
    
    fn target_version() -> u32 {
        1 // Current schema version
    }
    
    fn migrate(&mut self) -> Result<()> {
        match self.version {
            0 => {
                // Migration from v0 to v1
                self.version = 1;
                Ok(())
            }
            1 => Ok(()), // Already at target
            v => Err(anyhow!("Unknown config version: {}", v)),
        }
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Library error: {0}")]
    Library(#[from] LibraryError),
    
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Platform-Specific Data Directories

```rust
impl AppConfig {
    pub fn default_data_dir() -> Result<PathBuf> {
        #[cfg(target_os = "macos")]
        let dir = dirs::data_dir()
            .ok_or_else(|| anyhow!("Could not determine data directory"))?
            .join("spacedrive");
        
        #[cfg(target_os = "windows")]
        let dir = dirs::data_dir()
            .ok_or_else(|| anyhow!("Could not determine data directory"))?
            .join("Spacedrive");
        
        #[cfg(target_os = "linux")]
        let dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow!("Could not determine data directory"))?
            .join("spacedrive");
        
        Ok(dir)
    }
}
```

## Usage Example

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize with default data directory
    let core = Core::new().await?;
    
    // Or with custom data directory
    let core = Core::new_with_config("/custom/data/dir".into()).await?;
    
    // Subscribe to events
    let mut events = core.events.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("Event: {:?}", event);
        }
    });
    
    // Create a library if none exist
    if core.libraries.list().await.is_empty() {
        let library = core.libraries.create_library("My Library").await?;
        println!("Created library: {}", library.id());
    }
    
    // Run until shutdown
    tokio::signal::ctrl_c().await?;
    
    // Graceful shutdown
    core.shutdown().await?;
    
    Ok(())
}
```

## Next Steps

1. Implement AppConfig with load/save functionality
2. Update Core::new() to follow this lifecycle
3. Add LibraryManager with auto-loading
4. Implement EventBus for reactivity
5. Add migration system for configs
6. Create background service management