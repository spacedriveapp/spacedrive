# Pragmatic Sync System Design

## Overview

This document outlines a simplified sync system for Spacedrive Core v2 that prioritizes pragmatism over theoretical perfection. Instead of complex CRDTs and distributed consensus, we use a simple leader-based approach with automatic change tracking.

## Core Principles

1. **One Leader Per Library** - Each library has a designated leader device that maintains the sync log
2. **Automatic Tracking** - SeaORM hooks capture changes without manual intervention
3. **Domain Control** - Each model decides its own sync behavior
4. **Simple Conflicts** - Last-write-wins with optional field-level merging
5. **Pragmatic Transport** - Sync protocol is transport-agnostic (P2P, cloud, etc.)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          Library A                               │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │ Leader: Device 1│     │Follower: Device 2│                   │
│  │  ┌───────────┐  │     │  ┌───────────┐  │                   │
│  │  │  Sync Log │  │────▶│  │  Local DB │  │                   │
│  │  └───────────┘  │     │  └───────────┘  │                   │
│  └─────────────────┘     └─────────────────┘                   │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                          Library B                               │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │Follower: Device 1│     │ Leader: Device 2│                   │
│  │  ┌───────────┐  │◀────│  ┌───────────┐  │                   │
│  │  │  Local DB │  │     │  │  Sync Log │  │                   │
│  │  └───────────┘  │     │  └───────────┘  │                   │
│  └─────────────────┘     └─────────────────┘                   │
└─────────────────────────────────────────────────────────────────┘

Each library can have a different leader device, distributing the sync
responsibility across devices based on usage patterns and availability.
```

## Implementation

### 1. Sync Trait

Every syncable domain model implements a simple trait:

```rust
#[async_trait]
pub trait Syncable: ActiveModelTrait {
    /// Unique sync identifier for this model type
    const SYNC_ID: &'static str;
    
    /// Which fields should be synced (None = all fields)
    fn sync_fields() -> Option<Vec<&'static str>> {
        None // Sync all fields by default
    }
    
    /// Custom merge logic for conflicts (None = last-write-wins)
    fn merge(local: Self::Model, remote: Self::Model) -> Self::Model {
        remote // Default to remote version
    }
    
    /// Whether this model should sync at all
    fn should_sync(&self) -> bool {
        true
    }
}
```

### 2. Automatic Change Tracking

Using SeaORM's lifecycle hooks:

```rust
impl ActiveModelBehavior for DeviceActiveModel {
    // Triggered after insert or update
    fn after_save(self, insert: bool) -> Result<Self, DbErr> {
        // Record change in sync log (would need async runtime)
        if let Some(sync_log) = SYNC_LOG.get() {
            let change_type = if insert { 
                ChangeType::Insert 
            } else { 
                ChangeType::Update 
            };
            
            // Convert ActiveModel to Model for serialization
            let model = self.clone().try_into_model()?;
            
            // Queue sync operation (processed by background task)
            sync_log.queue_change(
                Device::SYNC_ID,
                model.id.to_string(),
                change_type,
                serde_json::to_value(&model).ok(),
            );
        }
        Ok(self)
    }
    
    // Triggered after delete
    fn after_delete(self) -> Result<Self, DbErr> {
        if let Some(sync_log) = SYNC_LOG.get() {
            if let Ok(model) = self.clone().try_into_model() {
                sync_log.queue_change(
                    Device::SYNC_ID,
                    model.id.to_string(),
                    ChangeType::Delete,
                    None, // No data needed for deletes
                );
            }
        }
        Ok(self)
    }
}

// Global sync log (initialized during core startup)
static SYNC_LOG: OnceCell<Arc<SyncLog>> = OnceCell::new();

// Background task processes queued changes
impl SyncLog {
    pub fn queue_change(&self, model_type: &str, record_id: String, 
                       change_type: ChangeType, data: Option<Value>) {
        self.queue.push(QueuedChange {
            model_type: model_type.to_string(),
            record_id,
            change_type,
            data,
            timestamp: Utc::now(),
        });
    }
    
