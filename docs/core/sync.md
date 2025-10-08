# Spacedrive Sync System

**Status**: Implementation Ready
**Version**: 2.0
**Last Updated**: 2025-10-08

## Overview

Spacedrive's sync system enables real-time, multi-device synchronization of library metadata, ensuring that changes made on one device are reflected across all paired devices. This document provides the definitive specification for implementing sync.

## Core Architecture

### The Three Pillars

1. **TransactionManager (TM)**: Sole gatekeeper for all syncable database writes, ensuring atomic DB commits + sync log creation
2. **Sync Log**: Append-only, sequentially-ordered log of all state changes per library, maintained only by the leader device
3. **Sync Service**: Replicates sync log entries between paired devices using pull-based synchronization

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│ Device A                                                            │
│                                                                     │
│  User Action (e.g., create album)                                  │
│         ↓                                                           │
│  [ Action Layer ]                                                   │
│         ↓                                                           │
│  [ TransactionManager ]                                             │
│         ↓                                                           │
│  ┌─────────────────────────────┐                                   │
│  │  ATOMIC TRANSACTION         │                                   │
│  │  1. Write to database       │                                   │
│  │  2. Create sync log entry   │                                   │
│  │  COMMIT                     │                                   │
│  └─────────────────────────────┘                                   │
│         ↓                                                           │
│  [ Event Bus ] → Client cache updates                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                              ↓ Sync replication
┌─────────────────────────────────────────────────────────────────────┐
│ Device B                                                            │
│                                                                     │
│  [ Sync Service ]                                                   │
│         ↓ (polls for new entries)                                  │
│  Fetch sync log from Device A                                      │
│         ↓                                                           │
│  [ Apply Sync Entry ]                                               │
│         ↓                                                           │
│  [ TransactionManager ] (applies change)                            │
│         ↓                                                           │
│  Database updated + Event emitted                                   │
│         ↓                                                           │
│  Client cache updates → UI reflects change                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Syncable Trait

All database models that need to sync implement the `Syncable` trait:

```rust
/// Enables automatic sync log creation for database models
pub trait Syncable {
    /// Stable model identifier used in sync logs (e.g., "album", "tag", "entry")
    const SYNC_MODEL: &'static str;

    /// Globally unique ID for this resource across all devices
    fn sync_id(&self) -> Uuid;

    /// Version number for optimistic concurrency control
    fn version(&self) -> i64;

    /// Optional: Exclude platform-specific or derived fields from sync
    fn exclude_fields() -> Option<&'static [&'static str]> {
        None
    }

    /// Optional: Convert to sync-safe JSON (default: full serialization)
    fn to_sync_json(&self) -> serde_json::Value where Self: Serialize {
        serde_json::to_value(self).unwrap_or(serde_json::json!({}))
    }
}
```

**Example Implementation**:
```rust
// Database model
#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "albums")]
pub struct Model {
    pub id: i32,              // Database primary key
    pub uuid: Uuid,           // Sync identifier
    pub name: String,
    pub version: i64,         // For conflict resolution
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Syncable for albums::Model {
    const SYNC_MODEL: &'static str = "album";

    fn sync_id(&self) -> Uuid {
        self.uuid
    }

    fn version(&self) -> i64 {
        self.version
    }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        // Don't sync database IDs or timestamps (platform-specific)
        Some(&["id", "created_at", "updated_at"])
    }
}
```

## TransactionManager

The TM is the **only** component that performs state-changing writes. It guarantees atomicity and automatic sync log creation.

### Core API

```rust
pub struct TransactionManager {
    event_bus: Arc<EventBus>,
    sync_sequence: Arc<Mutex<HashMap<Uuid, u64>>>, // library_id → sequence
}

impl TransactionManager {
    /// Commit single resource change (creates sync log)
    pub async fn commit<M, R>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<R, TxError>
    where
        M: Syncable + IntoActiveModel,
        R: Identifiable + From<M>;

    /// Commit batch of changes (10-1K items, creates per-item sync logs)
    pub async fn commit_batch<M, R>(
        &self,
        library: Arc<Library>,
        models: Vec<M>,
    ) -> Result<Vec<R>, TxError>
    where
        M: Syncable + IntoActiveModel,
        R: Identifiable + From<M>;

    /// Commit bulk operation (1K+ items, creates ONE metadata sync log)
    pub async fn commit_bulk<M>(
        &self,
        library: Arc<Library>,
        changes: ChangeSet<M>,
    ) -> Result<BulkAck, TxError>
    where
        M: Syncable + IntoActiveModel;
}
```

