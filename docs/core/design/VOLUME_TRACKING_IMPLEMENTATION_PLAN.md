# Volume Tracking Implementation Plan

## Overview
This document outlines the implementation plan for volume tracking functionality in Spacedrive, aligned with existing codebase patterns and architecture.

## Current State Analysis

### What Exists
- `VolumeManager` with in-memory volume detection
- Volume events already defined in event system
- Volume actions scaffolded (Track, Untrack, SpeedTest)
- SeaORM infrastructure and migration system
- Hybrid ID pattern (integer + UUID) for entities

### What's Missing
- Database migration for volumes table
- SeaORM entity for volumes
- Actual database operations in VolumeManager
- Volume-library relationship tracking

## Implementation Plan

### Phase 1: Database Schema

#### 1.1 Create Migration
Create: `crates/migration/src/m20240125_create_volumes.rs`

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create volumes table
        manager
            .create_table(
                Table::create()
                    .table(Volume::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Volume::Id).integer().not_null().primary_key().auto_increment())
                    .col(ColumnDef::new(Volume::Uuid).string().not_null().unique_key())
                    .col(ColumnDef::new(Volume::Fingerprint).string().not_null())
                    .col(ColumnDef::new(Volume::LibraryId).integer().not_null())
                    .col(ColumnDef::new(Volume::DisplayName).string())
                    .col(ColumnDef::new(Volume::TrackedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Volume::LastSeenAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Volume::IsOnline).boolean().not_null().default(true))
                    .col(ColumnDef::new(Volume::TotalCapacity).big_integer())
                    .col(ColumnDef::new(Volume::AvailableCapacity).big_integer())
                    .col(ColumnDef::new(Volume::ReadSpeedMbps).integer())
                    .col(ColumnDef::new(Volume::WriteSpeedMbps).integer())
                    .col(ColumnDef::new(Volume::LastSpeedTestAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Volume::Table, Volume::LibraryId)
                            .to(Library::Table, Library::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .to_owned(),
            )
            .await?;
            
        // Create index on fingerprint for fast lookups
        manager
            .create_index(
                Index::create()
                    .table(Volume::Table)
                    .name("idx_volume_fingerprint_library")
                    .col(Volume::Fingerprint)
                    .col(Volume::LibraryId)
                    .unique()
                    .to_owned(),
            )
            .await?;
            
        Ok(())
    }
}
```

#### 1.2 Create SeaORM Entity
Create: `src/infrastructure/database/entities/volume.rs`

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "volumes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: String,
    pub fingerprint: String,
    pub library_id: i32,
    pub display_name: Option<String>,
    pub tracked_at: DateTimeWithTimeZone,
    pub last_seen_at: DateTimeWithTimeZone,
    pub is_online: bool,
    pub total_capacity: Option<i64>,
    pub available_capacity: Option<i64>,
    pub read_speed_mbps: Option<i32>,
    pub write_speed_mbps: Option<i32>,
    pub last_speed_test_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::library::Entity",
        from = "Column::LibraryId",
        to = "super::library::Column::Id"
    )]
    Library,
}

impl Related<super::library::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Library.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Domain model conversion
impl Model {
    pub fn to_domain(&self) -> crate::volume::TrackedVolume {
        crate::volume::TrackedVolume {
            id: self.id,
            uuid: Uuid::parse_str(&self.uuid).unwrap(),
            fingerprint: VolumeFingerprint(self.fingerprint.clone()),
            display_name: self.display_name.clone(),
            tracked_at: self.tracked_at,
            last_seen_at: self.last_seen_at,
            is_online: self.is_online,
            total_capacity: self.total_capacity.map(|c| c as u64),
            available_capacity: self.available_capacity.map(|c| c as u64),
            read_speed_mbps: self.read_speed_mbps.map(|s| s as u32),
            write_speed_mbps: self.write_speed_mbps.map(|s| s as u32),
            last_speed_test_at: self.last_speed_test_at,
        }
    }
}
```

### Phase 2: Update VolumeManager

