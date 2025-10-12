# Spacedrive Database Schema - Actual Implementation

This document describes the **actual database schema** as implemented in the entity files, reflecting the current state of the codebase (as of the recent cleanup).

## Core Architecture

The schema implements a **Virtual Distributed File System (VDFS)** with:
- **Hierarchical file/directory representation** via Entry + closure tables
- **Content deduplication** via ContentIdentity
- **Semantic tagging system** with graph-based relationships
- **Device-owned and shared resources** with sync capabilities
- **User metadata** separate from system metadata

---

## Entity Relationships Diagram

```
Device (1) ──→ (N) Location
       ↓
       └──→ (N) Volume

Location (1) ──→ (1) Entry (root)

Entry (self-referential hierarchy)
  ├── parent_id → Entry
  ├── metadata_id → UserMetadata (optional)
  ├── content_id → ContentIdentity (optional)
  └── closure table → EntryClosure

EntryClosure (transitive closure for hierarchy queries)
  ├── ancestor_id → Entry
  └── descendant_id → Entry

DirectoryPaths (materialized path cache)
  └── entry_id → Entry

ContentIdentity (deduplication)
  ├── mime_type_id → MimeType (optional)
  ├── kind_id → ContentKind
  └── (1) ──→ (N) Entry (many files, same content)

UserMetadata
  ├── entry_uuid → Entry (file-specific, OR)
  ├── content_identity_uuid → ContentIdentity (content-universal)
  └── (N) ──→ (N) Tag via UserMetadataTag

Tag (semantic tagging)
  ├── (N) ──→ (N) TagRelationship (parent/child/synonym)
  ├── closure table → TagClosure
  └── usage patterns → TagUsagePattern

Collection
  └── (N) ──→ (N) Entry via CollectionEntry

Sidecar
  ├── content_uuid → ContentIdentity
  └── source_entry_id → Entry (optional, for references)
```

---

## Core Tables

### Device
**Purpose**: Represents physical/virtual devices in the Spacedrive network

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique device identifier |
| name | String | Device name |
| os | String | Operating system |
| os_version | String? | OS version |
| hardware_model | String? | Hardware model |
| network_addresses | Json | Array of network addresses |
| is_online | bool | Current online status |
| last_seen_at | DateTime | Last seen timestamp |
| capabilities | Json | Device capabilities |
| sync_enabled | bool | Whether sync is enabled |
| last_sync_at | DateTime? | Last sync timestamp |
| created_at | DateTime | |
| updated_at | DateTime | |

**Relationships**:
- Has many: Location, Volume
- Sync: State-based (device-owned)

---

### Location
**Purpose**: Represents an indexed directory on a device

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique location identifier |
| device_id | i32 | FK to Device |
| entry_id | i32 | FK to Entry (root directory) |
| name | String? | Optional custom name |
| index_mode | String | "shallow" \| "content" \| "deep" |
| scan_state | String | "pending" \| "scanning" \| "completed" \| "error" |
| last_scan_at | DateTime? | Last scan timestamp |
| error_message | String? | Error details if failed |
| total_file_count | i64 | Total files indexed |
| total_byte_size | i64 | Total size in bytes |
| created_at | DateTime | |
| updated_at | DateTime | |

**Relationships**:
- Belongs to: Device, Entry (root)
- Sync: State-based (device-owned)

---

### Entry
**Purpose**: Universal representation of files and directories

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid? | Assigned after content identification (sync readiness) |
| name | String | File/directory name |
| kind | i32 | 0=File, 1=Directory, 2=Symlink |
| extension | String? | File extension (without dot) |
| metadata_id | i32? | FK to UserMetadata (optional) |
| content_id | i32? | FK to ContentIdentity (optional) |
| size | i64 | Size in bytes (0 for directories) |
| aggregate_size | i64 | Total size including children |
| child_count | i32 | Direct children count |
| file_count | i32 | Total files in subtree |
| created_at | DateTime | File creation time |
| modified_at | DateTime | File modification time |
| accessed_at | DateTime? | File access time |
| permissions | String? | Unix permissions |
| inode | i64? | Platform-specific file identifier |
| parent_id | i32? | FK to parent Entry |

