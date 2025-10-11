<!--CREATED: 2025-10-11-->
## **Design Document: Spacedrive Sync Conduits**

### 1\. Overview

This document specifies the design and implementation plan for **Sync Conduits**, a system for synchronizing file content between user-defined points within the Spacedrive VDFS. This feature is distinct from **Library Sync**, which is the separate, underlying process for replicating the VDFS index and its associated metadata. Sync Conduits provide users with explicit, transparent, and configurable control over how the physical file content is mirrored, backed up, or managed across different storage locations.

### 2\. Core Concepts

#### 2.1. Sync Conduit

A **Sync Conduit** is the central concept. It is a durable, long-running job that represents a user-configured synchronization relationship between a **source Entry** and a **destination Entry**. Linking the conduit to an `Entry` rather than a `Location` provides maximum flexibility, allowing users to sync any directory without formally adding it as a managed `Location`.

#### 2.2. State-Based Reconciliation

The sync mechanism will use a **state-based reconciliation** model. Instead of replaying a log of events, the system periodically compares the live filesystem state of the source and destination against the VDFS index. This approach is resilient to offline changes and naturally compresses multiple intermediate operations (e.g., create -\> modify -\> delete) into a single, final state, significantly optimizing performance.

### 3\. Use Cases & Sync Policies

Users can create a Sync Conduit with one of four distinct policies, each designed for a specific use case.

#### 3.1. Replicate (One-Way Mirror)

  * **Use Case**: Creating robust, automated backups of critical data. A photographer wants to automatically back up her `Active Projects` folder from her laptop's fast SSD to her large, archival NAS. She needs new photos and edits to be copied over automatically, and if she deletes a photo from her active folder, it should also be removed from the backup to keep it clean.
  * **Methodology**: The conduit monitors the source `Entry`. It propagates all creates, modifies, and (optionally) deletes from the source to the destination. The destination becomes a perfect mirror of the source.

#### 3.2. Synchronize (Two-Way)

  * **Use Case**: Keeping directories identical for working across multiple machines. A developer works on a project from a desktop PC at home and a laptop on the go. He needs the project folder to be identical on both machines, so changes made on his laptop during the day are available on his desktop in the evening, and vice-versa.
  * **Methodology**: The conduit monitors both `Entries` and syncs changes bidirectionally. Conflict resolution uses a "last-writer-wins" strategy based on the file's modification timestamp.