#### 2.1 Add Database Operations
Update `src/volume/manager.rs`:

```rust
impl VolumeManager {
    /// Track a volume in a library
    pub async fn track_volume(
        &self,
        library: &Library,
        fingerprint: &VolumeFingerprint,
        display_name: Option<String>,
    ) -> Result<entities::volume::Model, VolumeError> {
        let db = library.db().conn();
        
        // Check if already tracked
        if let Some(existing) = entities::volume::Entity::find()
            .filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
            .filter(entities::volume::Column::LibraryId.eq(library.id()))
            .one(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?
        {
            return Err(VolumeError::AlreadyTracked);
        }
        
        // Get current volume info
        let volume = self.get_volume(fingerprint).await
            .ok_or_else(|| VolumeError::NotFound(fingerprint.clone()))?;
            
        // Create tracking record
        let active_model = entities::volume::ActiveModel {
            uuid: Set(Uuid::new_v4().to_string()),
            fingerprint: Set(fingerprint.0.clone()),
            library_id: Set(library.id()),
            display_name: Set(display_name),
            tracked_at: Set(chrono::Utc::now()),
            last_seen_at: Set(chrono::Utc::now()),
            is_online: Set(volume.is_mounted),
            total_capacity: Set(Some(volume.total_bytes as i64)),
            available_capacity: Set(Some(volume.total_bytes_available as i64)),
            read_speed_mbps: Set(volume.read_speed_mbps.map(|s| s as i32)),
            write_speed_mbps: Set(volume.write_speed_mbps.map(|s| s as i32)),
            last_speed_test_at: Set(None),
            ..Default::default()
        };
        
        let model = active_model
            .insert(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?;
            
        Ok(model)
    }
    
    /// Untrack a volume from a library
    pub async fn untrack_volume(
        &self,
        library: &Library,
        fingerprint: &VolumeFingerprint,
    ) -> Result<(), VolumeError> {
        let db = library.db().conn();
        
        let result = entities::volume::Entity::delete_many()
            .filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
            .filter(entities::volume::Column::LibraryId.eq(library.id()))
            .exec(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?;
            
        if result.rows_affected == 0 {
            return Err(VolumeError::NotTracked);
        }
        
        Ok(())
    }
    
    /// Get all volumes tracked in a library
    pub async fn get_tracked_volumes(
        &self,
        library: &Library,
    ) -> Result<Vec<entities::volume::Model>, VolumeError> {
        let db = library.db().conn();
        
        let volumes = entities::volume::Entity::find()
            .filter(entities::volume::Column::LibraryId.eq(library.id()))
            .all(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?;
            
        Ok(volumes)
    }
    
    /// Update tracked volume state during refresh
    pub async fn update_tracked_volume_state(
        &self,
        library: &Library,
        fingerprint: &VolumeFingerprint,
        volume: &Volume,
    ) -> Result<(), VolumeError> {
        let db = library.db().conn();
        
        let mut active_model: entities::volume::ActiveModel = entities::volume::Entity::find()
            .filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
            .filter(entities::volume::Column::LibraryId.eq(library.id()))
            .one(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?
            .ok_or_else(|| VolumeError::NotTracked)?
            .into();
            
        active_model.last_seen_at = Set(chrono::Utc::now());
        active_model.is_online = Set(volume.is_mounted);
        active_model.total_capacity = Set(Some(volume.total_bytes as i64));
        active_model.available_capacity = Set(Some(volume.total_bytes_available as i64));
        
        active_model
            .update(db)
            .await
            .map_err(|e| VolumeError::Database(e.to_string()))?;
            
        Ok(())
    }
}
```

### Phase 3: Update Volume Actions

#### 3.1 Track Action
Update `src/operations/volumes/track/handler.rs`:

```rust
match action {
    Action::VolumeTrack { action } => {
        // Get library
        let library = context
            .library_manager
            .get_library(action.library_id)
            .await
            .ok_or_else(|| ActionError::LibraryNotFound(action.library_id))?;
            
        // Track the volume
        let tracked = context
            .volume_manager
            .track_volume(&library, &action.fingerprint, action.name.clone())
            .await
            .map_err(|e| match e {
                VolumeError::AlreadyTracked => ActionError::InvalidInput(
                    "Volume is already tracked in this library".to_string()
                ),
                VolumeError::NotFound(_) => ActionError::InvalidInput(
                    "Volume not found".to_string()
                ),
                _ => ActionError::Internal(e.to_string()),
            })?;
            
        // Get volume info for the response
        let volume = context
            .volume_manager
            .get_volume(&action.fingerprint)
            .await
            .ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;
            
        // Emit event
        context.events.emit(Event::VolumeTracked {
            library_id: action.library_id,
            volume_fingerprint: action.fingerprint.clone(),
            display_name: tracked.display_name.clone(),
        }).await;
        
        Ok(ActionOutput::VolumeTracked {
            fingerprint: action.fingerprint,
            library_id: action.library_id,
            volume_name: tracked.display_name.unwrap_or(volume.name),
        })
    }
    _ => Err(ActionError::InvalidActionType),
}
```

### Phase 4: Volume Refresh Integration

#### 4.1 Update refresh_volumes
In `src/volume/manager.rs`:

```rust
pub async fn refresh_volumes(&self) -> Result<(), VolumeError> {
    let new_volumes = detect_volumes(&self.config)?;
    
    // Update in-memory cache
    let mut volumes = self.volumes.write().await;
    let old_volumes = std::mem::replace(&mut *volumes, new_volumes);
    
    // Detect changes and emit events
    for new_vol in &*volumes {
        if let Some(old_vol) = old_volumes.iter().find(|v| v.fingerprint == new_vol.fingerprint) {
            // Check for changes
            if old_vol.is_mounted != new_vol.is_mounted {
                self.events.emit(Event::VolumeMountChanged {
                    fingerprint: new_vol.fingerprint.clone(),
                    is_mounted: new_vol.is_mounted,
                }).await;
            }
            // Check capacity changes...
        } else {
            // New volume
            self.events.emit(Event::VolumeAdded {
                fingerprint: new_vol.fingerprint.clone(),
                name: new_vol.name.clone(),
            }).await;
        }
    }
    
    // Update tracked volumes in all libraries
    if let Some(library_manager) = self.library_manager.upgrade() {
        for library in library_manager.get_all_libraries().await {
            for tracked_volume in self.get_tracked_volumes(&library).await? {
                if let Some(current_volume) = volumes.iter()
                    .find(|v| v.fingerprint.0 == tracked_volume.fingerprint)
                {
                    self.update_tracked_volume_state(
                        &library,
                        &current_volume.fingerprint,
                        current_volume,
                    ).await?;
                }
            }
        }
    }
    
    Ok(())
}
```

### Phase 5: Background Service

#### 5.1 Create Volume Monitor Service
Create: `src/services/volume_monitor.rs`

```rust
use crate::{
    services::{Service, ServiceError},
    volume::VolumeManager,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct VolumeMonitorService {
    volume_manager: Arc<VolumeManager>,
    running: Arc<RwLock<bool>>,
    handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl VolumeMonitorService {
    pub fn new(volume_manager: Arc<VolumeManager>) -> Self {
        Self {
            volume_manager,
            running: Arc::new(RwLock::new(false)),
            handle: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl Service for VolumeMonitorService {
    async fn start(&self) -> Result<(), ServiceError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        
        let volume_manager = self.volume_manager.clone();
        let running_flag = self.running.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            
            while *running_flag.read().await {
                interval.tick().await;
                
                if let Err(e) = volume_manager.refresh_volumes().await {
                    tracing::error!("Failed to refresh volumes: {}", e);
                }
            }
        });
        
        *self.handle.write().await = Some(handle);
        Ok(())
    }
    
    async fn stop(&self) -> Result<(), ServiceError> {
        *self.running.write().await = false;
        
        if let Some(handle) = self.handle.write().await.take() {
            handle.abort();
        }
        
        Ok(())
    }
    
    async fn is_running(&self) -> bool {
        *self.running.read().await
    }
    
    fn name(&self) -> &'static str {
        "volume_monitor"
    }
}
```