**Key Design**:
- `uuid` is `None` until content identification completes (directories and empty files get UUIDs immediately)
- Self-referential hierarchy via `parent_id`
- Closure table for efficient hierarchical queries

**Relationships**:
- Belongs to: UserMetadata, ContentIdentity, Entry (parent)
- Has many: Entry (children)
- Closure: EntryClosure
- Sync: State-based (device-owned)

---

### EntryClosure
**Purpose**: Transitive closure table for efficient hierarchical queries

| Field | Type | Description |
|-------|------|-------------|
| ancestor_id | i32 | PK, FK to Entry |
| descendant_id | i32 | PK, FK to Entry |
| depth | i32 | Distance (0=self, 1=direct child) |

**Key Design**:
- Composite primary key: (ancestor_id, descendant_id)
- Enables fast "all descendants" and "all ancestors" queries
- Includes self-referential rows (depth=0)

---

### DirectoryPaths
**Purpose**: Materialized path cache for directory paths

| Field | Type | Description |
|-------|------|-------------|
| entry_id | i32 | PK, FK to Entry |
| path | String | Full directory path |

**Key Design**:
- Only for directories
- Rebuilt during sync to maintain correctness
- Cached for performance (avoid recursive path construction)

---

### ContentIdentity
**Purpose**: Content deduplication - represents unique file content

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid? | Deterministic UUID from content_hash + library_id |
| integrity_hash | String? | Full hash for validation |
| content_hash | String | Fast sampled hash for dedup |
| mime_type_id | i32? | FK to MimeType |
| kind_id | i32 | FK to ContentKind |
| media_data | Json? | Media-specific metadata |
| text_content | String? | Extracted text content |
| total_size | i64 | Size of one instance |
| entry_count | i32 | Number of entries with this content (in this library) |
| first_seen_at | DateTime | First discovery |
| last_verified_at | DateTime | Last validation |

**Key Design**:
- `uuid` is deterministic: `UUID_v5(library_namespace, content_hash)` - ensures consistency within library
- `content_hash` is fast (sampled) for deduplication
- `integrity_hash` is full hash for validation (optional, generated by validate job)
- One ContentIdentity can have many Entries (deduplication)

**Relationships**:
- Belongs to: MimeType, ContentKind
- Has many: Entry, Sidecar

---

### MimeType
**Purpose**: Lookup table for MIME types (runtime discovered)

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique identifier |
| mime_type | String | MIME type string (unique) |
| created_at | DateTime | |

---

### ContentKind
**Purpose**: Lookup table for content kinds (predefined enum)

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key (no auto-increment) |
| name | String | Kind name |

**Values**: Document, Image, Video, Audio, Archive, Code, etc.

---

## User Organization

### UserMetadata
**Purpose**: User-applied metadata for entries or content

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique identifier |
| entry_uuid | Uuid? | FK to Entry (file-specific, OR) |
| content_identity_uuid | Uuid? | FK to ContentIdentity (content-universal) |
| notes | String? | Free-form notes |
| favorite | bool | Favorite flag |
| hidden | bool | Hidden flag |
| custom_data | Json | Arbitrary custom fields |
| created_at | DateTime | |
| updated_at | DateTime | |

**Key Design**:
- **Scoped metadata**: Either entry-scoped OR content-scoped (exactly one must be set)
  - Entry-scoped: Metadata specific to a file instance (higher priority)
  - Content-scoped: Metadata applying to all instances of the same content (lower priority)
- **Tags are NOT stored here** - managed via semantic tagging system (UserMetadataTag junction)

**Relationships**:
- Belongs to: Entry OR ContentIdentity (mutually exclusive)
- Has many: Tag (via UserMetadataTag junction)

---

### Tag
**Purpose**: Semantic tags with graph-based relationships

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique tag identifier |
| canonical_name | String | Primary name |
| display_name | String? | Display variant |
| formal_name | String? | Formal variant |
| abbreviation | String? | Short form |
| aliases | Json? | Array of alternative names |
| namespace | String? | Namespace for disambiguation |
| tag_type | String | "standard" \| "organizational" \| "privacy" \| "system" |
| color | String? | Hex color |
| icon | String? | Icon identifier |
| description | String? | Tag description |
| is_organizational_anchor | bool | Creates UI hierarchies |
| privacy_level | String | "normal" \| "archive" \| "hidden" |
| search_weight | i32 | Search ranking weight |
| attributes | Json? | Custom attributes HashMap |
| composition_rules | Json? | Composition rules |
| created_at | DateTime | |
| updated_at | DateTime | |
| created_by_device | Uuid? | Creating device |

