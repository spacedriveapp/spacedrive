//! Library manager - handles creation, opening, and discovery of libraries

use super::{
    config::{LibraryConfig, LibrarySettings, LibraryStatistics, ThumbnailMetadata},
    error::{LibraryError, Result},
    lock::LibraryLock,
    Library, LIBRARY_CONFIG_VERSION, LIBRARY_EXTENSION,
};
use crate::infrastructure::database::Database;
use crate::infrastructure::events::{Event, EventBus};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Information about a discovered library
#[derive(Debug, Clone)]
pub struct DiscoveredLibrary {
    /// Path to the library directory
    pub path: PathBuf,
    
    /// Library configuration
    pub config: LibraryConfig,
    
    /// Whether the library is currently locked
    pub is_locked: bool,
}

/// Manages all Spacedrive libraries
pub struct LibraryManager {
    /// Currently open libraries
    libraries: Arc<RwLock<HashMap<Uuid, Arc<Library>>>>,
    
    /// Paths to search for libraries
    search_paths: Vec<PathBuf>,
    
    /// Event bus for library events
    event_bus: Arc<EventBus>,
}

impl LibraryManager {
    /// Create a new library manager
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        // Default search paths
        let mut search_paths = vec![];
        
        // Add user's home directory
        if let Some(home) = dirs::home_dir() {
            search_paths.push(home.join("Spacedrive").join("Libraries"));
        }
        
