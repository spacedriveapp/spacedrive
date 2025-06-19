# File Watcher VDFS Integration Design

## Overview

This document outlines how the cross-platform file watcher integrates with the core-new Virtual Distributed File System (VDFS), leveraging the new Entry-centric data model and SdPath addressing system.

## Key Differences from Original Implementation

### Original Spacedrive Architecture
- **FilePath-centric**: Files were primarily `file_path` records with optional `object` links
- **Content-first**: Required content hashing for full functionality  
- **Prisma ORM**: Complex query patterns with extensive invalidation
- **Immediate indexing**: Heavy operations triggered on every file event

### Core-New Architecture  
- **Entry-centric**: Every file/directory is an `Entry` with mandatory `UserMetadata`
- **Metadata-first**: User metadata (tags, notes) available immediately
- **SeaORM**: Modern Rust ORM with better performance patterns
- **Progressive indexing**: Lightweight discovery → optional content indexing → deep analysis

## Integration Architecture

### 1. Event Flow Overview

```
File System Event → Platform Handler → Direct Database Operations → Event Bus
```

**Detailed Flow:**
1. **File system events** detected by platform-specific handlers (FSEvents, inotify, etc.)
2. **Platform handler** filters and processes events (debouncing, rename correlation)
3. **Direct database operations** immediately create/update Entry and UserMetadata records
4. **Event bus** notifies other systems of changes
5. **Background tasks** (spawned, not job system) handle heavy operations like thumbnails

**Key Principle**: Following the original implementation, file system events trigger **immediate database updates**, not job scheduling. This ensures real-time consistency between the file system and database state.

### 2. Database Operations by Event Type

#### CREATE Events

```rust
async fn handle_file_created(
    sd_path: SdPath, 
    library_id: Uuid,
    db: &DatabaseConnection
) -> Result<Entry> {
    // 1. Get filesystem metadata
    let metadata = tokio::fs::metadata(sd_path.as_local_path()?).await?;
    
    // 2. Check for existing Entry (handle duplicates/race conditions)
    if let Some(existing) = find_entry_by_sdpath(&sd_path, db).await? {
        return Ok(existing);
    }
    
    // 3. Create Entry record
    let entry_id = Uuid::new_v7();
    let metadata_id = Uuid::new_v7();
    
    let entry = entry::ActiveModel {
        id: Set(entry_id),
        uuid: Set(Uuid::new_v4()), // Public UUID for API
        device_id: Set(sd_path.device_id()),
        path: Set(sd_path.path().to_string()),
        library_id: Set(Some(library_id)),
        name: Set(sd_path.file_name().unwrap_or_default()),
        kind: Set(if metadata.is_dir() { 
            EntryKind::Directory 
        } else { 
            EntryKind::File { 
                extension: sd_path.extension().map(|s| s.to_string()) 
            } 
        }),
        size: Set(if metadata.is_dir() { None } else { Some(metadata.len()) }),
        created_at: Set(metadata.created().ok().map(|t| t.into())),
        modified_at: Set(metadata.modified().ok().map(|t| t.into())),
        metadata_id: Set(metadata_id),
        content_id: Set(None), // Will be set during indexing
        // ... other fields
    };
    
    // 4. Create UserMetadata record
    let user_metadata = user_metadata::ActiveModel {
        id: Set(metadata_id),
        tags: Set(vec![]),
        labels: Set(vec![]),
        notes: Set(None),
        favorite: Set(false),
        hidden: Set(false),
        // ... other fields
    };
    
    // 5. Insert both in transaction
    let txn = db.begin().await?;
    let entry = entry.insert(&txn).await?;
    user_metadata.insert(&txn).await?;
    txn.commit().await?;
    
    // 6. Generate content identity immediately (following original pattern)
    if should_index_content(&sd_path) {
        if let Ok(cas_id) = generate_cas_id(&sd_path).await {
            let content_identity = find_or_create_content_identity(cas_id, &txn).await?;
            
            // Link entry to content
            entry.content_id = Set(Some(content_identity.id));
            entry.save(&txn).await?;
            
            // Spawn background task for heavy operations (thumbnails, media extraction)
            let sd_path_clone = sd_path.clone();
            let entry_id = entry.id.clone();
            tokio::spawn(async move {
                if let Err(e) = generate_thumbnails(&sd_path_clone, entry_id).await {
                    tracing::warn!("Thumbnail generation failed: {}", e);
                }
                if let Err(e) = extract_media_metadata(&sd_path_clone, entry_id).await {
                    tracing::warn!("Media extraction failed: {}", e);
                }
            });
        }
    }
    
    Ok(entry)
}
```

