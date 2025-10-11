# Library Organization - Implementation Guide

## Core Types

```rust
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Represents a complete Spacedrive library
pub struct Library {
    /// Root directory of the library
    path: PathBuf,
    
    /// Loaded configuration
    config: LibraryConfig,
    
    /// Database connection
    db: Arc<Database>,
    
    /// Lock file handle (dropped on library close)
    _lock: LibraryLock,
}

/// Library configuration stored in library.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// Version of the config format
    pub version: u32,
    
    /// Unique identifier for the library
    pub id: Uuid,
    
    /// Human-readable name
    pub name: String,
    
    /// Optional description
    pub description: Option<String>,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last modification timestamp  
    pub updated_at: DateTime<Utc>,
    
    /// Library-specific settings
    pub settings: LibrarySettings,
    
    /// Statistics (updated periodically)
    pub statistics: LibraryStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarySettings {
    pub generate_thumbnails: bool,
    pub thumbnail_quality: u8,
    pub enable_ai_tagging: bool,
    pub sync_enabled: bool,
    pub encryption_enabled: bool,
}
```

## Library Manager

```rust
/// Manages all open libraries
pub struct LibraryManager {
    /// Currently open libraries
    libraries: RwLock<HashMap<Uuid, Arc<Library>>>,
    
    /// Configured library locations to scan
    search_paths: Vec<PathBuf>,
    
    /// Event bus for library events
    events: EventBus<LibraryEvent>,
}

impl LibraryManager {
    /// Create a new library
    pub async fn create_library(
        &self,
        name: impl Into<String>,
        location: Option<PathBuf>,
    ) -> Result<Arc<Library>> {
        let name = name.into();
        let safe_name = sanitize_filename(&name);
        
        // Determine location
        let base_path = location.unwrap_or_else(|| {
            self.search_paths.first()
                .cloned()
                .unwrap_or_else(|| dirs::home_dir().unwrap().join("Spacedrive/Libraries"))
        });
        
        // Create library directory
        let library_path = base_path.join(format!("{}.sdlibrary", safe_name));
        if library_path.exists() {
            // Add number suffix if needed
            let library_path = find_unique_path(library_path);
        }
        
        fs::create_dir_all(&library_path).await?;
        
        // Initialize library structure
        self.initialize_library_directory(&library_path, name).await?;
        
        // Open the newly created library
        self.open_library(library_path).await
    }
    
    /// Open a library from a path
    pub async fn open_library(&self, path: impl AsRef<Path>) -> Result<Arc<Library>> {
        let path = path.as_ref();
        
        // Validate it's a library directory
        if !path.extension().map(|e| e == "sdlibrary").unwrap_or(false) {
            return Err(LibraryError::NotALibrary);
        }
        
        // Acquire lock
        let lock = LibraryLock::acquire(path)?;
        
        // Load configuration
        let config_path = path.join("library.json");
        let config: LibraryConfig = load_json(&config_path).await?;
        
        // Check if already open
        if self.libraries.read().await.contains_key(&config.id) {
            return Err(LibraryError::AlreadyOpen);
        }
        
        // Open database
        let db_path = path.join("database.db");
        let db = Database::open(&db_path).await?;
        
        // Run migrations if needed
        db.migrate().await?;
        
        // Create library instance
        let library = Arc::new(Library {
            path: path.to_path_buf(),
            config,
            db,
            _lock: lock,
        });
        
        // Register library
        self.libraries.write().await.insert(library.config.id, library.clone());
        
        // Emit event
        self.events.emit(LibraryEvent::Opened(library.config.id)).await;
        
        Ok(library)
    }
    
    /// Scan configured locations for libraries
    pub async fn scan_locations(&self) -> Result<Vec<DiscoveredLibrary>> {
        let mut discovered = Vec::new();
        
        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }
            
            let mut entries = fs::read_dir(search_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                // Check if it's a library directory
                if path.extension().map(|e| e == "sdlibrary").unwrap_or(false) {
                    match self.read_library_info(&path).await {
                        Ok(info) => discovered.push(info),
                        Err(e) => {
                            error!("Failed to read library at {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
        
        Ok(discovered)
    }
}
```