**Key Design**:
- **Polymorphic naming**: Same canonical_name can have different UUIDs (context-dependent)
- **Multiple name variants**: canonical, display, formal, abbreviation, aliases
- **Namespace disambiguation**: "travel::vacation" vs "work::vacation"
- **Shared resource**: Syncs via HLC-ordered log with union merge

**Relationships**:
- Has many: TagRelationship (parent/child relationships)
- Closure: TagClosure (transitive closure)
- Usage: TagUsagePattern
- Sync: Log-based (shared resource)

---

### UserMetadataTag
**Purpose**: Junction table for tags applied to user metadata

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| user_metadata_id | i32 | FK to UserMetadata |
| tag_id | i32 | FK to Tag |
| applied_context | String? | Context when applied |
| applied_variant | String? | Which name variant was used |
| confidence | f32 | Confidence score (0.0-1.0) |
| source | String | "user" \| "ai" \| "import" \| "sync" |
| instance_attributes | Json? | Instance-specific attributes |
| created_at | DateTime | |
| updated_at | DateTime | |
| device_uuid | Uuid | Device that applied the tag |

**Key Design**:
- Each tag application is tracked separately with metadata
- Supports AI-applied tags with confidence scores
- Tracks which name variant was used to apply the tag

---

### TagRelationship
**Purpose**: Direct relationships between tags

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| parent_tag_id | i32 | FK to Tag |
| child_tag_id | i32 | FK to Tag |
| relationship_type | String | "parent_child" \| "synonym" \| "related" |
| strength | f32 | Relationship strength (0.0-1.0) |
| created_at | DateTime | |

---

### TagClosure
**Purpose**: Transitive closure for tag hierarchies

| Field | Type | Description |
|-------|------|-------------|
| ancestor_id | i32 | PK, FK to Tag |
| descendant_id | i32 | PK, FK to Tag |
| depth | i32 | Distance in hierarchy |
| path_strength | f32 | Cumulative relationship strength |

**Key Design**:
- Enables efficient "all descendants" and "all ancestors" queries
- Similar to EntryClosure but for tags

---

### TagUsagePattern
**Purpose**: Track tag usage patterns for recommendations

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| tag_id | i32 | FK to Tag |
| context | String | Usage context |
| frequency | i32 | Usage count |
| last_used_at | DateTime | |
| created_at | DateTime | |

---

## Collections

### Collection
**Purpose**: User-created collections of entries

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique identifier |
| name | String | Collection name |
| description | String? | Collection description |
| created_at | DateTime | |
| updated_at | DateTime | |

**Relationships**:
- Has many: Entry (via CollectionEntry junction)

---

### CollectionEntry
**Purpose**: Junction table for collection membership

| Field | Type | Description |
|-------|------|-------------|
| collection_id | i32 | PK, FK to Collection |
| entry_id | i32 | PK, FK to Entry |
| added_at | DateTime | When added to collection |

**Key Design**:
- Composite primary key
- Cascade deletes on both sides

---

## Sidecars

### Sidecar
**Purpose**: Generated derivatives and metadata files

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| content_uuid | Uuid | FK to ContentIdentity |
| kind | String | Sidecar type (e.g., "thumbnail", "preview") |
| variant | String | Variant identifier |
| format | String | File format |
| rel_path | String | Relative path to sidecar file |
| source_entry_id | i32? | FK to Entry (for reference sidecars) |
| size | i64 | File size in bytes |
| checksum | String? | File checksum |
| status | String | "pending" \| "processing" \| "ready" \| "error" |
| source | String? | Generation source |
| version | i32 | Sidecar version |
| created_at | DateTime | |
| updated_at | DateTime | |

**Key Design**:
- Linked to ContentIdentity (not Entry) - one thumbnail for all duplicates
- `source_entry_id` allows referencing existing files as sidecars without copying

---

### SidecarAvailability
**Purpose**: Track which sidecars are available on which devices

