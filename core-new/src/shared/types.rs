//! Core type definitions

use std::path::{Path, PathBuf};
use std::fmt;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// A path within the Spacedrive Virtual Distributed File System
/// 
/// This is the core abstraction that enables cross-device operations.
/// An SdPath can represent:
/// - A local file on this device
/// - A file on another device in the same library
/// - A cloud-synced file
/// - A file that exists in multiple locations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SdPath {
    /// The device where this file exists
    pub device_id: Uuid,
    
    /// The local path on that device
    pub path: PathBuf,
    
    /// Optional library context
    /// If None, uses the current active library
    pub library_id: Option<Uuid>,
}

impl SdPath {
    /// Create a new SdPath
    pub fn new(device_id: Uuid, path: impl Into<PathBuf>) -> Self {
        Self {
            device_id,
            path: path.into(),
            library_id: None,
        }
    }
    
    /// Create an SdPath with a specific library
    pub fn with_library(device_id: Uuid, path: impl Into<PathBuf>, library_id: Uuid) -> Self {
        Self {
            device_id,
            path: path.into(),
            library_id: Some(library_id),
        }
    }
    
    /// Create an SdPath for a local file on this device
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            device_id: get_current_device_id(), // Get the current device ID
            path: path.into(),
            library_id: None,
        }
    }
    
    /// Check if this path is on the current device
    pub fn is_local(&self) -> bool {
        self.device_id == get_current_device_id()
    }
    
    /// Get the local PathBuf if this is a local path
    pub fn as_local_path(&self) -> Option<&Path> {
        if self.is_local() {
            Some(&self.path)
        } else {
            None
        }
    }
    
    /// Convert to a display string
    pub fn display(&self) -> String {
        if self.is_local() {
            self.path.display().to_string()
        } else {
            format!("{}:{}", self.device_id, self.path.display())
        }
    }
    
    /// Get just the file name
    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name()?.to_str()
    }
    
    /// Get the parent directory as an SdPath
    pub fn parent(&self) -> Option<SdPath> {
        self.path.parent().map(|p| SdPath {
            device_id: self.device_id,
            path: p.to_path_buf(),
            library_id: self.library_id,
        })
    }
    
    /// Join with another path component
    pub fn join(&self, path: impl AsRef<Path>) -> SdPath {
        SdPath {
            device_id: self.device_id,
            path: self.path.join(path),
            library_id: self.library_id,
        }
    }
}

impl fmt::Display for SdPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

use std::sync::RwLock;

/// Global reference to current device ID
/// This is set during Core initialization
pub static CURRENT_DEVICE_ID: once_cell::sync::Lazy<RwLock<Uuid>> = 
    once_cell::sync::Lazy::new(|| RwLock::new(Uuid::nil()));

/// Initialize the current device ID
pub fn set_current_device_id(id: Uuid) {
    if let Ok(mut device_id) = CURRENT_DEVICE_ID.write() {
        *device_id = id;
    }
}

/// Get the current device ID
pub fn get_current_device_id() -> Uuid {
    match CURRENT_DEVICE_ID.read() {
        Ok(guard) => *guard,
        Err(_) => Uuid::nil(),
    }
}

/// A batch of SdPaths, useful for operations on multiple files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdPathBatch {
    pub paths: Vec<SdPath>,
}

impl SdPathBatch {
    /// Create a new batch
    pub fn new(paths: Vec<SdPath>) -> Self {
        Self { paths }
    }
    
    /// Filter to only local paths
    pub fn local_only(&self) -> Vec<&Path> {
        self.paths.iter()
            .filter_map(|p| p.as_local_path())
            .collect()
    }
    
    /// Group by device
    pub fn by_device(&self) -> std::collections::HashMap<Uuid, Vec<&SdPath>> {
        let mut map = std::collections::HashMap::new();
        for path in &self.paths {
            map.entry(path.device_id).or_insert_with(Vec::new).push(path);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sdpath_creation() {
        let device_id = Uuid::new_v4();
        let path = SdPath::new(device_id, "/home/user/file.txt");
        
        assert_eq!(path.device_id, device_id);
        assert_eq!(path.path, PathBuf::from("/home/user/file.txt"));
        assert_eq!(path.library_id, None);
    }
    
    #[test]
    fn test_sdpath_display() {
        let device_id = Uuid::new_v4();
        let path = SdPath::new(device_id, "/home/user/file.txt");
        
        let display = path.display();
        assert!(display.contains(&device_id.to_string()));
        assert!(display.contains("/home/user/file.txt"));
    }
}