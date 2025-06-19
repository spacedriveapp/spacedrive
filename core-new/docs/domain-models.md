# Domain Models

The core domain models represent Spacedrive's unified approach to file management. These models implement the new file data model design where every file/directory has immediate metadata capabilities.

## Entry - The Universal File/Directory Model

The `Entry` is the central concept - everything (files, directories) is represented uniformly:

```rust
pub struct Entry {
    pub id: i32,                    // Database primary key
    pub uuid: Uuid,                 // Global unique identifier
    pub prefix_id: i32,             // Path compression reference
    pub relative_path: String,      // Path relative to prefix
    pub name: String,               // Display name
    pub kind: EntryKind,            // File or Directory
    pub metadata_id: i32,           // Always present - immediate tagging!
    pub content_id: Option<i32>,    // Optional - for deduplication
    pub location_id: Option<i32>,   // Optional - for organized files
    pub relative_path: String,        // Materialized path for hierarchy
    pub size: u64,                  // Size in bytes
    pub permissions: Option<String>, // File system permissions
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub accessed_at: Option<DateTime<Utc>>,
}

pub enum EntryKind {
    File,
    Directory,
}
```

### Key Design Decisions

**1. Always-Present Metadata**
- Every entry gets a `metadata_id` immediately upon creation
- No more "can't tag this file until it's indexed" problems
- Instant organization capabilities

**2. Optional Content Identity**
- `content_id` is only populated when content analysis is performed
- Allows immediate file operations without waiting for hashing
- Enables efficient deduplication when desired

**3. Flexible Relationships**
- `relative_path` provides hierarchy through materialized paths
- `location_id` links to organized collections
- Can represent the same physical file in multiple organizational contexts

### Path Optimization

The path storage system dramatically reduces database size:

```rust
// PathPrefix table stores common prefixes once
pub struct PathPrefix {
    pub id: i32,
    pub device_id: i32,
    pub prefix: String,  // e.g., "/Users/james/Documents"
}

// Entry stores only the unique portion
pub struct Entry {
    pub prefix_id: i32,      // References PathPrefix
    pub relative_path: String, // e.g., "photos/vacation.jpg"
    // ...
}
```

**Benefits:**
- **70%+ space savings** for large file collections
- **Faster queries** on path-based searches
- **Easier path manipulation** and normalization

## UserMetadata - Immediate Organization

Every entry has associated metadata for instant tagging and organization:

