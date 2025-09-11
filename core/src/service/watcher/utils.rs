//! Utility functions for file system watching

use std::path::{Path, PathBuf};
use tracing::debug;

/// Check if a path should be ignored by the watcher
pub fn should_ignore_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    
    // Skip system directories
    if path_str.contains("/.git/") ||
       path_str.contains("/.svn/") ||
       path_str.contains("/.hg/") ||
       path_str.contains("/node_modules/") ||
       path_str.contains("/.vscode/") ||
       path_str.contains("/.idea/") ||
       path_str.contains("/target/") ||
       path_str.contains("/build/") ||
       path_str.contains("/dist/") {
        return true;
    }
    
    // Skip system files
    if let Some(file_name) = path.file_name() {
        let name = file_name.to_string_lossy();
        if name == ".DS_Store" ||
           name == "Thumbs.db" ||
           name == "desktop.ini" ||
           name.starts_with("._") ||
           name.starts_with("~$") {
            return true;
        }
    }
    
    false
}

/// Extract the relative path from a location root
pub fn extract_relative_path(location_root: &Path, full_path: &Path) -> Option<PathBuf> {
    full_path.strip_prefix(location_root)
        .ok()
        .map(|p| p.to_path_buf())
}

/// Check if a path is a subdirectory of another path
pub fn is_subdirectory(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent) && child != parent
}

/// Normalize path separators for cross-platform compatibility
pub fn normalize_path(path: &Path) -> PathBuf {
    // Convert all separators to forward slashes for internal storage
    let path_str = path.to_string_lossy();
    let normalized = path_str.replace('\\', "/");
    PathBuf::from(normalized)
}

/// Check if a file extension indicates it should be watched
pub fn should_watch_extension(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        
        // Skip some binary and cache files
        !matches!(ext.as_str(),
            "tmp" | "temp" | "cache" | "log" | "lock" | "pid" |
            "swap" | "swp" | "bak" | "old" | "orig"
        )
    } else {
        true // Files without extensions are usually important
    }
}

/// Get a human-readable description of a file system event
pub fn describe_event(event_kind: &str, path: &Path) -> String {
    match event_kind {
        "create" => format!("Created: {}", path.display()),
        "modify" => format!("Modified: {}", path.display()),
        "remove" => format!("Removed: {}", path.display()),
        "rename" => format!("Renamed: {}", path.display()),
        _ => format!("{}: {}", event_kind, path.display()),
    }
}

/// Calculate a simple hash for a path (for inode fallback on non-Unix systems)
pub fn path_hash(path: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

/// Check if a directory is empty
pub async fn is_directory_empty(path: &Path) -> bool {
    match tokio::fs::read_dir(path).await {
        Ok(mut entries) => entries.next_entry().await.map_or(true, |e| e.is_none()),
        Err(_) => true, // If we can't read it, consider it empty
    }
}

/// Get file size safely
pub async fn get_file_size(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Check if a path is likely a temporary file based on naming patterns
pub fn is_likely_temporary(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Common temporary file patterns across platforms
    path_str.contains(".tmp") ||
    path_str.contains(".temp") ||
    path_str.contains(".partial") ||
    path_str.contains(".part") ||
    path_str.contains(".crdownload") ||
    path_str.contains(".download") ||
    path_str.ends_with("~") ||
    path_str.starts_with(".#") ||
    path_str.contains(".swp") ||
    path_str.contains(".swo")
}

/// Debounce helper that tracks the last time a path was seen
pub struct PathDebouncer {
    last_seen: std::collections::HashMap<PathBuf, std::time::Instant>,
    debounce_duration: std::time::Duration,
}

impl PathDebouncer {
    pub fn new(debounce_duration: std::time::Duration) -> Self {
        Self {
            last_seen: std::collections::HashMap::new(),
            debounce_duration,
        }
    }
    
    /// Check if a path should be debounced (returns true if should skip)
    pub fn should_debounce(&mut self, path: &Path) -> bool {
        let now = std::time::Instant::now();
        let path_buf = path.to_path_buf();
        
        if let Some(&last_time) = self.last_seen.get(&path_buf) {
            if now.duration_since(last_time) < self.debounce_duration {
                return true;
            }
        }
        
        self.last_seen.insert(path_buf, now);
        false
    }
    
    /// Clean up old entries to prevent memory leaks
    pub fn cleanup_old_entries(&mut self) {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(60);
        self.last_seen.retain(|_, &mut last_time| last_time > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_should_ignore_path() {
        assert!(should_ignore_path(Path::new("/project/.git/config")));
        assert!(should_ignore_path(Path::new("/project/node_modules/package")));
        assert!(should_ignore_path(Path::new("/project/.DS_Store")));
        assert!(!should_ignore_path(Path::new("/project/src/main.rs")));
    }

    #[test]
    fn test_extract_relative_path() {
        let root = Path::new("/home/user/project");
        let full = Path::new("/home/user/project/src/main.rs");
        let relative = extract_relative_path(root, full).unwrap();
        assert_eq!(relative, Path::new("src/main.rs"));
    }

    #[test]
    fn test_is_subdirectory() {
        let parent = Path::new("/home/user");
        let child = Path::new("/home/user/project");
        let other = Path::new("/home/other");
        
        assert!(is_subdirectory(parent, child));
        assert!(!is_subdirectory(parent, other));
        assert!(!is_subdirectory(parent, parent)); // Same path
    }

    #[test]
    fn test_should_watch_extension() {
        assert!(should_watch_extension(Path::new("file.txt")));
        assert!(should_watch_extension(Path::new("file.rs")));
        assert!(!should_watch_extension(Path::new("file.tmp")));
        assert!(!should_watch_extension(Path::new("file.cache")));
        assert!(should_watch_extension(Path::new("README"))); // No extension
    }

    #[test]
    fn test_is_likely_temporary() {
        assert!(is_likely_temporary(Path::new("file.tmp")));
        assert!(is_likely_temporary(Path::new("download.part")));
        assert!(is_likely_temporary(Path::new("document.docx.crdownload")));
        assert!(is_likely_temporary(Path::new("file~")));
        assert!(!is_likely_temporary(Path::new("important.txt")));
    }

    #[test]
    fn test_path_debouncer() {
        let mut debouncer = PathDebouncer::new(Duration::from_millis(100));
        let path = Path::new("/test/file.txt");
        
        // First call should not debounce
        assert!(!debouncer.should_debounce(path));
        
        // Immediate second call should debounce
        assert!(debouncer.should_debounce(path));
        
        // Different path should not debounce
        assert!(!debouncer.should_debounce(Path::new("/test/other.txt")));
    }
}