    pub async fn process_queue(&self, db: &DatabaseConnection) -> Result<()> {
        while let Some(change) = self.queue.pop() {
            // Insert into sync_log table
            sync_log::ActiveModel {
                seq: NotSet, // Auto-increment
                timestamp: Set(change.timestamp),
                device_id: Set(get_current_device_id()),
                model_type: Set(change.model_type),
                record_id: Set(change.record_id),
                change_type: Set(change.change_type),
                data: Set(change.data),
            }.insert(db).await?;
        }
        Ok(())
    }
}
```

### 3. Sync Log Structure

Simple append-only log on the leader device:

```rust
pub struct SyncLogEntry {
    /// Auto-incrementing sequence number
    pub seq: u64,
    
    /// Which library this change belongs to
    pub library_id: Uuid,
    
    /// When this change occurred
    pub timestamp: DateTime<Utc>,
    
    /// Which device made the change
    pub device_id: Uuid,
    
    /// Model type identifier
    pub model_type: String,
    
    /// Record identifier
    pub record_id: String,
    
    /// Type of change
    pub change_type: ChangeType,
    
    /// Serialized model data (JSON)
    pub data: Option<serde_json::Value>,
}

pub enum ChangeType {
    Upsert, // Insert or Update
    Delete,
}
```

### 4. Sync Protocol

Dead simple request/response:

```rust
// Get changes since sequence number
pub struct PullRequest {
    pub from_seq: u64,
    pub limit: Option<usize>,
}

pub struct PullResponse {
    pub changes: Vec<SyncLogEntry>,
    pub latest_seq: u64,
}

// Push local changes (for future bi-directional sync)
pub struct PushRequest {
    pub changes: Vec<LocalChange>,
}

pub struct LocalChange {
    pub model_type: String,
    pub record_id: String,
    pub change_type: ChangeType,
    pub data: Option<serde_json::Value>,
}
```

### 5. Model Examples

#### Simple Model (Device)
```rust
impl Syncable for device::ActiveModel {
    const SYNC_ID: &'static str = "device";
    // Uses all defaults - syncs all fields, last-write-wins
}
```

#### Selective Sync (Volume)
```rust
impl Syncable for volume::ActiveModel {
    const SYNC_ID: &'static str = "volume";
    
    fn sync_fields() -> Option<Vec<&'static str>> {
        Some(vec![
            "name",
            "total_capacity", 
            "is_tracked",
            "display_name",
            "color",
            "icon",
        ])
        // Excludes: mount_point, available_space, is_mounted
    }
}
```

#### Custom Merge (UserMetadata)
```rust
impl Syncable for user_metadata::ActiveModel {
    const SYNC_ID: &'static str = "user_metadata";
    
    fn merge(local: Self::Model, remote: Self::Model) -> Self::Model {
        // Keep local tags if they're newer
        let tags = if local.updated_at > remote.updated_at {
            local.tags
        } else {
            remote.tags
        };
        
        Self::Model {
            tags,
            ..remote // Take other fields from remote
        }
    }
}
```

#### No Sync (TempFile)
```rust
impl Syncable for temp_file::ActiveModel {
    const SYNC_ID: &'static str = "temp_file";
    
    fn should_sync(&self) -> bool {
        false // Never sync temporary files
    }
}
```

## Sync Process

### Leader Device (Per Library)

1. **Check Leadership**: Verify this device is the leader for the library
2. **Capture Changes**: SeaORM hooks automatically log all changes
3. **Serve Log**: Expose sync log via API/P2P protocol
4. **Maintain State**: Track each device's sync position

### Follower Device

1. **Find Leader**: Query which device is the leader for this library
2. **Pull Changes**: Request changes since last sync from the leader
3. **Apply Changes**: Process in order, using merge logic for conflicts
4. **Track Position**: Remember last processed sequence number

### Leadership Management

```rust
/// Determine sync leader for a library
async fn get_sync_leader(library_id: Uuid) -> Result<DeviceId> {
    // Query all devices in the library
    let devices = library.get_devices().await?;
    
    // Find the designated leader
    let leader = devices
        .iter()
        .find(|d| d.is_sync_leader(&library_id))
        .ok_or("No sync leader assigned")?;
    
    Ok(leader.id)
}

