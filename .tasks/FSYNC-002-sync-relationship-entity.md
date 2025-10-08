---
id: FSYNC-002
title: SyncRelationship Entity & ActiveModel
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, database, seaorm]
depends_on: [FSYNC-001]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the SeaORM Entity and ActiveModel for the `sync_relationships` table, providing a type-safe interface for managing sync conduits.

## Implementation Notes

Create `src/infra/db/entities/sync_relationship.rs`:

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_relationships")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub source_entry_id: i32,
    pub destination_entry_id: i32,
    pub policy: String, // "replicate", "synchronize", "offload", "archive"
    pub policy_config: serde_json::Value,
    pub status: String, // "idle", "syncing", "paused", "error"
    pub is_enabled: bool,
    pub last_sync_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::SourceEntryId",
        to = "super::entry::Column::Id"
    )]
    SourceEntry,
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::DestinationEntryId",
        to = "super::entry::Column::Id"
    )]
    DestinationEntry,
}

impl ActiveModelBehavior for ActiveModel {}
```

## Acceptance Criteria

- [ ] Entity created in `src/infra/db/entities/sync_relationship.rs`
- [ ] Proper relations to `entry::Entity` defined
- [ ] Enum types for `policy` and `status` created
- [ ] Export added to `src/infra/db/entities/mod.rs`
- [ ] Compiles without errors