## Library Directory Operations

```rust
impl LibraryManager {
    /// Initialize a new library directory structure
    async fn initialize_library_directory(
        &self,
        path: &Path,
        name: String,
    ) -> Result<()> {
        // Create subdirectories
        fs::create_dir_all(path.join("thumbnails")).await?;
        fs::create_dir_all(path.join("previews")).await?;
        fs::create_dir_all(path.join("indexes")).await?;
        fs::create_dir_all(path.join("exports")).await?;
        
        // Create initial config
        let config = LibraryConfig {
            version: LIBRARY_VERSION_V2,
            id: Uuid::new_v4(),
            name,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settings: LibrarySettings::default(),
            statistics: LibraryStatistics::default(),
        };
        
        // Save config
        save_json(&path.join("library.json"), &config).await?;
        
        // Initialize database
        let db_path = path.join("database.db");
        let db = Database::create(&db_path).await?;
        db.initialize_schema().await?;
        
        // Create thumbnail metadata
        let thumb_meta = ThumbnailMetadata {
            version: 1,
            quality: 85,
            sizes: vec![128, 256, 512],
        };
        save_json(&path.join("thumbnails/metadata.json"), &thumb_meta).await?;
        
        Ok(())
    }
}
```

## Library Lock Implementation

```rust
/// Prevents concurrent access to a library
pub struct LibraryLock {
    path: PathBuf,
    _file: File,
}

impl LibraryLock {
    pub fn acquire(library_path: &Path) -> Result<Self> {
        let lock_path = library_path.join(".sdlibrary.lock");
        
        // Try to create lock file exclusively
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    // Check if lock is stale
                    if let Ok(metadata) = fs::metadata(&lock_path) {
                        if let Ok(modified) = metadata.modified() {
                            let age = SystemTime::now().duration_since(modified).unwrap_or_default();
                            if age > Duration::from_secs(3600) { // 1 hour
                                // Stale lock, try to remove
                                let _ = fs::remove_file(&lock_path);
                                return LibraryError::StaleLock;
                            }
                        }
                    }
                    LibraryError::AlreadyInUse
                } else {
                    e.into()
                }
            })?;
        
        // Write lock info
        let lock_info = LockInfo {
            node_id: *CURRENT_DEVICE_ID,
            process_id: std::process::id(),
            acquired_at: Utc::now(),
        };
        
        file.write_all(serde_json::to_string(&lock_info)?.as_bytes())?;
        file.sync_all()?;
        
        Ok(Self {
            path: lock_path,
            _file: file,
        })
    }
}

impl Drop for LibraryLock {
    fn drop(&mut self) {
        // Clean up lock file
        let _ = fs::remove_file(&self.path);
    }
}
```

## Thumbnail Management

```rust
impl Library {
    /// Get thumbnail path for a CAS ID
    pub fn thumbnail_path(&self, cas_id: &str) -> PathBuf {
        // Two-level sharding
        let first = &cas_id[0..1];
        let second = &cas_id[1..2];
        
        self.path
            .join("thumbnails")
            .join(first)
            .join(second)
            .join(format!("{}.webp", cas_id))
    }
    
    /// Save a thumbnail
    pub async fn save_thumbnail(
        &self,
        cas_id: &str,
        thumbnail_data: &[u8],
    ) -> Result<()> {
        let path = self.thumbnail_path(cas_id);
        
        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Write thumbnail
        fs::write(&path, thumbnail_data).await?;
        
        Ok(())
    }
    
    /// Check if thumbnail exists
    pub async fn has_thumbnail(&self, cas_id: &str) -> bool {
        self.thumbnail_path(cas_id).exists()
    }
}
```