### Commit Strategies

| Method | Use Case | Sync Log | Event | Example |
|--------|----------|----------|-------|---------|
| `commit()` | Single user action | 1 per item | Rich resource | User renames file |
| `commit_batch()` | Watcher events (10-1K) | 1 per item | Batch | User copies folder |
| `commit_bulk()` | Initial indexing (1K+) | 1 metadata only | Summary | Index 1M files |

### Critical: Bulk Operations

**Problem**: Indexing 1M files shouldn't create 1M sync log entries.

**Solution**: Bulk operations create **ONE** metadata sync log:

```json
{
  "sequence": 1234,
  "model_type": "bulk_operation",
  "operation": "InitialIndex",
  "location_id": "uuid-...",
  "affected_count": 1000000,
  "hints": {
    "location_path": "/Users/alice/Photos"
  }
}
```

**Why**: Each device indexes its own filesystem independently. The sync log just says "I indexed location X" — it does NOT replicate 1M entries. Other devices trigger their own local indexing jobs when they see this notification.

**Performance Impact**:
- With per-entry sync logs: ~500MB, 10 minutes, 3M operations
- With bulk metadata: ~500 bytes, 1 minute, 1M operations (10x faster!)

### Usage Example

```rust
// Before: Manual DB write + event emission (error-prone)
let model = albums::ActiveModel { /* ... */ };
model.insert(db).await?;
event_bus.emit(Event::AlbumCreated { /* ... */ }); // Can forget this!

// After: TransactionManager (atomic, automatic)
let model = albums::ActiveModel { /* ... */ };
let album = tm.commit::<albums::Model, Album>(library, model).await?;
// ✅ DB write + sync log + event — all atomic!
```

## Sync Log Schema

```sql
CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence INTEGER NOT NULL,              -- Monotonic per library
    library_id TEXT NOT NULL,
    device_id TEXT NOT NULL,               -- Device that created this entry
    timestamp TEXT NOT NULL,

    -- Change details
    model_type TEXT NOT NULL,              -- "album", "tag", "entry", "bulk_operation"
    record_id TEXT NOT NULL,               -- UUID of changed record
    change_type TEXT NOT NULL,             -- "insert", "update", "delete", "bulk_insert"
    version INTEGER NOT NULL DEFAULT 1,     -- Optimistic concurrency

    -- Data payload (JSON)
    data TEXT NOT NULL,

    UNIQUE(library_id, sequence)
);

CREATE INDEX idx_sync_log_library_sequence ON sync_log(library_id, sequence);
CREATE INDEX idx_sync_log_device ON sync_log(device_id);
CREATE INDEX idx_sync_log_model_record ON sync_log(model_type, record_id);
```

## Leader Election

Each library requires a **single leader device** responsible for assigning sync log sequence numbers. This prevents sequence collisions.

### Election Strategy

1. **Initial Leader**: Device that creates the library
2. **Heartbeat**: Leader sends heartbeat every 30 seconds
3. **Re-election**: If leader offline >60s, devices elect new leader (highest device_id wins)
4. **Lease**: Leader holds exclusive write lease

### Implementation

```rust
pub struct SyncLeader {
    library_id: Uuid,
    leader_device_id: Uuid,
    lease_expires_at: DateTime<Utc>,
}

impl TransactionManager {
    pub async fn request_leadership(&self, library_id: Uuid) -> Result<bool, TxError> {
        // Check if current leader is still valid
        // If not, attempt to become leader
        // Update leadership table with lease
    }

    pub async fn is_leader(&self, library_id: Uuid) -> bool {
        // Check if this device holds valid lease
    }

    async fn next_sequence(&self, library_id: Uuid) -> Result<u64, TxError> {
        if !self.is_leader(library_id).await {
            return Err(TxError::NotLeader);
        }

        let mut sequences = self.sync_sequence.lock().unwrap();
        let seq = sequences.entry(library_id).or_insert(0);
        *seq += 1;
        Ok(*seq)
    }
}
```

