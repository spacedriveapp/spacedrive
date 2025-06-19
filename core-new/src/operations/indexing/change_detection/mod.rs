//! Change detection for incremental indexing
//! 
//! This module provides efficient change detection using:
//! - Inode tracking for move/rename detection
//! - Modification time comparison
//! - Size verification
//! - Directory hierarchy tracking

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
    collections::HashMap,
};
use crate::infrastructure::{
    database::entities,
    jobs::prelude::JobContext,
};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QuerySelect};
use super::state::EntryKind;

/// Represents a change detected in the file system
#[derive(Debug, Clone)]
pub enum Change {
    /// New file/directory not in database
    New(PathBuf),
    
    /// File/directory modified (content or metadata changed)
    Modified {
        path: PathBuf,
        entry_id: i32,
        old_modified: Option<SystemTime>,
        new_modified: Option<SystemTime>,
    },
    
    /// File/directory moved or renamed (same inode, different path)
    Moved {
        old_path: PathBuf,
        new_path: PathBuf,
        entry_id: i32,
        inode: u64,
    },
    
    /// File/directory deleted (exists in DB but not on disk)
    Deleted {
        path: PathBuf,
        entry_id: i32,
    },
}

/// Tracks changes between database state and file system
pub struct ChangeDetector {
    /// Maps paths to their database entries
    path_to_entry: HashMap<PathBuf, DatabaseEntry>,
    
    /// Maps inodes to paths (for detecting moves)
    inode_to_path: HashMap<u64, PathBuf>,
    
    /// Precision for timestamp comparison (some filesystems have lower precision)
    timestamp_precision_ms: i64,
}

#[derive(Debug, Clone)]
struct DatabaseEntry {
    id: i32,
    path: PathBuf,
    kind: EntryKind,
    size: u64,
    modified: Option<SystemTime>,
    inode: Option<u64>,
}

impl ChangeDetector {
    /// Create a new change detector
    pub fn new() -> Self {
        Self {
            path_to_entry: HashMap::new(),
            inode_to_path: HashMap::new(),
            timestamp_precision_ms: 1, // Default to 1ms precision
        }
    }
    