| Field | Type | Description |
|-------|------|-------------|
| sidecar_id | i32 | PK, FK to Sidecar |
| device_id | i32 | PK, FK to Device |
| is_available | bool | Whether sidecar is present |
| last_checked_at | DateTime | Last availability check |

---

## Volume Tracking

### Volume
**Purpose**: Track physical volumes (drives, partitions)

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | Uuid | Unique volume identifier |
| device_id | Uuid | FK to Device |
| fingerprint | String | Unique volume fingerprint |
| display_name | String? | Volume name |
| tracked_at | DateTime | When tracking started |
| last_seen_at | DateTime | Last seen online |
| is_online | bool | Current online status |
| total_capacity | i64? | Total capacity in bytes |
| available_capacity | i64? | Available space in bytes |
| read_speed_mbps | i32? | Read speed in MB/s |
| write_speed_mbps | i32? | Write speed in MB/s |
| last_speed_test_at | DateTime? | Last speed test |
| file_system | String? | Filesystem type |
| mount_point | String? | Current mount point |
| is_removable | bool? | Whether removable |
| is_network_drive | bool? | Whether network drive |
| device_model | String? | Physical device model |
| volume_type | String? | Volume classification |
| is_user_visible | bool? | Visible in default UI |
| auto_track_eligible | bool? | Eligible for auto-tracking |

**Key Design**:
- Fingerprint is stable across mounts/unmounts
- Tracks performance characteristics
- Supports removable media tracking

---

## Audit & Jobs

### AuditLog
**Purpose**: Track system operations for accountability

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| uuid | String | Unique log entry identifier |
| action_type | String | Type of action |
| actor_device_id | String | Device that performed action |
| targets | String | Affected resources |
| status | String | "success" \| "error" \| "pending" |
| job_id | String? | Associated job ID |
| created_at | DateTime | When action started |
| completed_at | DateTime? | When action completed |
| error_message | String? | Error details if failed |
| result_payload | String? | Action results |

---

### IndexerRule
**Purpose**: Rules for indexing behavior

| Field | Type | Description |
|-------|------|-------------|
| id | i32 | Primary key |
| location_id | i32? | FK to Location (optional) |
| rule_type | String | Type of rule |
| pattern | String | Match pattern |
| action | String | Action to take |
| priority | i32 | Rule priority |
| enabled | bool | Whether rule is active |
| created_at | DateTime | |
| updated_at | DateTime | |

---

## Key Design Principles

### 1. **Immediate Organization**
Every Entry can have UserMetadata immediately, even before content indexing completes. This enables instant tagging and organization.

### 2. **Content Deduplication**
ContentIdentity is separate from Entry, allowing multiple files with identical content to reference a single content record. Saves space and enables powerful queries.

### 3. **Hierarchical Queries**
Both Entry and Tag use closure tables (EntryClosure, TagClosure) for efficient hierarchical queries without recursive SQL.

### 4. **Scoped Metadata**
UserMetadata supports both entry-scoped (file-specific) and content-scoped (applies to all duplicates) organization.

### 5. **Semantic Tagging**
Tags are first-class entities with:
- Polymorphic naming (same name, different contexts)
- Graph-based relationships
- Rich metadata and variants
- AI support with confidence scores

### 6. **Sync-Ready Architecture**
- Device-owned resources (Entry, Location) use state-based replication
- Shared resources (Tag) use HLC-ordered log-based replication
- UUID assignment indicates sync readiness
- FK mappings support cross-device references

### 7. **Separation of Concerns**
- **Entry**: Physical file system representation
- **ContentIdentity**: What's inside the file
- **UserMetadata**: How the user organizes it
- **Tag**: Rich semantic categorization
- **Sidecar**: Generated derivatives

---

## Sync Implementation Notes

### State-Based (Device-Owned)
- **Entities**: Device, Location, Entry
- **Strategy**: Last state wins, no conflict resolution needed
- **Ownership**: Only owning device can modify

### Log-Based (Shared)
- **Entities**: Tag, TagRelationship
- **Strategy**: HLC-ordered with union merge
- **Conflict**: Different UUIDs coexist (polymorphic naming)

### FK Mapping
- Integer IDs used locally for performance
- UUIDs used for sync wire format
- `FKMapper` handles bidirectional conversion
- Ensures references work across devices