## Sync Service (Follower)

Devices that are not the leader pull sync log entries and apply them locally.

```rust
pub struct SyncFollowerService {
    library_id: Uuid,
    leader_device_id: Uuid,
    last_synced_sequence: Arc<Mutex<u64>>,
    tx_manager: Arc<TransactionManager>,
}

impl SyncFollowerService {
    /// Poll for new sync entries (called every 5 seconds)
    pub async fn sync_iteration(&mut self) -> Result<SyncResult, SyncError> {
        let last_seq = *self.last_synced_sequence.lock().unwrap();

        // Fetch entries from leader since last_seq
        let entries = self.fetch_entries_from_leader(last_seq).await?;

        if entries.is_empty() {
            return Ok(SyncResult::NoChanges);
        }

        // Apply each entry
        for entry in entries {
            self.apply_sync_entry(entry).await?;
        }

        Ok(SyncResult::Applied { count: entries.len() })
    }

    async fn apply_sync_entry(&mut self, entry: SyncLogEntry) -> Result<(), SyncError> {
        match entry.model_type.as_str() {
            "bulk_operation" => {
                // Parse metadata
                let metadata: BulkOperationMetadata = serde_json::from_value(entry.data)?;
                self.handle_bulk_operation(metadata).await?;
            }
            _ => {
                // Regular sync entry - deserialize and apply
                let model = self.deserialize_model(&entry)?;
                self.apply_model_change(model, entry.change_type).await?;
            }
        }

        // Update last synced sequence
        *self.last_synced_sequence.lock().unwrap() = entry.sequence;
        Ok(())
    }

    async fn handle_bulk_operation(&mut self, metadata: BulkOperationMetadata) -> Result<(), SyncError> {
        match metadata.operation {
            BulkOperation::InitialIndex { location_id, location_path } => {
                tracing::info!(
                    "Peer indexed location {} with {} entries",
                    location_id, metadata.affected_count
                );

                // Check if we have this location locally
                if let Some(local_location) = self.find_matching_location(&location_path).await? {
                    // Trigger our own indexing job
                    self.job_manager.queue(IndexerJob {
                        location_id: local_location.id,
                        mode: IndexMode::Full,
                    }).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

## Library Sync Setup (Phase 1)

Before devices can sync, they must:
1. **Pair** (cryptographic authentication)
2. **Discover** libraries on remote device
3. **Register** devices in each other's libraries

See `sync-setup.md` for complete implementation details.

### Setup Flow

```rust
// 1. Discover remote libraries (after pairing)
let discovery = client.query(
    "query:network.sync_setup.discover.v1",
    DiscoverRemoteLibrariesInput { device_id: paired_device.id }
).await?;

// 2. Setup library sync (RegisterOnly in Phase 1)
let setup_result = client.action(
    "action:network.sync_setup.input.v1",
    LibrarySyncSetupInput {
        local_device_id: my_device_id,
        remote_device_id: paired_device.id,
        local_library_id: my_library.id,
        remote_library_id: discovery.libraries[0].id,
        action: LibrarySyncAction::RegisterOnly,
        leader_device_id: my_device_id, // This device becomes leader
    }
).await?;

// 3. Ready for sync!
// Sync service starts polling for changes
```

## Sync Domains

Spacedrive syncs different types of data with different strategies:

| Domain | What Syncs | Strategy |
|--------|-----------|----------|
| **Index** | File/folder entries | Metadata only (each device indexes own filesystem) |
| **Metadata** | Tags, albums, collections | Full replication across devices |
| **Content** | File content (future) | User-configured sync conduits |
| **State** | UI state, preferences | Device-specific, no sync |

## Junction Tables (Many-to-Many Relationships)

Junction tables require special consideration for sync. We use **two patterns** depending on whether the relationship itself has mutable state:

### Pattern 1: Sync as Array in Parent (Immutable Junction)

For junction tables with **no mutable metadata** (just the relationship + maybe immutable timestamps):

**Example: `collection_entry` (Collection ↔ Entry)**
```rust
// collection_entry just has: collection_id, entry_id, added_at
// ❌ Don't sync the junction table directly
// ✅ Sync as array in the Collection

