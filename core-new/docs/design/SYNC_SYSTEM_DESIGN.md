# Pragmatic Sync System Design

## Overview

This document outlines a simplified sync system for Spacedrive Core v2 that prioritizes pragmatism over theoretical perfection. Instead of complex CRDTs and distributed consensus, we use a simple leader-based approach with automatic change tracking.

## Core Principles

1. **One Leader Device** - A single device maintains the authoritative sync log
2. **Automatic Tracking** - SeaORM hooks capture changes without manual intervention
3. **Domain Control** - Each model decides its own sync behavior
4. **Simple Conflicts** - Last-write-wins with optional field-level merging
5. **Pragmatic Transport** - Sync protocol is transport-agnostic (P2P, cloud, etc.)

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Leader Device │     │ Follower Device │     │ Follower Device │
│                 │     │                 │     │                 │
│  ┌───────────┐  │     │  ┌───────────┐  │     │  ┌───────────┐  │
│  │  Sync Log │  │────▶│  │  Local DB │  │────▶│  │  Local DB │  │
│  │           │  │     │  └───────────┘  │     │  └───────────┘  │
│  │ Change #1 │  │     │                 │     │                 │
│  │ Change #2 │  │     │  Sync Client    │     │  Sync Client    │
│  │ Change #3 │  │     │  - Pull changes │     │  - Pull changes │
│  └───────────┘  │     │  - Apply local  │     │  - Apply local  │
│                 │     │                 │     │                 │
│  Sync Server    │     └─────────────────┘     └─────────────────┘
│  - Serve log    │
│  - Accept push  │
└─────────────────┘
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
    async fn after_save(model: Model, db: &DatabaseConnection) -> Result<Model, DbErr> {
        if let Some(sync_log) = db.extension::<SyncLog>() {
            sync_log.record_change(
                Device::SYNC_ID,
                model.id,
                ChangeType::Upsert,
                model.clone(),
            ).await?;
        }
        Ok(model)
    }
    
    async fn after_delete(model: Model, db: &DatabaseConnection) -> Result<Model, DbErr> {
        if let Some(sync_log) = db.extension::<SyncLog>() {
            sync_log.record_change(
                Device::SYNC_ID,
                model.id,
                ChangeType::Delete,
                model.clone(),
            ).await?;
        }
        Ok(model)
    }
}
```

### 3. Sync Log Structure

Simple append-only log on the leader device:

```rust
pub struct SyncLogEntry {
    /// Auto-incrementing sequence number
    pub seq: u64,
    
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

### Leader Device

1. **Capture Changes**: SeaORM hooks automatically log all changes
2. **Serve Log**: Expose sync log via API/P2P protocol
3. **Maintain State**: Track each device's sync position

### Follower Device

1. **Pull Changes**: Request changes since last sync
2. **Apply Changes**: Process in order, using merge logic for conflicts
3. **Track Position**: Remember last processed sequence number

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

## Conclusion

This design prioritizes simplicity and developer experience over theoretical perfection. By accepting some limitations (single leader, last-write-wins defaults), we gain a system that's easy to understand, implement, and debug. The automatic change tracking eliminates the biggest pain point of the original system while the flexible trait system allows models to customize their sync behavior as needed.