#### MODIFY Events

```rust
async fn handle_file_modified(
    sd_path: SdPath,
    db: &DatabaseConnection
) -> Result<Option<Entry>> {
    // 1. Find existing Entry
    let entry = match find_entry_by_sdpath(&sd_path, db).await? {
        Some(entry) => entry,
        None => {
            // File was modified but we don't know about it yet
            // This can happen during rapid file operations
            return handle_file_created(sd_path, library_id, db).await.map(Some);
        }
    };
    
    // 2. Update basic metadata
    let metadata = tokio::fs::metadata(sd_path.as_local_path()?).await?;
    
    let mut active_entry: entry::ActiveModel = entry.into();
    active_entry.size = Set(if metadata.is_dir() { None } else { Some(metadata.len()) });
    active_entry.modified_at = Set(metadata.modified().ok().map(|t| t.into()));
    
    // 3. Handle content changes immediately
    if let Some(content_id) = entry.content_id {
        // File had content identity - check if content actually changed
        if let Ok(new_cas_id) = generate_cas_id(&sd_path).await {
            let old_content = get_content_identity(content_id, db).await?;
            if old_content.cas_id != new_cas_id {
                // Content changed - create or link to new content identity
                let new_content = find_or_create_content_identity(new_cas_id, db).await?;
                active_entry.content_id = Set(Some(new_content.id));
                
                // Update reference counts
                decrease_content_reference_count(content_id, db).await?;
                increase_content_reference_count(new_content.id, db).await?;
                
                // Spawn background task for re-generating thumbnails/media data
                let sd_path_clone = sd_path.clone();
                let entry_id = entry.id;
                tokio::spawn(async move {
                    let _ = regenerate_media_data(&sd_path_clone, entry_id).await;
                });
            }
        }
    } else if should_index_content(&sd_path) {
        // File didn't have content identity but should be indexed now
        if let Ok(cas_id) = generate_cas_id(&sd_path).await {
            let content_identity = find_or_create_content_identity(cas_id, db).await?;
            active_entry.content_id = Set(Some(content_identity.id));
        }
    }
    
    // 4. Update Entry
    let updated_entry = active_entry.update(db).await?;
    
    Ok(Some(updated_entry))
}
```

#### RENAME/MOVE Events

```rust
async fn handle_file_moved(
    old_path: SdPath,
    new_path: SdPath, 
    db: &DatabaseConnection
) -> Result<Option<Entry>> {
    // 1. Find existing Entry by old path
    let entry = find_entry_by_sdpath(&old_path, db).await?;
    
    let entry = match entry {
        Some(entry) => entry,
        None => {
            // Entry doesn't exist - treat as create
            return handle_file_created(new_path, library_id, db).await.map(Some);
        }
    };
    
    // 2. Update path information
    let mut active_entry: entry::ActiveModel = entry.into();
    active_entry.device_id = Set(new_path.device_id());
    active_entry.path = Set(new_path.path().to_string());
    active_entry.name = Set(new_path.file_name().unwrap_or_default());
    
    // Update extension if it changed
    if let EntryKind::File { extension } = &entry.kind {
        let new_extension = new_path.extension().map(|s| s.to_string());
        if extension != &new_extension {
            active_entry.kind = Set(EntryKind::File { extension: new_extension });
        }
    }
    
    // 3. Handle directory moves (update all children)
    if matches!(entry.kind, EntryKind::Directory) {
        update_child_paths_recursively(entry.id, &old_path, &new_path, db).await?;
    }
    
    // 4. Update parent relationship
    if let Some(parent_path) = new_path.parent() {
        if let Some(parent_entry) = find_entry_by_sdpath(&parent_path, db).await? {
            active_entry.parent_id = Set(Some(parent_entry.id));
        }
    }
    
    // 5. Update Entry
    let updated_entry = active_entry.update(db).await?;
    
    // Note: UserMetadata and ContentIdentity remain unchanged during moves
    // This preserves tags, notes, and deduplication relationships
    
    Ok(Some(updated_entry))
}
```