impl Syncable for Collection {
    fn to_sync_json(&self) -> Result<serde_json::Value> {
        let entry_uuids = self.get_entry_uuids().await?;

        json!({
            "uuid": self.uuid,
            "name": self.name,
            "entry_uuids": entry_uuids,  // Array of UUIDs
            "version": self.version,
        })
    }
}

// When applying sync on follower:
// - Sync the Collection (which includes entry_uuids array)
// - Recreate junction records locally using UUIDs
// - Collection's version tracks when entries are added/removed
```

**Benefits**:
- Fewer sync log entries (1 instead of potentially 1000s)
- Natural "last write wins" - latest Collection state includes all relationships
- Simpler conflict resolution

### Pattern 2: Sync as Individual Entities (Mutable Junction)

For junction tables with **mutable metadata** beyond just the relationship:

**Example: `user_metadata_tag` (UserMetadata ↔ Tag with metadata)**
```rust
// user_metadata_tag has mutable fields:
// - confidence: f32 (can change)
// - applied_context: Option<String> (can change)
// - instance_attributes: JSON (can change)
// - source: String (user/ai/import)

impl Syncable for user_metadata_tag::Model {
    const SYNC_MODEL: &'static str = "user_metadata_tag";

    fn sync_id(&self) -> Uuid {
        self.uuid  // Each relationship has its own UUID
    }

    fn version(&self) -> i64 {
        self.version  // Needs version for conflict resolution
    }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "user_metadata_id", "tag_id", "created_at", "updated_at"])
    }
}
```

**Why this needs versioning**:
- The relationship itself has mutable state (confidence, attributes)
- Two devices could modify the same relationship concurrently
- Need conflict resolution for the relationship's metadata

**Example: `tag_relationship` (Tag ↔ Tag with metadata)**
```rust
// tag_relationship has mutable fields:
// - relationship_type: String (parent_child/synonym/related - can change)
// - strength: f32 (can change)

impl Syncable for tag_relationship::Model {
    const SYNC_MODEL: &'static str = "tag_relationship";

    fn sync_id(&self) -> Uuid {
        self.uuid
    }

    fn version(&self) -> i64 {
        self.version
    }
}
```

### Decision Tree

```rust
// Use this to decide which pattern:
if junction_table.has_mutable_fields_beyond_the_link() {
    // Pattern 2: Sync as entity with UUID and version
    sync_individually_with_version();
} else {
    // Pattern 1: Sync as array in parent entity
    sync_as_part_of_parent();
}
```

### Spacedrive's Junction Tables

| Junction Table | Pattern | Reason |
|----------------|---------|--------|
| `user_metadata_tag` | **Pattern 2** | Has mutable fields: `confidence`, `applied_context`, `instance_attributes` |
| `tag_relationship` | **Pattern 2** | Has mutable fields: `relationship_type`, `strength` |
| `collection_entry` | **Pattern 1** | Only has immutable `added_at` timestamp |

### Foreign Key Handling

Junction tables must use **UUIDs, not database IDs** for relationships:

```rust
// ❌ Bad: Using database IDs
CREATE TABLE user_metadata_tag (
    user_metadata_id INTEGER,  // Device-specific!
    tag_id INTEGER              // Device-specific!
);

// ✅ Good: Using UUIDs
CREATE TABLE user_metadata_tag (
    user_metadata_uuid UUID,   // Global!
    tag_uuid UUID               // Global!
);