### Phase 6: Event System Updates

The volume events are already defined in `src/infrastructure/events/mod.rs`:
- `VolumeAdded`
- `VolumeRemoved` 
- `VolumeUpdated`
- `VolumeSpeedTested`
- `VolumeMountChanged`
- `VolumeError`
- `VolumeTracked` (need to add)
- `VolumeUntracked` (need to add)

Add the tracking events:

```rust
/// Volume was tracked in a library
VolumeTracked {
    library_id: Uuid,
    volume_fingerprint: VolumeFingerprint,
    display_name: Option<String>,
},

/// Volume was untracked from a library
VolumeUntracked {
    library_id: Uuid,
    volume_fingerprint: VolumeFingerprint,
},
```

## Testing Strategy

### Integration Tests

1. **Volume Tracking Test** (`tests/volume_tracking_test.rs`):
```rust
#[tokio::test]
async fn test_volume_tracking_persistence() {
    let core = create_test_core().await;
    let library = create_test_library(&core).await;
    
    // Track a volume
    let volume = core.volumes.get_all_volumes().await.first().cloned().unwrap();
    let tracked = core.volumes.track_volume(
        &library,
        &volume.fingerprint,
        Some("Test Volume".to_string())
    ).await.unwrap();
    
    // Verify it's tracked
    let tracked_volumes = core.volumes.get_tracked_volumes(&library).await.unwrap();
    assert_eq!(tracked_volumes.len(), 1);
    assert_eq!(tracked_volumes[0].fingerprint, volume.fingerprint.0);
    
    // Untrack
    core.volumes.untrack_volume(&library, &volume.fingerprint).await.unwrap();
    
    // Verify it's untracked
    let tracked_volumes = core.volumes.get_tracked_volumes(&library).await.unwrap();
    assert_eq!(tracked_volumes.len(), 0);
}
```

## Migration Notes

1. The existing `VolumeManager` has TODO comments where database operations should go
2. The event system is already set up for volume events
3. The action system is ready for the volume actions
4. Follow the existing patterns in `LocationManager` for similar functionality

## Next Steps

1. Create the database migration
2. Create the SeaORM entity
3. Implement the database methods in VolumeManager
4. Update the action handlers
5. Add the volume monitor service
6. Write integration tests

## ActionOutput Design Note

The current implementation uses a centralized `ActionOutput` enum for all action results. This design decision has been investigated and the following findings were documented:

### Current State
- All action handlers return `ActionResult<ActionOutput>`
- ActionOutput serves multiple purposes:
  - Provides standardized return type for all actions
  - Gets serialized to JSON for audit logs (`result_payload`)
  - Gets returned to CLI via `DaemonResponse::ActionOutput`
  - Has both specific variants (VolumeTracked, VolumeUntracked, etc.) and a generic Custom variant

### Design Pattern
- Most actions define their own output struct implementing `ActionOutputTrait`
- They use `ActionOutput::from_trait()` to convert to the centralized enum
- This provides type safety while allowing flexibility

### Trade-offs

**Pros:**
- Centralized enum makes it easy to handle all outputs uniformly in infrastructure code
- Audit logging can serialize any action output
- CLI can display any action output consistently
- The `Custom` variant provides an escape hatch for actions that don't need specific handling

**Cons:**
- Central enum needs updating for each new action type
- Could become a maintenance burden as more actions are added
- Goes against open/closed principle

### Recommendation
The current approach is reasonable because:
1. It's already implemented and working across the codebase
2. Provides good type safety and pattern matching
3. Makes audit logging straightforward
4. The volume actions follow this established pattern with specific variants (VolumeTracked, VolumeUntracked, VolumeSpeedTested)

Any future refactoring to remove the centralized enum would require changes to:
- Audit log serialization
- CLI response handling
- Any code that pattern matches on specific output types