#### DELETE Events

```rust
async fn handle_file_deleted(
    sd_path: SdPath,
    db: &DatabaseConnection
) -> Result<()> {
    // 1. Find Entry
    let entry = match find_entry_by_sdpath(&sd_path, db).await? {
        Some(entry) => entry,
        None => return Ok(()), // Already deleted or never existed
    };
    
    // 2. Handle directory deletion (recursive)
    if matches!(entry.kind, EntryKind::Directory) {
        delete_children_recursively(entry.id, db).await?;
    }
    
    // 3. Check ContentIdentity reference count
    if let Some(content_id) = entry.content_id {
        decrease_content_reference_count(content_id, db).await?;
    }
    
    // 4. Delete Entry (UserMetadata is deleted via cascade)
    entry::Entity::delete_by_id(entry.id).execute(db).await?;
    
    Ok(())
}

async fn decrease_content_reference_count(
    content_id: Uuid,
    db: &DatabaseConnection
) -> Result<()> {
    // 1. Count remaining entries with this content
    let remaining_count = entry::Entity::find()
        .filter(entry::Column::ContentId.eq(content_id))
        .count(db)
        .await? as u32;
    
    // 2. Update ContentIdentity
    if remaining_count == 0 {
        // No more entries reference this content - delete it
        content_identity::Entity::delete_by_id(content_id).execute(db).await?;
    } else {
        // Update reference count
        let mut active_content: content_identity::ActiveModel = 
            content_identity::Entity::find_by_id(content_id)
                .one(db)
                .await?
                .unwrap()
                .into();
        
        active_content.entry_count = Set(remaining_count);
        active_content.update(db).await?;
    }
    
    Ok(())
}
```

### 3. Background Task Handling

Following the original approach, heavy operations are handled via spawned tasks, not the job system:

```rust
/// Generate thumbnails in background (original pattern)
async fn generate_thumbnails(sd_path: &SdPath, entry_id: Uuid) -> Result<()> {
    let file_path = sd_path.as_local_path()?;
    
    // Check if file is a supported media type
    if !is_thumbnail_supported(&file_path) {
        return Ok(());
    }
    
    // Generate thumbnail (this can be slow)
    let thumbnail_data = create_thumbnail(&file_path).await?;
    
    // Save thumbnail to storage
    let thumbnail_path = get_thumbnail_path(entry_id);
    save_thumbnail(thumbnail_path, thumbnail_data).await?;
    
    // Update entry with thumbnail info
    update_entry_thumbnail_info(entry_id, true).await?;
    
    Ok(())
}

/// Extract media metadata in background (original pattern)
async fn extract_media_metadata(sd_path: &SdPath, entry_id: Uuid) -> Result<()> {
    let file_path = sd_path.as_local_path()?;
    
    // Extract metadata based on file type
    let media_data = match get_file_type(&file_path) {
        FileType::Image => extract_exif_data(&file_path).await?,
        FileType::Video => extract_ffmpeg_metadata(&file_path).await?,
        FileType::Audio => extract_audio_metadata(&file_path).await?,
        _ => return Ok(()), // Not a media file
    };
    
    // Update content identity with media data
    update_content_media_data(entry_id, media_data).await?;
    
    Ok(())
}

/// Directory scanning - this one actually uses the job system like original
async fn spawn_directory_scan(location_id: Uuid, path: SdPath) {
    // Wait 1 second like original to avoid scanning rapidly changing directories
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Trigger location sub-path scan job (this part uses job system)
    if let Err(e) = trigger_location_scan_job(location_id, path).await {
        tracing::error!("Failed to trigger directory scan job: {}", e);
    }
}
```