/// Assign new sync leader (when current leader is offline)
async fn reassign_leader(library_id: Uuid, new_leader: DeviceId) -> Result<()> {
    // Update old leader
    if let Some(old_leader) = find_current_leader(library_id).await? {
        old_leader.set_sync_role(library_id, SyncRole::Follower);
    }
    
    // Update new leader
    let new_leader_device = get_device(new_leader).await?;
    new_leader_device.set_sync_role(library_id, SyncRole::Leader);
    
    // Notify all devices of leadership change
    broadcast_leadership_change(library_id, new_leader).await?;
    
    Ok(())
}
```

### Initial Sync

For new devices joining a library:

```rust
async fn initial_sync(leader: &SyncClient) -> Result<()> {
    // 1. Get current sequence number
    let latest = leader.get_latest_seq().await?;
    
    // 2. Pull all entries in batches
    let mut from_seq = 0;
    loop {
        let batch = leader.pull_changes(from_seq, Some(1000)).await?;
        
        for change in batch.changes {
            apply_change(change).await?;
        }
        
        if batch.latest_seq >= latest {
            break;
        }
        from_seq = batch.latest_seq;
    }
    
    // 3. Save sync position
    save_sync_position(latest).await?;
    Ok(())
}
```

## Conflict Resolution

### Last-Write-Wins (Default)
```
Device A: Update name="Photos" at 10:00
Device B: Update name="Pictures" at 10:01
Result: name="Pictures" (B's timestamp is later)
```

### Field-Level Merge (Custom)
```
Device A: Update tags=["vacation"] at 10:00
Device B: Update description="Summer 2024" at 10:01
Result: tags=["vacation"], description="Summer 2024" (non-conflicting)
```

## Advantages Over Original System

1. **Zero Manual Work**: No sync operations to write, hooks handle everything
2. **Simple Mental Model**: One leader, sequential log, pull changes
3. **Flexible**: Each model controls its own sync behavior
4. **Debuggable**: Can inspect sync log, replay changes, understand state
5. **Transport Agnostic**: Works over any connection (HTTP, WebSocket, P2P)
6. **Incremental**: Can sync partially, resume after interruption

## Migration Path

1. **Phase 1**: Implement sync traits on core models
2. **Phase 2**: Add SeaORM hooks for change tracking
3. **Phase 3**: Build simple HTTP-based sync for testing
4. **Phase 4**: Add P2P transport when ready
5. **Phase 5**: Consider multi-leader for advanced users

## Future Enhancements

### Compression
```rust
// Compress similar consecutive operations
[Update(id=1, name="A"), Update(id=1, name="B"), Update(id=1, name="C")]
// Becomes:
[Update(id=1, name="C")]
```

### Selective Sync
```rust
// Sync only specific libraries or models
sync_client.pull_changes(from_seq, Some(1000), SyncFilter {
    libraries: Some(vec![library_id]),
    models: Some(vec!["location", "tag"]),
}).await?
```

### Offline Changes
```rust
// Queue changes when offline
pub struct OfflineQueue {
    changes: Vec<LocalChange>,
}

// Replay when connected
impl OfflineQueue {
    async fn flush(&mut self, sync_client: &SyncClient) -> Result<()> {
        sync_client.push_changes(&self.changes).await?;
        self.changes.clear();
        Ok(())
    }
}
```

## Example Usage

### Making a Model Syncable

```rust
// 1. Add to domain model
#[derive(DeriveEntityModel, Syncable)]
#[sea_orm(table_name = "locations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub device_id: Uuid,
    pub updated_at: DateTime<Utc>,
}

// 2. That's it! Sync happens automatically
```

### Manual Sync Control

```rust
// Disable sync for specific operation
db.transaction_with_no_sync(|txn| async move {
    // These changes won't be synced
    location::ActiveModel {
        name: Set("Temp Location".to_string()),
        ..Default::default()
    }.insert(txn).await?;
    Ok(())
}).await?;