#### 3.3. Offload (Smart Cache)

  * **Use Case**: Freeing up space on a primary device with limited storage. A video editor works on a laptop with a small SSD but has a large home server. She wants to keep only recently accessed project files locally. Older files should be moved to the server to free up space, but their `Entry` must remain in the VDFS index so they are still searchable and can be retrieved on demand.
  * **Methodology**: The conduit uses the `VolumeManager` to monitor free space on the source volume. When a user-defined threshold is met, it moves the least recently used files (based on the `Entry`'s `accessed_at` timestamp) to the destination. Files can be pinned with a "Pinned" tag to prevent offloading.

#### 3.4. Archive (Move and Consolidate)

  * **Use Case**: Moving completed work to long-term storage and safely reclaiming space. A researcher finishes a data analysis project and wants to move the entire folder to a long-term archival drive. The transfer must be cryptographically verified before the original files are deleted from her workstation.
  * **Methodology**: The conduit executes a `FileCopyJob` with `delete_after_copy` enabled. It leverages the **Commit-Then-Verify** step to ensure the file was transferred with perfect integrity before deleting the source copy.

### 4\. Architectural Methodology

#### 4.1. The Sync Lifecycle

1.  **Trigger**: Initiated by the `LocationWatcher` service or a timer (`Sync Cadence`).
2.  **Delta Calculation**: The `SyncConduitJob` performs a live scan of source and destination filesystems. The VDFS index is used as a high-performance cache to quickly identify unchanged files. The result is an ephemeral list of `COPY` and `DELETE` operations.
3.  **Execution**: The job dispatches `FileCopyAction` and `FileDeleteAction` operations to the durable job system.
4.  **Verification**: After transfer, a **Commit-Then-Verify (CTV)** step is initiated via a `ValidationRequest` to the destination, which confirms the file's BLAKE3 hash.
5.  **Completion**: Once all actions are verified, the sync cycle is complete.

#### 4.2. Sync Cadence (Action Compression)

Each Sync Conduit has a configurable **Sync Frequency** (e.g., Instantly, Every 5 Minutes). Because the system reconciles state rather than replaying an event log, any series of changes within the time window are naturally compressed. If a file is created, modified, and then deleted within a 5-minute window, the sync job will see that the file doesn't exist at the start and end of the window and will perform **no action**.

### 5\. Detailed Implementation Plan

#### 5.1. Database Schema Changes

A new migration file will be created in `./src/infra/db/migration/` to add the `sync_relationships` table.

```rust
// In a new migration file, e.g., mYYYYMMDD_HHMMSS_create_sync_relationships.rs

#[derive(DeriveIden)]
enum SyncRelationships {
    Table, Id, Uuid, SourceEntryId, DestinationEntryId, Policy, PolicyConfig,
    Status, IsEnabled, LastSyncAt, CreatedAt, UpdatedAt,
}

// In the up() function:
manager.create_table(
    Table::create()
        .table(SyncRelationships::Table)
        .if_not_exists()
        .col(ColumnDef::new(SyncRelationships::Id).integer().not_null().auto_increment().primary_key())
        .col(ColumnDef::new(SyncRelationships::Uuid).uuid().not_null().unique_key())
        .col(ColumnDef::new(SyncRelationships::SourceEntryId).integer().not_null())
        .col(ColumnDef::new(SyncRelationships::DestinationEntryId).integer().not_null())
        .col(ColumnDef::new(SyncRelationships::Policy).string().not_null())
        .col(ColumnDef::new(SyncRelationships::PolicyConfig).json().not_null())
        .col(ColumnDef::new(SyncRelationships::Status).string().not_null().default("idle"))
        .col(ColumnDef::new(SyncRelationships::IsEnabled).boolean().not_null().default(true))
        .col(ColumnDef::new(SyncRelationships::LastSyncAt).timestamp_with_time_zone())
        .col(ColumnDef::new(SyncRelationships::CreatedAt).timestamp_with_time_zone().not_null())
        .col(ColumnDef::new(SyncRelationships::UpdatedAt).timestamp_with_time_zone().not_null())
        .foreign_key(
            ForeignKey::create()
                .from(SyncRelationships::Table, SyncRelationships::SourceEntryId)
                .to(entities::entry::Entity, entities::entry::Column::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .from(SyncRelationships::Table, SyncRelationships::DestinationEntryId)
                .to(entities::entry::Entity, entities::entry::Column::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned(),
).await?;
```

*An associated `Entity` and `ActiveModel` will be created in `./src/infra/db/entities/`.*

#### 5.2. New Modules and Structs

A new module will be created at `src/ops/sync/`.

##### 5.2.1. Job Definition (`src/ops/sync/job.rs`)

```rust
use serde::{Deserialize, Serialize};
use crate::infra::job::prelude::*;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct SyncConduitJob {
    pub sync_conduit_uuid: uuid::Uuid,
    // Internal state for resumption (e.g., current file being processed)
}

impl Job for SyncConduitJob {
    const NAME: &'static str = "sync_conduit";
    const RESUMABLE: bool = true;
}

#[async_trait::async_trait]
impl JobHandler for SyncConduitJob {
    type Output = SyncOutput; // Defined in src/ops/sync/output.rs

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Core sync logic will be implemented here
        unimplemented!()
    }
}
```

##### 5.2.2. Actions (`src/ops/sync/action.rs`)

New `LibraryAction`s will be created for managing conduits.

**Input for Create Action (`src/ops/sync/input.rs`):**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConduitCreateInput {
    pub source_entry_id: i32,
    pub destination_entry_id: i32,
    pub policy: String, // "replicate", "synchronize", etc.
    pub policy_config: serde_json::Value, // For policy-specific settings like cadence
}
```

#### 5.3. Networking Protocol

The `file_transfer` protocol will be extended with messages for the CTV step.

```rust
// In src/service/network/protocol/file_transfer.rs
enum FileTransferMessage {
    // ... existing messages
    ValidationRequest {
        transfer_id: Uuid,
        destination_path: String,
    },
    ValidationResponse {
        transfer_id: Uuid,
        is_valid: bool,
        blake3_hash: Option<String>,
        error: Option<String>,
    },
}
```

#### 5.4. Modifications to Existing Systems

  * **`FileCopyJob`**: Add a `Verifying` state to its state machine. After a file is transferred, it will enter this state, send a `ValidationRequest`, and await a `ValidationResponse` before moving to `Completed`.
  * **`LocationWatcher`**: The event handler will be updated to check if a filesystem event occurred within an `Entry` managed by a Sync Conduit. If the cadence allows, it will trigger a `SyncConduitJob`.

### 6\. User Experience (UX) Flow

1.  A user right-clicks on a directory in the Spacedrive UI.
2.  They select a new "Sync To..." option.
3.  A dialog appears, allowing them to select a destination directory.
4.  The user chooses a **Sync Policy** (e.g., Replicate) and configures its options (e.g., Sync Cadence).
5.  Upon confirmation, a `SyncConduitCreateAction` is dispatched, creating the **Sync Conduit**.
6.  The UI displays the active conduit in a dedicated "Sync Status" panel, showing its policy, status, and last sync time.
