//! Entry processing and metadata extraction

use crate::{
    infrastructure::{
        database::entities::{self},
        jobs::prelude::{JobContext, JobError},
    },
    domain::content_identity::{CasGenerator, ContentKind},
    file_type::{FileTypeRegistry, IdentificationMethod},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryFilter, ColumnTrait};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use super::state::{DirEntry, EntryKind, IndexerState};

/// Metadata about a file system entry
#[derive(Debug, Clone)]
pub struct EntryMetadata {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub accessed: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
    pub inode: Option<u64>,
    pub permissions: Option<u32>,
    pub is_hidden: bool,
}

impl From<DirEntry> for EntryMetadata {
    fn from(entry: DirEntry) -> Self {
        Self {
            path: entry.path.clone(),
            kind: entry.kind,
            size: entry.size,
            modified: entry.modified,
            accessed: None,
            created: None,
            inode: entry.inode,
            permissions: None,
            is_hidden: entry.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false),
        }
    }
}

/// Handles entry creation and updates in the database
pub struct EntryProcessor;

impl EntryProcessor {
    /// Get the parent entry ID for a given path
    async fn get_parent_id(
        state: &mut IndexerState,
        ctx: &JobContext<'_>,
        path: &Path,
        location_id: i32,
        location_root_path: &Path,
    ) -> Result<Option<i32>, JobError> {
        // Get parent path
        let parent_path = match path.parent() {
            Some(p) => p,
            None => return Ok(None), // Root has no parent
        };
        
        // If parent is the location root itself, no parent entry
        if parent_path == location_root_path {
            return Ok(None);
        }
        
        // Check cache first
        if let Some(&parent_id) = state.entry_id_cache.get(parent_path) {
            return Ok(Some(parent_id));
        }
        
        // Calculate parent's relative path from location root
        let parent_relative_path = if let Ok(rel_path) = parent_path.strip_prefix(location_root_path) {
            if let Some(parent_parent) = rel_path.parent() {
                if parent_parent == std::path::Path::new("") {
                    String::new()
                } else {
                    parent_parent.to_string_lossy().to_string()
                }
            } else {
                String::new()
            }
        } else {
            return Ok(None);
        };
        
        // Get parent name
        let parent_name = parent_path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Query database for parent entry by matching relative path and name
        let parent_entry = entities::entry::Entity::find()
            .filter(entities::entry::Column::LocationId.eq(location_id))
            .filter(entities::entry::Column::Kind.eq(1)) // Directory
            .filter(entities::entry::Column::RelativePath.eq(&parent_relative_path))
            .filter(entities::entry::Column::Name.eq(&parent_name))
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to query parent entry: {}", e)))?;
        
        if let Some(parent) = parent_entry {
            // Cache for future lookups
            state.entry_id_cache.insert(parent_path.to_path_buf(), parent.id);
            tracing::debug!("Found parent entry {} for path {}", parent.id, path.display());
            Ok(Some(parent.id))
        } else {
            tracing::debug!("No parent entry found for path {} (parent_path: {}, relative: {}, name: {})", 
                path.display(), parent_path.display(), parent_relative_path, parent_name);
            Ok(None)
        }
    }
    /// Get platform-specific inode
    #[cfg(unix)]
    pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.ino())
    }
    
    #[cfg(windows)]
    pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
        // Windows doesn't have inodes, but we can use file index
        use std::os::windows::fs::MetadataExt;
        Some(metadata.file_index().unwrap_or(0))
    }
    
    #[cfg(not(any(unix, windows)))]
    pub fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
        None
    }
    
    /// Extract detailed metadata from a path
    pub async fn extract_metadata(path: &Path) -> Result<EntryMetadata, std::io::Error> {
        let metadata = tokio::fs::metadata(path).await?;
        
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_symlink() {
            EntryKind::Symlink
        } else {
            EntryKind::File
        };
        
        let inode = Self::get_inode(&metadata);
        
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::MetadataExt;
            Some(metadata.mode())
        };
        
        #[cfg(not(unix))]
        let permissions = None;
        
        Ok(EntryMetadata {
            path: path.to_path_buf(),
            kind,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
            created: metadata.created().ok(),
            inode,
            permissions,
            is_hidden: path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false),
        })
    }
    
    /// Get or create a path prefix for efficient storage
    pub async fn get_or_create_location_prefix(
        state: &mut IndexerState,
        ctx: &JobContext<'_>,
        device_id: i32,
        location_root_path: &Path,
    ) -> Result<i32, JobError> {
        let root_path_str = location_root_path.to_string_lossy().to_string();
        
        // Check cache first
        if let Some(&prefix_id) = state.path_prefix_cache.get(&root_path_str) {
            return Ok(prefix_id);
        }
        
        // Look up existing prefix in database
        let existing = entities::path_prefix::Entity::find()
            .filter(entities::path_prefix::Column::DeviceId.eq(device_id))
            .filter(entities::path_prefix::Column::Prefix.eq(&root_path_str))
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to query path prefix: {}", e)))?;
        
        let prefix_id = if let Some(existing) = existing {
            existing.id
        } else {
            // Create new prefix for the location root
            let new_prefix = entities::path_prefix::ActiveModel {
                device_id: Set(device_id),
                prefix: Set(root_path_str.clone()),
                created_at: Set(chrono::Utc::now()),
                ..Default::default()
            };
            
            let result = new_prefix.insert(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to create location path prefix: {}", e)))?;
            
            result.id
        };
        
        // Cache for future use
        state.path_prefix_cache.insert(root_path_str, prefix_id);
        
        Ok(prefix_id)
    }
    
    /// Create an entry record in the database
    pub async fn create_entry(
        state: &mut IndexerState,
        ctx: &JobContext<'_>,
        entry: &DirEntry,
        location_id: i32,
        device_id: i32,
        location_root_path: &Path,
    ) -> Result<i32, JobError> {
        // Calculate relative directory path from location root (without filename)
        let relative_path = if let Ok(rel_path) = entry.path.strip_prefix(location_root_path) {
            // Get parent directory relative to location root
            if let Some(parent) = rel_path.parent() {
                if parent == std::path::Path::new("") {
                    String::new()
                } else {
                    parent.to_string_lossy().to_string()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        let prefix_id = Self::get_or_create_location_prefix(
            state, ctx, device_id, location_root_path
        ).await?;
        
        // Extract file extension (without dot) for files, None for directories
        let extension = match entry.kind {
            EntryKind::File => {
                entry.path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase())
            }
            EntryKind::Directory | EntryKind::Symlink => None,
        };
        
        // Get file name without extension (stem)
        let name = entry.path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                entry.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            });
        
        // Convert timestamps
        let modified_at = entry.modified
            .and_then(|t| chrono::DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64, 0
            ))
            .unwrap_or_else(|| chrono::Utc::now());
        
        // Get parent ID
        let parent_id = Self::get_parent_id(state, ctx, &entry.path, location_id, location_root_path)
            .await
            .ok()
            .flatten();
        
        if let Some(pid) = parent_id {
            tracing::debug!("Setting parent_id={} for entry {}", pid, entry.path.display());
        }
        
        // Create entry
        let new_entry = entities::entry::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            prefix_id: Set(prefix_id),
            relative_path: Set(relative_path),
            name: Set(name),
            kind: Set(Self::entry_kind_to_int(entry.kind)),
            extension: Set(extension),
            metadata_id: Set(None), // User metadata only created when user adds metadata
            content_id: Set(None), // Will be set later if content indexing is enabled
            location_id: Set(Some(location_id)),
            parent_id: Set(parent_id),
            size: Set(entry.size as i64),
            aggregate_size: Set(0), // Will be calculated in aggregation phase
            child_count: Set(0), // Will be calculated in aggregation phase
            file_count: Set(0), // Will be calculated in aggregation phase
            created_at: Set(chrono::Utc::now()),
            modified_at: Set(modified_at),
            accessed_at: Set(None),
            permissions: Set(None), // TODO: Could extract from metadata
            inode: Set(entry.inode.map(|i| i as i64)),
            ..Default::default()
        };
        
        let result = new_entry.insert(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;
        
        // Cache the entry ID for potential children
        state.entry_id_cache.insert(entry.path.clone(), result.id);
        
        Ok(result.id)
    }
    
    /// Update an existing entry
    pub async fn update_entry(
        ctx: &JobContext<'_>,
        entry_id: i32,
        entry: &DirEntry,
    ) -> Result<(), JobError> {
        let db_entry = entities::entry::Entity::find_by_id(entry_id)
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
            .ok_or_else(|| JobError::execution("Entry not found for update".to_string()))?;
        
        let mut entry_active: entities::entry::ActiveModel = db_entry.into();
        
        // Update modifiable fields
        entry_active.size = Set(entry.size as i64);
        if let Some(modified) = entry.modified {
            if let Some(timestamp) = chrono::DateTime::from_timestamp(
                modified.duration_since(std::time::UNIX_EPOCH).ok()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0), 0
            ) {
                entry_active.modified_at = Set(timestamp);
            }
        }
        
        if let Some(inode) = entry.inode {
            entry_active.inode = Set(Some(inode as i64));
        }
        
        entry_active.update(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;
        
        Ok(())
    }
    
    /// Convert EntryKind to integer for database storage
    pub fn entry_kind_to_int(kind: EntryKind) -> i32 {
        match kind {
            EntryKind::File => 0,
            EntryKind::Directory => 1, 
            EntryKind::Symlink => 2,
        }
    }
    
    /// Create or find content identity and link to entry
    pub async fn create_content_identity(
        ctx: &JobContext<'_>,
        entry_id: i32,
        path: &Path,
        cas_id: String,
    ) -> Result<(), JobError> {
        // Check if content identity already exists
        let existing = entities::content_identity::Entity::find()
            .filter(entities::content_identity::Column::CasId.eq(&cas_id))
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to query content identity: {}", e)))?;
        
        let content_id = if let Some(existing) = existing {
            // Update entry count and last verification
            let existing_id = existing.id;
            let mut existing_active: entities::content_identity::ActiveModel = existing.into();
            existing_active.entry_count = Set(existing_active.entry_count.unwrap() + 1);
            existing_active.last_verified_at = Set(chrono::Utc::now());
            
            existing_active.update(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to update content identity: {}", e)))?;
            
            existing_id
        } else {
            // Create new content identity
            let file_size = tokio::fs::metadata(path).await
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            
            // Detect file type using the file type registry
            let registry = FileTypeRegistry::default();
            let file_type_result = registry.identify(path).await;
            
            let (kind_id, mime_type_id) = match file_type_result {
                Ok(result) => {
                    // Get content kind ID directly from the enum
                    let kind_id = result.file_type.category as i32;
                    
                    // Handle MIME type - upsert if found
                    let mime_type_id = if let Some(mime_str) = result.file_type.primary_mime_type() {
                        // Check if MIME type already exists
                        let existing = entities::mime_type::Entity::find()
                            .filter(entities::mime_type::Column::MimeType.eq(mime_str))
                            .one(ctx.library_db())
                            .await
                            .map_err(|e| JobError::execution(format!("Failed to query mime type: {}", e)))?;
                        
                        match existing {
                            Some(mime_record) => Some(mime_record.id),
                            None => {
                                // Create new MIME type entry
                                let new_mime = entities::mime_type::ActiveModel {
                                    uuid: Set(Uuid::new_v4()),
                                    mime_type: Set(mime_str.to_string()),
                                    created_at: Set(chrono::Utc::now()),
                                    ..Default::default()
                                };
                                
                                let mime_result = new_mime.insert(ctx.library_db()).await
                                    .map_err(|e| JobError::execution(format!("Failed to create mime type: {}", e)))?;
                                
                                Some(mime_result.id)
                            }
                        }
                    } else {
                        None
                    };
                    
                    (kind_id, mime_type_id)
                }
                Err(_) => {
                    // If identification fails, fall back to "unknown" (0)
                    (0, None)
                }
            };
            
            let new_content = entities::content_identity::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                full_hash: Set(None), // Could implement full hash later
                cas_id: Set(cas_id),
                cas_version: Set(1), // CAS version
                mime_type_id: Set(mime_type_id),
                kind_id: Set(kind_id),
                media_data: Set(None), // TODO: Extract media metadata
                text_content: Set(None), // TODO: Extract text content for indexing
                total_size: Set(file_size),
                entry_count: Set(1),
                first_seen_at: Set(chrono::Utc::now()),
                last_verified_at: Set(chrono::Utc::now()),
                ..Default::default()
            };
            
            let result = new_content.insert(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to create content identity: {}", e)))?;
            
            result.id
        };
        
        // Update entry to link to content identity
        let entry = entities::entry::Entity::find_by_id(entry_id)
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
            .ok_or_else(|| JobError::execution("Entry not found after creation".to_string()))?;
        
        let mut entry_active: entities::entry::ActiveModel = entry.into();
        entry_active.content_id = Set(Some(content_id));
        
        entry_active.update(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to link content identity to entry: {}", e)))?;
        
        Ok(())
    }
}