// Force sync of specific model
sync_log.force_record(location).await?;
```

## SeaORM Integration Details

### Hook Limitations

SeaORM hooks are synchronous, but sync logging needs async database operations. Solutions:

1. **Queue-based**: Queue changes in hooks, process asynchronously
```rust
// In hook (sync)
sync_log.queue_change(...);

// Background task (async)
loop {
    sync_log.process_queue(&db).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

2. **Thread-local Storage**: Store pending changes per-thread
```rust
thread_local! {
    static PENDING_CHANGES: RefCell<Vec<QueuedChange>> = RefCell::new(Vec::new());
}

// Flush after transaction
db.transaction(|txn| async move {
    // Do work...
    Ok(())
}).await?;

// Flush pending changes
flush_pending_sync_changes(&db).await?;
```

### Making Models Sync-Aware

Simple macro to reduce boilerplate:

```rust
#[macro_export]
macro_rules! impl_syncable {
    ($model:ty, $active_model:ty, $sync_id:expr) => {
        impl Syncable for $active_model {
            const SYNC_ID: &'static str = $sync_id;
        }
        
        impl ActiveModelBehavior for $active_model {
            fn after_save(self, insert: bool) -> Result<Self, DbErr> {
                if <$active_model as Syncable>::should_sync(&self) {
                    queue_sync_change(Self::SYNC_ID, self.clone(), insert);
                }
                Ok(self)
            }
            
            fn after_delete(self) -> Result<Self, DbErr> {
                if <$active_model as Syncable>::should_sync(&self) {
                    queue_sync_delete(Self::SYNC_ID, self.clone());
                }
                Ok(self)
            }
        }
    };
}

// Usage
impl_syncable!(device::Model, device::ActiveModel, "device");
impl_syncable!(location::Model, location::ActiveModel, "location");
```

### Database Schema

Sync log table:

```sql
CREATE TABLE sync_log (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id TEXT NOT NULL,
    timestamp DATETIME NOT NULL,
    device_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    record_id TEXT NOT NULL,
    change_type TEXT NOT NULL,
    data TEXT, -- JSON
    INDEX idx_sync_log_seq (seq),
    INDEX idx_sync_log_library (library_id, seq),
    INDEX idx_sync_log_model (model_type, record_id)
);

-- Sync position tracking per library
CREATE TABLE sync_positions (
    device_id TEXT NOT NULL,
    library_id TEXT NOT NULL,
    last_seq INTEGER NOT NULL,
    updated_at DATETIME NOT NULL,
    PRIMARY KEY (device_id, library_id)
);

-- Device sync roles (part of device table)
-- sync_leadership: JSON map of library_id -> role
```

## Implementation Roadmap

### Phase 1: Core Infrastructure (Week 1)
- [ ] Create `Syncable` trait
- [ ] Implement sync log table and models
- [ ] Build queue-based change tracking
- [ ] Add `impl_syncable!` macro

### Phase 2: Model Integration (Week 2)
- [ ] Add sync support to Device model
- [ ] Add sync support to Location model
- [ ] Add sync support to Tag model
- [ ] Test change tracking

### Phase 3: Sync Protocol (Week 3)
- [ ] Implement pull request/response
- [ ] Build sync client
- [ ] Add conflict resolution
- [ ] Test leader/follower sync

### Phase 4: Production Features (Week 4)
- [ ] Add compression for consecutive operations
- [ ] Implement selective sync
- [ ] Add offline queue
- [ ] Build sync status UI

## Conclusion

This design prioritizes simplicity and developer experience over theoretical perfection. By accepting some limitations (single leader, last-write-wins defaults), we gain a system that's easy to understand, implement, and debug. The automatic change tracking eliminates the biggest pain point of the original system while the flexible trait system allows models to customize their sync behavior as needed.

The SeaORM integration, while requiring some workarounds for async operations, provides a clean abstraction that keeps sync logic separate from business logic. With the macro system, adding sync to a model is as simple as a single line of code.