    /// Load existing entries from database for a location
    pub async fn load_existing_entries(
        &mut self,
        ctx: &JobContext<'_>,
        location_id: i32,
        location_root: &Path,
    ) -> Result<(), crate::infrastructure::jobs::prelude::JobError> {
        use crate::infrastructure::jobs::prelude::JobError;
        
        // Query all entries for this location
        let entries = entities::entry::Entity::find()
            .filter(entities::entry::Column::LocationId.eq(location_id))
            .select_only()
            .column(entities::entry::Column::Id)
            .column(entities::entry::Column::RelativePath)
            .column(entities::entry::Column::Name)
            .column(entities::entry::Column::Extension)
            .column(entities::entry::Column::Kind)
            .column(entities::entry::Column::Size)
            .column(entities::entry::Column::ModifiedAt)
            .column(entities::entry::Column::Inode)
            .into_tuple::<(i32, String, String, Option<String>, i32, i64, chrono::DateTime<chrono::Utc>, Option<i64>)>()
            .all(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to load existing entries: {}", e)))?;
        
        // Process entries
        for (id, relative_path, name, extension, kind, size, modified, inode) in entries {
            // Reconstruct full path
            let mut full_path = location_root.to_path_buf();
            if !relative_path.is_empty() {
                full_path.push(&relative_path);
            }
            
            // Add filename with extension
            let filename = if let Some(ext) = extension {
                format!("{}.{}", name, ext)
            } else {
                name
            };
            full_path.push(filename);
            
            // Convert types
            let entry_kind = match kind {
                0 => EntryKind::File,
                1 => EntryKind::Directory,
                2 => EntryKind::Symlink,
                _ => continue, // Skip unknown types
            };
            
            let modified_time = SystemTime::UNIX_EPOCH + 
                std::time::Duration::from_secs(modified.timestamp() as u64);
            
            let db_entry = DatabaseEntry {
                id,
                path: full_path.clone(),
                kind: entry_kind,
                size: size as u64,
                modified: Some(modified_time),
                inode: inode.map(|i| i as u64),
            };
            
            // Track by path
            self.path_to_entry.insert(full_path.clone(), db_entry);
            
            // Track by inode if available
            if let Some(inode_val) = inode {
                self.inode_to_path.insert(inode_val as u64, full_path);
            }
        }
        
        ctx.log(format!("Loaded {} existing entries for change detection", self.path_to_entry.len()));
        
        Ok(())
    }
    
    /// Check if a path represents a change
    pub fn check_path(
        &self,
        path: &Path,
        metadata: &std::fs::Metadata,
        inode: Option<u64>,
    ) -> Option<Change> {
        // Check if path exists in database
        if let Some(db_entry) = self.path_to_entry.get(path) {
            // Check for modifications
            if self.is_modified(db_entry, metadata) {
                return Some(Change::Modified {
                    path: path.to_path_buf(),
                    entry_id: db_entry.id,
                    old_modified: db_entry.modified,
                    new_modified: metadata.modified().ok(),
                });
            }
            
            // No change for this path
            return None;
        }
        
        // Path not in database - check if it's a move
        if let Some(inode_val) = inode {
            if let Some(old_path) = self.inode_to_path.get(&inode_val) {
                if old_path != path {
                    // Same inode, different path - it's a move
                    if let Some(db_entry) = self.path_to_entry.get(old_path) {
                        return Some(Change::Moved {
                            old_path: old_path.clone(),
                            new_path: path.to_path_buf(),
                            entry_id: db_entry.id,
                            inode: inode_val,
                        });
                    }
                }
            }
        }
        
        // New file/directory
        Some(Change::New(path.to_path_buf()))
    }
    
    /// Find deleted entries (in DB but not seen during scan)
    pub fn find_deleted(&self, seen_paths: &std::collections::HashSet<PathBuf>) -> Vec<Change> {
        self.path_to_entry
            .iter()
            .filter(|(path, _)| !seen_paths.contains(*path))
            .map(|(path, entry)| Change::Deleted {
                path: path.clone(),
                entry_id: entry.id,
            })
            .collect()
    }
    
    /// Check if an entry has been modified
    fn is_modified(&self, db_entry: &DatabaseEntry, metadata: &std::fs::Metadata) -> bool {
        // Check size first (fast)
        if db_entry.size != metadata.len() {
            return true;
        }
        
        // Check modification time
        if let (Some(db_modified), Ok(fs_modified)) = (db_entry.modified, metadata.modified()) {
            // Compare with precision tolerance
            let db_time = db_modified.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let fs_time = fs_modified.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            
            if (db_time - fs_time).abs() > self.timestamp_precision_ms {
                return true;
            }
        }
        
        false
    }
    
    /// Set timestamp precision for comparison (in milliseconds)
    pub fn set_timestamp_precision(&mut self, precision_ms: i64) {
        self.timestamp_precision_ms = precision_ms;
    }
    
    /// Get the number of tracked entries
    pub fn entry_count(&self) -> usize {
        self.path_to_entry.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_change_detection() {
        let mut detector = ChangeDetector::new();
        
        // Add a test entry
        let path = PathBuf::from("/test/file.txt");
        let db_entry = DatabaseEntry {
            id: 1,
            path: path.clone(),
            kind: EntryKind::File,
            size: 1000,
            modified: Some(SystemTime::now()),
            inode: Some(12345),
        };
        
        detector.path_to_entry.insert(path.clone(), db_entry);
        detector.inode_to_path.insert(12345, path.clone());
        
        // Test new file detection
        let new_path = PathBuf::from("/test/new_file.txt");
        let metadata = std::fs::Metadata::default(); // Would use real metadata in practice
        
        match detector.check_path(&new_path, &metadata, None) {
            Some(Change::New(p)) => assert_eq!(p, new_path),
            _ => panic!("Expected new file detection"),
        }
    }
}