// Or resolve during sync:
async fn apply_relationship(remote: RelationshipData) {
    let local_metadata_id = resolve_uuid_to_id(remote.user_metadata_uuid).await?;
    let local_tag_id = resolve_uuid_to_id(remote.tag_uuid).await?;

    insert_user_metadata_tag(local_metadata_id, local_tag_id, remote.confidence).await?;
}
```

## Conflict Resolution

### Optimistic Concurrency

All `Syncable` models have a `version` field. When applying a sync entry:

```rust
async fn apply_model_change(&self, remote_model: Model, change_type: ChangeType) -> Result<()> {
    match change_type {
        ChangeType::Update => {
            // Fetch current local version
            let local_model = Model::find_by_uuid(remote_model.sync_id(), db).await?;

            if let Some(local) = local_model {
                if local.version >= remote_model.version {
                    // Local is newer or same - skip update
                    tracing::debug!("Skipping sync entry: local version is newer");
                    return Ok(());
                }
            }

            // Remote is newer - apply update
            remote_model.update(db).await?;
        }
        ChangeType::Insert => {
            remote_model.insert(db).await?;
        }
        ChangeType::Delete => {
            Model::delete_by_uuid(remote_model.sync_id(), db).await?;
        }
    }
    Ok(())
}
```

### Conflict Strategy

- **Last-Write-Wins (LWW)**: Use `version` field to determine winner
- **No CRDTs**: Simpler, sufficient for metadata sync
- **User Metadata**: Tags, albums use union merge (both versions kept)

## Raw SQL Compatibility

**Reads**: Unrestricted. Use SeaORM query builder or raw SQL freely.

**Writes**: Must go through TransactionManager. For advanced cases:

```rust
tm.with_tx(library, |txn| async move {
    // Raw SQL writes inside TM transaction
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE albums SET name = ? WHERE uuid = ?",
        vec![name.into(), uuid.into()],
    )).await?;

    // Tell TM to log this change
    tm.sync_log_for::<albums::Model>(txn, uuid).await?;

    Ok(())
}).await?;
```

## Implementation Roadmap

### Phase 1: Foundation (Current)
- [x] Device pairing protocol
- [x] Library sync setup (RegisterOnly)
- [ ] TransactionManager core
- [ ] Syncable trait + derives
- [ ] Sync log schema
- [ ] Leader election

### Phase 2: Basic Sync
- [ ] Sync follower service (pull-based)
- [ ] Apply sync entries
- [ ] Handle bulk operations
- [ ] Conflict resolution
- [ ] Album/Tag/Location sync

### Phase 3: File Sync
- [ ] Entry sync (metadata only)
- [ ] Watcher integration
- [ ] Bulk indexing with metadata logs
- [ ] Cross-device file operations

### Phase 4: Advanced Features
- [ ] Content sync (via sync conduits)
- [ ] Push-based sync (optional optimization)
- [ ] Multi-leader support
- [ ] Conflict resolution UI

## Performance Considerations

### Indexing Performance

- **1M files, per-entry logs**: 10 minutes, 500MB sync log
- **1M files, bulk metadata**: 1 minute, 500 bytes sync log
- **Result**: 10x faster, 1 million times smaller sync log

### Network Efficiency

- Pull-based sync: Batch fetch (max 100 entries per request)
- Compression: Gzip sync log JSON (typically 5x reduction)
- Delta sync: Only fetch entries since last sequence

### Database Optimization

- Sync log: Append-only, no updates (fast writes)
- Indexes on (library_id, sequence) for efficient polling
- Vacuum old entries after successful sync (> 30 days)

## Security

### Encryption
- All sync data transmitted over encrypted Iroh streams
- Sync log contains full model data (no encryption at rest in Phase 1)
- Future: Library-level encryption (see `AT_REST_LIBRARY_ENCRYPTION.md`)

### Access Control
- Only paired devices can sync
- Device pairing uses cryptographic challenge/response
- Leader election prevents unauthorized writes

## Testing Strategy

### Unit Tests
```rust
#[tokio::test]
async fn test_sync_log_creation() {
    let tm = TransactionManager::new(event_bus);
    let model = albums::Model { /* ... */ };

    let album = tm.commit::<albums::Model, Album>(library, model).await.unwrap();

    // Verify sync log entry created
    let entry = sync_log::Entity::find()
        .filter(sync_log::Column::RecordId.eq(album.id))
        .one(db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(entry.model_type, "album");
}
```

### Integration Tests
- Two-device sync simulation
- Leader failover scenarios
- Bulk operation handling
- Conflict resolution

## References

- **Sync Setup**: `docs/core/sync-setup.md`
- **Event System**: `docs/core/events.md`
- **Client Cache**: `docs/core/normalized_cache.md`
- **Design Details**: `docs/core/design/sync/`
