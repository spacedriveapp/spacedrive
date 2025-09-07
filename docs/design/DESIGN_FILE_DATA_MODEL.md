# Spacedrive File Data Model Design (v2)

## Overview

This document describes a refreshed data model for Spacedrive that decouples user metadata from content deduplication, enabling a more flexible and powerful file management system.

## Core Principles

1. **Any file can have metadata** - Tagged files shouldn't require content indexing
2. **Content identity is optional** - Deduplication is a feature, not a requirement
3. **SdPath is the universal identifier** - Cross-device operations are first-class
4. **Graceful content changes** - Files evolve, the system should handle it
5. **Progressive enhancement** - Start simple, add richness over time

## Data Model

### 1. Entry (Replaces FilePath)

The `Entry` represents any filesystem entry (file or directory) that Spacedrive knows about.

```rust
struct Entry {
    id: Uuid,                    // Unique ID for this entry
    sd_path: SdPathSerialized,   // The virtual path (includes device)
    
    // Basic metadata (always available)
    name: String,
    kind: EntryKind,            // File, Directory, Symlink
    size: Option<u64>,          // None for directories
    created_at: Option<DateTime>,
    modified_at: Option<DateTime>,
    accessed_at: Option<DateTime>,
    
    // Platform-specific
    inode: Option<u64>,         // Unix/macOS
    file_id: Option<u64>,       // Windows
    
    // Relationships
    parent_id: Option<Uuid>,    // Parent directory Entry
    location_id: Option<Uuid>,  // If within an indexed location
    
    // User metadata holder
    metadata_id: Uuid,          // ALWAYS exists, links to UserMetadata
    
    // Content identity (optional)
    content_id: Option<Uuid>,   // Links to ContentIdentity if indexed
    
    // Tracking
    first_seen_at: DateTime,
    last_indexed_at: Option<DateTime>,
}

enum EntryKind {
    File { extension: Option<String> },
    Directory,
    Symlink { target: String },
}
```

### 2. UserMetadata (New!)

Decouples user-applied metadata from content identity. Every Entry has one.

```rust
struct UserMetadata {
    id: Uuid,
    
    // User-applied metadata
    tags: Vec<Tag>,
    labels: Vec<Label>,
    notes: Option<String>,
    favorite: bool,
    hidden: bool,
    
    // Custom fields (future)
    custom_fields: JsonValue,
    
    // Timestamps
    created_at: DateTime,
    updated_at: DateTime,
}
```

### 3. ContentIdentity (Replaces Object)

Represents unique content, used for deduplication. Only created when content is indexed.

```rust
struct ContentIdentity {
    id: Uuid,
    
    // Content addressing
    full_hash: Option<String>,      // Complete file hash (if computed)
    cas_id: String,                 // Sampled hash for quick comparison
    cas_version: u8,                // Version of the CAS algorithm used
    
    // Content metadata
    mime_type: Option<String>,
    kind: ObjectKind,               // Image, Video, Document, etc.
    
    // Extracted metadata (optional)
    media_data: Option<MediaData>,  // EXIF, video metadata, etc.
    text_content: Option<String>,   // For search indexing
    
    // Statistics
    total_size: u64,                // Combined size of all entries
    entry_count: u32,               // Number of entries with this content
    
    // Timestamps
    first_seen_at: DateTime,
    last_verified_at: DateTime,
}
```

### 4. SdPathSerialized

How SdPath is stored in the database:

```rust
struct SdPathSerialized {
    device_id: Uuid,
    path: String,               // Normalized path
}

// In the database, this could be:
// - Two columns: device_id, path
// - Or JSON: {"device_id": "...", "path": "..."}
// - Or a custom format: "device_id://path"
```

### 5. Location (Enhanced)

Indexed directories with richer functionality:

```rust
struct Location {
    id: Uuid,
    sd_path: SdPathSerialized,     // Root path of this location
    
    name: String,
    
    // Indexing configuration
    index_mode: IndexMode,
    scan_interval: Option<Duration>,
    
    // Statistics
    total_size: u64,
    file_count: u64,
    
    // State
    last_scan_at: Option<DateTime>,
    scan_state: ScanState,
}

enum IndexMode {
    Shallow,      // Just filesystem metadata
    Content,      // Generate cas_id for deduplication
    Deep,         // Extract text, generate thumbnails, etc.
}
```

## Key Processes

### 1. Initial File Discovery

When Spacedrive encounters a file (indexed or ephemeral):

```rust
async fn discover_entry(sd_path: SdPath) -> Entry {
    // 1. Create Entry with basic metadata
    let metadata = fs::metadata(&sd_path).await?;
    let entry = Entry {
        id: Uuid::new_v7(),
        sd_path: sd_path.serialize(),
        name: sd_path.file_name(),
        size: metadata.len(),
        // ... other metadata
        
        // Always create UserMetadata
        metadata_id: Uuid::new_v7(),
        
        // No content_id yet
        content_id: None,
    };
    
    // 2. Create empty UserMetadata
    let user_metadata = UserMetadata {
        id: entry.metadata_id,
        tags: vec![],
        // ... defaults
    };
    
    // 3. Save both
    save_entry(entry).await?;
    save_user_metadata(user_metadata).await?;
    
    entry
}
```

### 2. Content Indexing (Progressive)

Content indexing happens separately and progressively:

```rust
async fn index_entry_content(entry: &Entry, mode: IndexMode) -> Option<ContentIdentity> {
    match mode {
        IndexMode::Shallow => None, // No content indexing
        
        IndexMode::Content => {
            // Generate cas_id
            let cas_id = generate_cas_id(&entry.sd_path).await?;
            
            // Find or create ContentIdentity
            let content = find_or_create_content_identity(cas_id).await?;
            
            // Link entry to content
            update_entry_content_id(entry.id, content.id).await?;
            
            Some(content)
        }
        
        IndexMode::Deep => {
            // Same as Content, plus:
            // - Extract text for search
            // - Generate thumbnails
            // - Extract media metadata
        }
    }
}
```

### 3. CAS ID Generation (Enhanced)

The new CAS algorithm is versioned for future improvements:

```rust
const CAS_VERSION: u8 = 2;

async fn generate_cas_id(sd_path: &SdPath) -> Result<String> {
    // For remote files, request CAS from that device
    if !sd_path.is_local() {
        return request_remote_cas_id(sd_path).await;
    }
    
    let file = File::open(sd_path.as_local_path().unwrap()).await?;
    let size = file.metadata().await?.len();
    
    let mut hasher = Blake3Hasher::new();
    
    // Version prefix for algorithm changes
    hasher.update(&[CAS_VERSION]);
    hasher.update(&size.to_le_bytes());
    
    if size <= SMALL_FILE_THRESHOLD {
        // Hash entire file
        hash_entire_file(&mut hasher, file).await?;
    } else {
        // Sample-based hashing
        hash_file_samples(&mut hasher, file, size).await?;
    }
    
    Ok(format!("v{}:{}", CAS_VERSION, hasher.finalize().to_hex()[..16]))
}
```

### 4. Handling Content Changes

When a file's content changes:

```rust
async fn handle_file_modified(entry: &Entry) {
    // User metadata is unaffected - tags, notes, etc. remain
    
    if let Some(old_content_id) = entry.content_id {
        // Generate new CAS ID
        let new_cas_id = generate_cas_id(&entry.sd_path).await?;
        
        // Check if content actually changed
        let old_content = get_content_identity(old_content_id).await?;
        if old_content.cas_id == new_cas_id {
            return; // No actual change
        }
        
        // Find or create new content identity
        let new_content = find_or_create_content_identity(new_cas_id).await?;
        
        // Update entry link
        update_entry_content_id(entry.id, new_content.id).await?;
        
        // Decrease old content's entry count
        decrease_content_entry_count(old_content_id).await?;
    }
}
```

### 5. Cross-Device Operations

With SdPath integration:

```rust
async fn copy_with_metadata(source: SdPath, dest: SdPath) -> Result<Entry> {
    // 1. Copy the actual file content
    let copy_result = copy_file_content(&source, &dest).await?;
    
    // 2. Get source entry and metadata
    let source_entry = get_entry_by_sdpath(&source).await?;
    let source_metadata = get_user_metadata(source_entry.metadata_id).await?;
    
    // 3. Create destination entry
    let dest_entry = discover_entry(dest).await?;
    
    // 4. Copy user metadata (tags, notes, etc.)
    copy_user_metadata(&source_metadata, dest_entry.metadata_id).await?;
    
    // 5. If source had content identity, schedule indexing for dest
    if source_entry.content_id.is_some() {
        schedule_content_indexing(dest_entry.id).await?;
    }
    
    Ok(dest_entry)
}
```

## Benefits of This Design

### 1. Flexible Metadata
- Any file can be tagged immediately, even non-indexed
- Ephemeral files have full metadata support
- No need to wait for content indexing

### 2. Graceful Content Evolution
- File edits don't lose tags/notes
- Content identity tracks uniqueness when available
- Version history could be added later

### 3. Progressive Enhancement
- Start with just filesystem metadata
- Add content identity when needed
- Deep indexing (text extraction, etc.) is optional

### 4. SdPath Integration
- Entries are naturally cross-device via SdPath
- Operations work uniformly across devices
- Virtual filesystem is truly realized

### 5. Better Performance
- No forced content reading for basic operations
- Metadata operations are lightweight
- Content indexing can be batched/scheduled

## Migration from v1

```sql
-- Rough migration approach
-- 1. Create UserMetadata for each Object
INSERT INTO user_metadata (id, tags, labels, notes, favorite, hidden)
SELECT 
    uuid_v7() as id,
    -- Extract tags via junction table
    -- Extract other metadata
FROM object;

-- 2. Transform FilePath to Entry
INSERT INTO entry (id, device_id, path, metadata_id, content_id)
SELECT 
    uuid_v7() as id,
    COALESCE(device_id, current_device_id()) as device_id,
    path,
    -- Link to migrated UserMetadata
    -- Link to ContentIdentity (migrated from Object)
FROM file_path;

-- 3. Transform Object to ContentIdentity
INSERT INTO content_identity (id, cas_id, kind, ...)
SELECT 
    pub_id as id,
    -- Derive cas_id from linked file_paths
    kind,
    ...
FROM object;
```

## Future Considerations

1. **Content Versions**: Track history of content changes
2. **Metadata Sync**: Efficient sync of UserMetadata across devices
3. **Virtual Entries**: Entries that don't exist locally but we know about
4. **Cloud Integration**: Treat cloud storage as just another device
5. **Conflict Resolution**: When same file has different metadata on different devices