        Self {
            libraries: Arc::new(RwLock::new(HashMap::new())),
            search_paths,
            event_bus,
        }
    }
    
    /// Create a new library manager with a specific libraries directory
    pub fn new_with_dir(libraries_dir: PathBuf, event_bus: Arc<EventBus>) -> Self {
        let search_paths = vec![libraries_dir];
        
        Self {
            libraries: Arc::new(RwLock::new(HashMap::new())),
            search_paths,
            event_bus,
        }
    }
    
    /// Add a search path for libraries
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }
    
    /// Create a new library
    pub async fn create_library(
        &self,
        name: impl Into<String>,
        location: Option<PathBuf>,
    ) -> Result<Arc<Library>> {
        let name = name.into();
        
        // Validate name
        if name.is_empty() {
            return Err(LibraryError::InvalidName("Name cannot be empty".to_string()));
        }
        
        // Sanitize name for filesystem
        let safe_name = sanitize_filename(&name);
        
        // Determine base path
        let base_path = location.unwrap_or_else(|| {
            self.search_paths
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("Spacedrive")
                        .join("Libraries")
                })
        });
        
        // Ensure base path exists
        tokio::fs::create_dir_all(&base_path).await?;
        
        // Find unique library path
        let library_path = find_unique_library_path(&base_path, &safe_name).await?;
        
        // Create library directory
        tokio::fs::create_dir_all(&library_path).await?;
        
        // Initialize library
        self.initialize_library(&library_path, name).await?;
        
        // Open the newly created library
        let library = self.open_library(&library_path).await?;
        
        // Emit event
        self.event_bus.emit(Event::LibraryCreated {
            id: library.id(),
            name: library.name().await,
            path: library_path,
        });
        
        Ok(library)
    }
    
    /// Open a library from a path
    pub async fn open_library(&self, path: impl AsRef<Path>) -> Result<Arc<Library>> {
        let path = path.as_ref();
        
        // Validate it's a library directory
        if !is_library_directory(path) {
            return Err(LibraryError::NotALibrary(path.to_path_buf()));
        }
        
        // Acquire lock
        let lock = LibraryLock::acquire(path)?;
        
        // Load configuration
        let config_path = path.join("library.json");
        let config_data = tokio::fs::read_to_string(&config_path).await?;
        let config: LibraryConfig = serde_json::from_str(&config_data)?;
        
        // Check if already open
        {
            let libraries = self.libraries.read().await;
            if libraries.contains_key(&config.id) {
                return Err(LibraryError::AlreadyOpen(config.id));
            }
        }
        
        // Open database
        let db_path = path.join("database.db");
        let db = Arc::new(Database::open(&db_path).await?);
        
        // Create library instance
        let library = Arc::new(Library {
            path: path.to_path_buf(),
            config: RwLock::new(config.clone()),
            db,
            _lock: lock,
        });
        
        // Register library
        {
            let mut libraries = self.libraries.write().await;
            libraries.insert(config.id, library.clone());
        }
        
        // Emit event
        self.event_bus.emit(Event::LibraryOpened {
            id: config.id,
            name: config.name,
            path: path.to_path_buf(),
        });
        
        info!("Opened library {} at {:?}", library.id(), path);
        
        Ok(library)
    }
    
    /// Close a library
    pub async fn close_library(&self, id: Uuid) -> Result<()> {
        let library = {
            let mut libraries = self.libraries.write().await;
            libraries.remove(&id)
        };
        
        if let Some(library) = library {
            let name = library.name().await;
            
            // Emit event
            self.event_bus.emit(Event::LibraryClosed { id, name });
            
            info!("Closed library {}", id);
            Ok(())
        } else {
            Err(LibraryError::NotFound(id.to_string()))
        }
    }
    
    /// Get an open library by ID
    pub async fn get_library(&self, id: Uuid) -> Option<Arc<Library>> {
        self.libraries.read().await.get(&id).cloned()
    }
    
    /// Get all open libraries
    pub async fn get_open_libraries(&self) -> Vec<Arc<Library>> {
        self.libraries.read().await.values().cloned().collect()
    }
    
    /// List all open libraries
    pub async fn list(&self) -> Vec<Arc<Library>> {
        self.get_open_libraries().await
    }
    
    /// Load all libraries from the search paths
    pub async fn load_all(&self) -> Result<usize> {
        let mut loaded_count = 0;
        
        for search_path in &self.search_paths.clone() {
            if !search_path.exists() {
                info!("Search path {:?} does not exist, skipping", search_path);
                continue;
            }
            
            match tokio::fs::read_dir(search_path).await {
                Ok(mut entries) => {
                    while let Some(entry) = entries.next_entry().await? {
                        let path = entry.path();
                        
                        if is_library_directory(&path) {
                            match self.open_library(&path).await {
                                Ok(_) => {
                                    loaded_count += 1;
                                    info!("Auto-loaded library from {:?}", path);
                                }
                                Err(LibraryError::AlreadyOpen(_)) => {
                                    // Library is already open, skip
                                }
                                Err(e) => {
                                    warn!("Failed to auto-load library from {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read directory {:?}: {}", search_path, e);
                }
            }
        }
        
        Ok(loaded_count)
    }
    
    /// Close all open libraries
    pub async fn close_all(&self) -> Result<()> {
        let library_ids: Vec<Uuid> = self.libraries.read().await.keys().cloned().collect();
        
        for id in library_ids {
            if let Err(e) = self.close_library(id).await {
                error!("Failed to close library {}: {}", id, e);
            }
        }
        
        Ok(())
    }
    
    /// Scan search paths for libraries
    pub async fn scan_for_libraries(&self) -> Result<Vec<DiscoveredLibrary>> {
        let mut discovered = Vec::new();
        
        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }
            
            let mut entries = tokio::fs::read_dir(search_path).await?;
            
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                if is_library_directory(&path) {
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
    
    /// Initialize a new library directory
    async fn initialize_library(&self, path: &Path, name: String) -> Result<()> {
        // Create subdirectories
        tokio::fs::create_dir_all(path.join("thumbnails")).await?;
        tokio::fs::create_dir_all(path.join("previews")).await?;
        tokio::fs::create_dir_all(path.join("indexes")).await?;
        tokio::fs::create_dir_all(path.join("exports")).await?;
        
        // Create configuration
        let config = LibraryConfig {
            version: LIBRARY_CONFIG_VERSION,
            id: Uuid::new_v4(),
            name,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settings: LibrarySettings::default(),
            statistics: LibraryStatistics::default(),
        };
        
        // Save configuration
        let config_path = path.join("library.json");
        let json = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(config_path, json).await?;
        
        // Initialize database
        let db_path = path.join("database.db");
        let db = Database::create(&db_path).await?;
        
        // Run initial migrations
        db.migrate().await?;
        
        // Create thumbnail metadata
        let thumb_meta = ThumbnailMetadata::default();
        let thumb_meta_path = path.join("thumbnails").join("metadata.json");
        let json = serde_json::to_string_pretty(&thumb_meta)?;
        tokio::fs::write(thumb_meta_path, json).await?;
        
        info!("Initialized new library '{}' at {:?}", config.name, path);
        
        Ok(())
    }
    
    /// Read library information without opening it
    async fn read_library_info(&self, path: &Path) -> Result<DiscoveredLibrary> {
        let config_path = path.join("library.json");
        let config_data = tokio::fs::read_to_string(&config_path).await?;
        let config: LibraryConfig = serde_json::from_str(&config_data)?;
        
        // Check if locked
        let is_locked = path.join(".sdlibrary.lock").exists();
        
        Ok(DiscoveredLibrary {
            path: path.to_path_buf(),
            config,
            is_locked,
        })
    }
}

/// Check if a path is a library directory
fn is_library_directory(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == LIBRARY_EXTENSION)
        .unwrap_or(false)
}

/// Sanitize a filename for safe filesystem usage
fn sanitize_filename(name: &str) -> String {
    // Replace problematic characters
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Find a unique library path by adding numbers if needed
async fn find_unique_library_path(base_path: &Path, name: &str) -> Result<PathBuf> {
    let mut path = base_path.join(format!("{}.{}", name, LIBRARY_EXTENSION));
    let mut counter = 1;
    
    while path.exists() {
        path = base_path.join(format!("{} {}.{}", name, counter, LIBRARY_EXTENSION));
        counter += 1;
        
        if counter > 1000 {
            return Err(LibraryError::Other(
                "Could not find unique library name".to_string(),
            ));
        }
    }
    
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Library"), "My Library");
        assert_eq!(sanitize_filename("My/Library"), "My-Library");
        assert_eq!(sanitize_filename("My\\Library"), "My-Library");
        assert_eq!(sanitize_filename("My:Library"), "My-Library");
        assert_eq!(sanitize_filename("My*Library?"), "My-Library-");
    }
    
    #[tokio::test]
    async fn test_is_library_directory() {
        assert!(is_library_directory(Path::new("/path/to/My Library.sdlibrary")));
        assert!(!is_library_directory(Path::new("/path/to/My Library")));
        assert!(!is_library_directory(Path::new("/path/to/My Library.txt")));
    }
}