```rust
pub struct UserMetadata {
    pub id: i32,
    pub uuid: Uuid,
    pub notes: Option<String>,      // User notes
    pub favorite: bool,             // Favorite status
    pub hidden: bool,               // Hidden from normal views
    pub custom_data: Value,         // JSON for extensibility
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Tag System

Flexible tagging with many-to-many relationships:

```rust
pub struct Tag {
    pub id: i32,
    pub uuid: Uuid,
    pub name: String,               // "Work", "Personal", "Important"
    pub color: Option<String>,      // Hex color code
    pub icon: Option<String>,       // Icon identifier
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Junction table for many-to-many relationship
pub struct MetadataTag {
    pub metadata_id: i32,
    pub tag_id: i32,
}
```

### Label System

Hierarchical organization with labels:

```rust
pub struct Label {
    pub id: i32,
    pub uuid: Uuid,
    pub name: String,               // "Projects/Web Development"
    pub color: Option<String>,
    // Hierarchy determined by relative_path
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Junction table for labels
pub struct MetadataLabel {
    pub metadata_id: i32,
    pub label_id: i32,
}
```

## ContentIdentity - Deduplication

Optional content-based identity for deduplication and content analysis:

```rust
pub struct ContentIdentity {
    pub id: i32,
    pub cas_id: String,             // Content-addressable storage ID
    pub kind: ContentKind,          // Type of content
    pub size_bytes: u64,           // Actual content size
    pub media_data: Option<Value>, // Media-specific metadata (JSON)
    pub created_at: DateTime<Utc>,
}

pub enum ContentKind {
    Image,
    Video, 
    Audio,
    Document,
    Archive,
    Executable,
    Other,
}
```

### Content Addressing

The `cas_id` uses deterministic hashing for deduplication:

```rust
// Example CAS ID generation
pub fn generate_cas_id(content: &[u8]) -> String {
    let hash = blake3::hash(content);
    hash.to_hex().to_string()
}
```

**Benefits:**
- **Bit-level deduplication** - Identical content shares storage
- **Content verification** - Detect corruption or modification
- **Fast comparison** - Compare hashes instead of file contents

### Media Data

Rich metadata extraction for media files:

```rust
// Example media data structure
{
    "image": {
        "width": 1920,
        "height": 1080,
        "format": "JPEG",
        "camera": {
            "make": "Canon",
            "model": "EOS R5",
            "lens": "RF 24-70mm F2.8 L IS USM"
        },
        "location": {
            "latitude": 40.7128,
            "longitude": -74.0060,
            "altitude": 10.5
        }
    }
}
```

## Location - Organized Collections

Locations represent indexed directories with scanning capabilities:

```rust
pub struct Location {
    pub id: i32,
    pub uuid: Uuid,
    pub device_id: i32,             // Device that owns this location
    pub path: String,               // Absolute path
    pub name: Option<String>,       // Display name
    pub index_mode: IndexMode,      // How to scan this location
    pub scan_state: ScanState,      // Current scanning status
    pub last_scan_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub total_file_count: i64,      // Statistics
    pub total_byte_size: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum IndexMode {
    Metadata,    // Index file metadata only
    Content,     // Index metadata + generate content hashes
    Deep,        // Full content analysis + media metadata
}

pub enum ScanState {
    Pending,     // Not yet scanned
    Scanning,    // Currently scanning
    Complete,    // Scan completed successfully
    Error,       // Scan failed
    Paused,      // Scan paused by user
}
```

### Indexing Modes

**Metadata Mode:**
- Fast scanning
- File names, sizes, timestamps
- No content hashing
- Suitable for frequently changing directories

**Content Mode:**
- Moderate speed
- Includes content hashing for deduplication
- CAS ID generation
- Good balance of features and performance

**Deep Mode:**
- Comprehensive analysis
- Media metadata extraction
- Thumbnail generation
- Full-text content indexing (future)
- Best for media libraries and archives

## Device - Unified Identity

Single concept for device identity (replacing Node/Device/Instance confusion):

```rust
pub struct Device {
    pub id: i32,                    // Database primary key
    pub uuid: Uuid,                 // Global device identifier
    pub name: String,               // User-friendly name
    pub os: String,                 // Operating system
    pub os_version: Option<String>, // OS version details
    pub hardware_model: String,     // Hardware identifier
    pub network_addresses: Value,   // JSON array of IP addresses
    pub is_online: bool,            // Current online status
    pub last_seen_at: DateTime<Utc>,
    pub capabilities: Value,        // JSON capabilities object
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Device Capabilities

Structured capability reporting:

```rust
// Example capabilities JSON
{
    "indexing": true,       // Can index local files
    "p2p": true,           // Supports P2P connections
    "cloud": false,        // Has cloud sync enabled
    "thumbnails": true,    // Can generate thumbnails
    "preview": false,      // Can generate previews
    "transcoding": false   // Can transcode media
}
```

## Relationships

The domain models form a rich relationship graph:

```
Device (1) ──→ (N) Location
Location (1) ──→ (N) Entry
Entry (1) ──→ (1) UserMetadata
Entry (1) ──→ (0..1) ContentIdentity
Entry (1) ──→ (1) PathPrefix

UserMetadata (N) ──→ (N) Tag
UserMetadata (N) ──→ (N) Label

ContentIdentity (1) ──→ (N) Entry  [deduplication]
```

### Query Patterns

Common queries are optimized through proper indexing:

```rust
// Find all entries with a specific tag
let entries = Entry::find()
    .find_with_related(UserMetadata)
    .join(JoinType::InnerJoin, metadata_tag::Relation::Tag.def())
    .filter(tag::Column::Name.eq("Important"))
    .all(db)
    .await?;

// Find duplicate content
let duplicates = ContentIdentity::find()
    .find_with_related(Entry)
    .having(entry::Column::Id.count().gt(1))
    .group_by(content_identity::Column::CasId)
    .all(db)
    .await?;

// Reconstruct full path
let full_path = format!("{}/{}", 
    entry.prefix.prefix,
    entry.relative_path
);
```

## Serialization

All domain models support serialization for API responses and job persistence:

```rust
#[derive(Serialize, Deserialize)]
pub struct SdPathSerialized {
    pub device_uuid: Uuid,
    pub path: PathBuf,
}

impl From<Entry> for SdPathSerialized {
    fn from(entry: Entry) -> Self {
        // Convert Entry to serializable path representation
    }
}
```

## Migration Path

The domain models are designed to support migration from the original Spacedrive schema:

1. **Entry mapping** - Convert file_path and object tables to unified Entry
2. **Metadata creation** - Generate UserMetadata for all existing files  
3. **Path optimization** - Extract common prefixes and compress paths
4. **Content preservation** - Map existing CAS IDs to ContentIdentity
5. **Device unification** - Merge Node/Device/Instance concepts

This design provides a solid foundation for all Spacedrive operations while maintaining the flexibility to evolve as requirements change.