## Migration from v1

```rust
pub async fn migrate_v1_library(
    old_id: Uuid,
    v1_data_dir: &Path,
    target_dir: &Path,
) -> Result<PathBuf> {
    info!("Starting migration of library {}", old_id);
    
    // Load v1 config
    let v1_config_path = v1_data_dir
        .join("libraries")
        .join(format!("{}.sdlibrary", old_id));
    let v1_config: V1LibraryConfig = load_json(&v1_config_path).await?;
    
    // Create v2 library directory
    let safe_name = sanitize_filename(&v1_config.name);
    let v2_path = target_dir.join(format!("{}.sdlibrary", safe_name));
    fs::create_dir_all(&v2_path).await?;
    
    // Initialize v2 structure
    let manager = LibraryManager::new();
    manager.initialize_library_directory(&v2_path, v1_config.name.clone()).await?;
    
    // Migrate database
    info!("Migrating database...");
    let v1_db_path = v1_data_dir
        .join("libraries")
        .join(format!("{}.db", old_id));
    let v2_db_path = v2_path.join("database.db");
    
    migrate_database_v1_to_v2(&v1_db_path, &v2_db_path).await?;
    
    // Migrate thumbnails with progress
    info!("Migrating thumbnails...");
    let v1_thumb_dir = v1_data_dir.join("thumbnails").join(old_id.to_string());
    let v2_thumb_dir = v2_path.join("thumbnails");
    
    let thumb_count = count_files(&v1_thumb_dir).await?;
    let progress = ProgressBar::new(thumb_count);
    
    migrate_thumbnails_with_progress(&v1_thumb_dir, &v2_thumb_dir, progress).await?;
    
    // Update config with v1 data
    let mut v2_config: LibraryConfig = load_json(&v2_path.join("library.json")).await?;
    v2_config.id = old_id; // Preserve original ID
    v2_config.created_at = v1_config.date_created;
    save_json(&v2_path.join("library.json"), &v2_config).await?;
    
    info!("Migration completed successfully");
    Ok(v2_path)
}
```

## Benefits in Practice

### 1. Simple Backup/Restore
```rust
// Backup
fs::copy_dir_all("My Photos.sdlibrary", "/backup/location")?;

// Restore
fs::copy_dir_all("/backup/My Photos.sdlibrary", "~/Spacedrive/Libraries/")?;
libraries.open_library("~/Spacedrive/Libraries/My Photos.sdlibrary")?;
```

### 2. External Drive Support
```rust
// Open library from external drive
let external_lib = libraries.open_library("/Volumes/Backup/Archive.sdlibrary")?;

// Library works normally, regardless of location
external_lib.search("vacation photos").await?;
```

### 3. Cloud Sync Compatible
```rust
// Libraries in cloud-synced folders just work
let cloud_lib = libraries.open_library("~/Dropbox/Spacedrive/Travel.sdlibrary")?;

// Conflict resolution handled by cloud provider
// Or could implement custom sync-aware locking
```

### 4. Easy Sharing
```rust
// Export library for sharing (with optional exclusions)
library.export_to("/tmp/export.zip", ExportOptions {
    include_thumbnails: true,
    include_previews: false,
    compress: true,
})?;

// Import shared library
libraries.import_from("/tmp/shared-library.zip", "~/Spacedrive/Libraries/")?;
```

## Future-Proofing

The design supports adding new features without breaking existing libraries:

```rust
// New feature: Add AI embeddings
// Just add new directory - no migration needed
fs::create_dir_all(library.path.join("embeddings"))?;

// New feature: Version history
// Add versions directory
fs::create_dir_all(library.path.join("versions"))?;

// New setting: Just update config
library.config.settings.enable_version_history = true;
library.save_config().await?;
```

This implementation provides a solid foundation for portable, self-contained libraries that solve all the issues with the current system.