### 4. Location Integration

File watchers operate within the context of indexed Locations:

```rust
impl LocationWatcher {
    async fn add_location_to_watcher(&self, location: &Location) -> Result<()> {
        let sd_path = SdPath::from_serialized(&location.device_id, &location.path)?;
        
        let watched_location = WatchedLocation {
            id: location.id,
            library_id: location.library_id,
            path: sd_path.as_local_path()?.to_path_buf(),
            enabled: location.watch_enabled,
            index_mode: location.index_mode,
        };
        
        self.add_location(watched_location).await?;
        
        // Emit event
        self.events.emit(Event::LocationWatchingStarted {
            library_id: location.library_id,
            location_id: location.id,
        });
        
        Ok(())
    }
}
```

### 5. Event Bus Integration

The watcher emits detailed events for real-time UI updates:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Existing events...
    
    // Enhanced file system events
    EntryCreated { 
        library_id: Uuid, 
        entry_id: Uuid,
        entry_uuid: Uuid,  // Public UUID for frontend
        sd_path: String,   // Serialized SdPath
        kind: EntryKind,
    },
    EntryModified { 
        library_id: Uuid, 
        entry_id: Uuid,
        entry_uuid: Uuid,
        changes: EntryChanges, // What specifically changed
    },
    EntryDeleted { 
        library_id: Uuid, 
        entry_id: Uuid,
        entry_uuid: Uuid,
        sd_path: String,   // Path before deletion
    },
    EntryMoved { 
        library_id: Uuid, 
        entry_id: Uuid,
        entry_uuid: Uuid,
        old_path: String, 
        new_path: String,
    },
    
    // Content indexing events
    ContentIndexingStarted { entry_id: Uuid },
    ContentIndexingCompleted { 
        entry_id: Uuid, 
        content_id: Option<Uuid>,  // None if no unique content found
        is_duplicate: bool,
    },
    ContentIndexingFailed { 
        entry_id: Uuid, 
        error: String 
    },
    
    // Location watching events
    LocationWatchingStarted { library_id: Uuid, location_id: Uuid },
    LocationWatchingPaused { library_id: Uuid, location_id: Uuid },
    LocationWatchingError { library_id: Uuid, location_id: Uuid, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryChanges {
    pub size_changed: bool,
    pub modified_time_changed: bool,
    pub content_changed: bool,
    pub metadata_updated: bool,
}
```

### 6. Error Handling and Resilience

```rust
impl WatcherDatabaseOperations {
    async fn handle_database_error(&self, error: DbErr, sd_path: &SdPath) -> Result<()> {
        match error {
            DbErr::RecordNotFound(_) => {
                // Entry doesn't exist - retry as creation
                self.handle_file_created(sd_path.clone()).await
            }
            DbErr::Exec(sqlx_error) if sqlx_error.to_string().contains("UNIQUE constraint") => {
                // Duplicate entry - this is okay, ignore
                Ok(())
            }
            _ => {
                // Other errors - emit error event and continue
                self.events.emit(Event::WatcherError {
                    location_id: self.location_id,
                    error: error.to_string(),
                    path: sd_path.to_string(),
                });
                Err(error.into())
            }
        }
    }
}
```

### 7. Performance Optimizations

#### Batch Operations
```rust
impl WatcherDatabaseOperations {
    async fn flush_pending_operations(&self) -> Result<()> {
        let pending = self.pending_operations.lock().await;
        
        if pending.is_empty() {
            return Ok(());
        }
        
        // Group operations by type for efficient batch processing
        let creates: Vec<_> = pending.iter().filter_map(|op| {
            if let PendingOperation::Create(path) = op { Some(path) } else { None }
        }).collect();
        
        let updates: Vec<_> = pending.iter().filter_map(|op| {
            if let PendingOperation::Update(id, changes) = op { Some((id, changes)) } else { None }
        }).collect();
        
        // Batch insert entries
        if !creates.is_empty() {
            self.batch_create_entries(creates).await?;
        }
        
        // Batch update entries
        if !updates.is_empty() {
            self.batch_update_entries(updates).await?;
        }
        
        Ok(())
    }
}
```

#### Debouncing Strategy
```rust
struct WatcherDebouncer {
    pending_events: HashMap<PathBuf, (WatcherEvent, Instant)>,
    debounce_duration: Duration,
}

impl WatcherDebouncer {
    async fn process_event(&mut self, event: WatcherEvent) -> Option<WatcherEvent> {
        let path = event.primary_path()?.clone();
        let now = Instant::now();
        
        // Check if we have a recent event for this path
        if let Some((_, last_time)) = self.pending_events.get(&path) {
            if now.duration_since(*last_time) < self.debounce_duration {
                // Update the event and reset timer
                self.pending_events.insert(path, (event, now));
                return None; // Event is debounced
            }
        }
        
        // Event should be processed
        self.pending_events.insert(path, (event.clone(), now));
        Some(event)
    }
}
```

## Benefits of Core-New Integration

### 1. **Immediate Database Consistency** 
- File system changes immediately reflected in database (like original)
- Entry + UserMetadata records created synchronously
- Content identity generated on-the-fly when possible
- Real-time consistency between file system and database state

### 2. **True VDFS Support**
- SdPath enables cross-device file operations
- UserMetadata survives file moves/renames
- ContentIdentity provides global deduplication
- Cross-device operations work seamlessly

### 3. **Separated Concerns**
- Core database operations happen immediately (critical path)
- Heavy operations (thumbnails, media extraction) spawn in background
- Directory scanning uses job system for complex indexing operations
- Performance-critical path remains fast and responsive

### 4. **Enhanced Reliability**
- Follows proven original architecture patterns
- Atomic database transactions prevent partial states
- Platform-specific optimizations for edge cases
- Graceful degradation when background tasks fail

### 5. **Better Performance**
- Direct database operations are faster than job overhead
- Smart debouncing prevents duplicate work
- Background tasks don't block file system event processing
- Event-driven architecture provides real-time UI updates

## Future Enhancements

### 1. **Conflict Resolution**
When the same file is modified on multiple devices:
```rust
async fn resolve_content_conflict(
    entry_a: &Entry, 
    entry_b: &Entry
) -> ConflictResolution {
    if entry_a.content_id == entry_b.content_id {
        return ConflictResolution::NoConflict;
    }
    
    // User choice, timestamp-based, or content-aware resolution
    ConflictResolution::UserChoice { 
        options: vec![entry_a.clone(), entry_b.clone()],
        suggested: suggest_resolution(entry_a, entry_b).await,
    }
}
```

### 2. **Smart Indexing**
Machine learning to predict which files should be indexed:
```rust
async fn should_index_content_ml(entry: &Entry) -> bool {
    let features = extract_features(entry);
    ml_model.predict(features).await > INDEXING_THRESHOLD
}
```

### 3. **Version History**
Track file content changes over time:
```rust
struct ContentVersion {
    id: Uuid,
    content_id: Uuid,
    cas_id: String,
    created_at: DateTime<Utc>,
    size: u64,
}
```

This design provides a robust foundation for real-time file system monitoring while maintaining the flexibility and performance characteristics of the core-new architecture.