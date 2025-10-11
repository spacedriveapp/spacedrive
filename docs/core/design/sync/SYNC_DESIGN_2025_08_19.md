# Pragmatic Sync System Design (2025-08-19 Revision)

## Overview

This document outlines the new sync system for Spacedrive Core v2 that prioritizes pragmatism over theoretical perfection. The system is built on Spacedrive's service and job architecture, focusing on three distinct sync domains: **index sync** (filesystem mirroring), **user metadata sync** (tags, ratings), and **file operations** (separate from sync).

## Sync Domain Separation

Spacedrive distinguishes between three separate data synchronization concerns:

### 1. Index Sync (Filesystem Mirror)

- **Purpose**: Mirror each device's local filesystem index and file-specific metadata
- **Data**: Entry records (with `parent_id`), device-specific paths, file-level tags, location metadata
- **Conflicts**: Minimal - each device owns its filesystem index exclusively
- **Transport**: Via the live sync service and dedicated backfill jobs over the networking layer
- **Source of Truth**: Local filesystem watcher events

> The `Entry` records, including their `parent_id` relationships, are the source of truth for the filesystem hierarchy. Derived data structures like the `entry_closure` table are explicitly excluded from sync and are rebuilt locally on each device. This minimizes sync traffic and prevents complex conflicts.

### 2. User Metadata Sync (Library Content)

- **Purpose**: Sync content-universal metadata across all instances of the same content within a library
- **Data**: Content-level tags, ContentIdentity metadata, library-scoped favorites
- **Conflicts**: Possible - multiple users can tag the same content simultaneously
- **Resolution**: Union merge for content tags, deterministic ContentIdentity UUIDs prevent most conflicts
- **Transport**: Real-time sync via the live service + batch jobs for backfill

### 3. File Operations (Remote Operations)

- **Purpose**: Actual file transfer, copying, and cross-device movement
- **Protocol**: Separate from sync - uses dedicated file transfer protocol
- **Trigger**: User-initiated operations (Spacedrop, cross-device copy/move)
- **Relationship**: File operations trigger filesystem changes → watcher events → index sync

> **Key Insight**: Index sync is largely conflict-free because devices only modify their own filesystem indices. User metadata sync operates on library-scoped ContentIdentity, enabling content-universal tagging that follows the content across devices within the same library.

## Core Principles

1.  **Universal Dependency Awareness** - Every sync operation automatically respects foreign key constraints and dependency order
2.  **Jobs for Finite Tasks, Services for Long-Running Processes** - Finite tasks (`Backfill`) are durable, resumable jobs. Continuous operations (`LiveSync`) are persistent background services.
3.  **Networking Integration** - Built on the persistent networking layer with automatic device connection management
4.  **Library-Scoped ContentIdentity** - Content is addressable within each library via deterministic UUIDs derived from content_id hash
5.  **Dual Tagging System** - Users can tag individual files (Entry-level) or all instances of content (ContentIdentity-level)
6.  **Domain Separation** - Index, user metadata, and file operations are distinct protocols with different conflict resolution
7.  **One Leader Per Library** - Each library has a designated leader device that maintains the sync log
8.  **Hybrid Change Tracking** - SeaORM hooks with async queuing + event system for comprehensive coverage
9.  **Intelligent Conflicts** - Union merge for content tags, deterministic UUIDs prevent ContentIdentity conflicts
10. **Sync Readiness** - UUIDs optional until content identification complete, preventing premature sync of incomplete data
11. **Declarative Dependencies** - Simple `depends_on = ["location", "device"]` syntax with automatic circular resolution
12. **Derived Data is Not Synced** - Derived data, such as the closure table for hierarchical queries, is not synced directly. Each device rebuilds it locally from the synced source of truth (e.g., parent-child relationships), ensuring efficiency and consistency.
13. **Privacy through Log Redaction & Compaction** - The sync log on the leader is not permanent. A background process will periodically redact sensitive data from deleted records and compact the log by creating snapshots to preserve privacy and save space.

## Architecture

The architecture separates finite, resumable **Jobs** from persistent, long-running **Services**.

-   **Jobs** (`BackfillSyncJob`): Have a clear start and end. They are queued and executed by the Job Manager. They are perfect for bringing a device up-to-date.
-   **Services** (`LiveSyncService`): A singleton process that runs for the entire application lifecycle. It listens for real-time changes and can queue Jobs when needed.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Library A (Photos)                           │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │ Leader: Device 1│     │Follower: Device 2│                   │
│  │ ┌─────────────┐ │     │ ┌─────────────┐ │                   │
│  │ │Phase 1:     │ │     │ │Phase 1:     │ │                   │
│  │ │CAPTURE      │ │     │ │CAPTURE      │ │                   │
│  │ │(SeaORM hooks)│ │     │ │(SeaORM hooks)│ │                   │
│  │ └─────────────┘ │     │ └─────────────┘ │                   │
│  │ ┌─────────────┐ │     │ ┌─────────────┐ │                   │
│  │ │Phase 2:     │ │────│ │Phase 3:     │ │                   │
│  │ │STORE        │ │     │ │INGEST       │ │                   │
│  │ │(Dependency  │ │     │ │(Buffer &    │ │                   │
│  │ │ ordering)   │ │     │ │ reorder)    │ │                   │
│  │ └─────────────┘ │     │ └─────────────┘ │                   │
│  │ ┌─────────────┐ │     │ ┌─────────────┐ │                   │
│  │ │  Sync Log   │ │     │ │  Local DB   │ │                   │
│  │ │  Networking │ │     │ │  Networking │ │                   │
│  │ └─────────────┘ │     │ └─────────────┘ │                   │
│  └─────────────────┘     └─────────────────┘                   │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation

### 1. Sync Jobs & Services

#### Backfill & Setup Jobs

Finite operations like the initial sync for a device or a catch-up backfill are implemented as Jobs. They are queued by the system when a new device pairs or an existing device comes online after a long time.

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct BackfillSyncJob {
    pub library_id: Uuid,
    pub target_device_id: Uuid,
    // ... other options
}

impl Job for BackfillSyncJob {
    const NAME: &'static str = "backfill_sync";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Backfills historical sync data from a peer.");
}

// ... JobHandler implementation for BackfillSyncJob
```

#### Live Sync Service (Long-Running Process)

The long-running process of handling real-time changes is modeled as a `Service`, aligning with the existing architectural pattern for persistent background processes. It is managed by the application's core service container.

```rust
use crate::core::services::Service; // Assuming this is the path to the trait

pub struct LiveSyncService {
    // context, state, etc.
    is_running: Arc<AtomicBool>,
    // Handle to the job manager to queue backfills
    job_manager: Arc<JobManager>,
}

impl LiveSyncService {
    pub fn new(context: Arc<CoreContext>) -> Self {
        // ... initialization
    }
}

#[async_trait::async_trait]
impl Service for LiveSyncService {
    fn name(&self) -> &'static str {
        "live_sync_service"
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    async fn start(&self) -> Result<()> {
        self.is_running.store(true, Ordering::SeqCst);
        // Spawn the main loop as a background Tokio task
        // This loop listens on the event bus and network for changes.
        // It can queue jobs like BackfillSyncJob when needed.
        tokio::spawn(async move {
            // ... loop { ... }
        });
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        // Signal the background task to gracefully shut down
        Ok(())
    }
}
```

### 2. Universal Dependency-Aware Sync Trait

Every syncable domain model implements a simple trait with built-in dependency awareness:

```rust
#[async_trait]
pub trait Syncable: ActiveModelTrait {
    /// Unique sync identifier for this model type
    const SYNC_ID: &'static str;

    /// Sync domain (Index, UserMetadata, or None for no sync)
    const SYNC_DOMAIN: SyncDomain;

    /// Dependencies - models that must be synced before this one
    const DEPENDENCIES: &'static [&'static str] = &[];

    /// Sync priority within dependency level (0 = highest priority)
    const SYNC_PRIORITY: u8 = 50;

    /// Whether this model should sync at all (includes UUID readiness check)
    fn should_sync(&self) -> bool;

    /// Custom merge logic for conflicts
    fn merge(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model>;

    // ... other helper methods and associated enums ...
}
```

### 3. Three-Phase Sync Architecture

The sync system operates in three distinct phases, each with different dependency handling requirements:

#### Phase 1: Creating Sync Operations (Local Change Capture)

When changes occur locally, we capture them without dependency ordering concerns:

```rust
impl ActiveModelBehavior for EntryActiveModel {
    fn after_save(self, insert: bool) -> Result<Self, DbErr> {
        // PHASE 1: CAPTURE - No dependency ordering needed yet
        if <EntryActiveModel as Syncable>::should_sync(&self) {
            // Queue change in memory for async processing
            SYNC_QUEUE.queue_change(/* ... */);
        }
        Ok(self)
    }
}
```

#### Phase 2 & 3: Storing and Ingesting (Service Logic)

The logic for storing changes (on the leader) and ingesting them (on followers) is handled within the `LiveSyncService`. 

On the leader device, the service's main loop processes the queue of captured changes, resolves their dependencies, and writes them to the persistent `SyncLog`. On follower devices, the service's main loop polls the leader for new log entries and applies them locally, buffering them as needed to ensure dependencies are met even with out-of-order network delivery.

```rust
// Example logic within the LiveSyncService on a LEADER device
async fn leader_loop(&self) {
    loop {
        let captured_changes = SYNC_QUEUE.drain_pending();
        if !captured_changes.is_empty() {
            // PHASE 2: Apply dependency ordering and store to sync log
            let dependency_batches = SYNC_REGISTRY.batch_changes_by_dependencies(captured_changes);

            for batch in dependency_batches {
                self.store_dependency_batch(batch).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// Example logic within the LiveSyncService on a FOLLOWER device
async fn follower_loop(&self) {
    loop {
        // Poll leader for changes since last sequence
        if let Ok(changes) = self.pull_changes_from_leader().await {
            // PHASE 3: Buffer and apply changes in dependency order
            self.ingest_changes(changes).await;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

### 4. Sync Log Structure

Domain-aware append-only log on the leader device:

```rust
pub struct SyncLogEntry {
    /// Auto-incrementing sequence number
    pub seq: u64,
    pub library_id: Uuid,
    pub domain: SyncDomain,
    pub timestamp: DateTime<Utc>,
    pub device_id: Uuid,
    pub model_type: String,
    pub record_id: String,
    pub change_type: ChangeType,
    pub data: Option<Vec<u8>>, // Encrypted JSON payload
    pub was_sync_ready: bool,
}

pub enum ChangeType {
    Upsert,
    Delete,
}
```

### 5. Sync Protocol (Networking Integration)

Built on the existing networking message protocol:

```rust
// Sync messages integrated into DeviceMessage enum
pub enum DeviceMessage {
    // ... existing messages ...

    // Sync protocol messages
    SyncPullRequest { /* ... */ },
    SyncPullResponse { /* ... */ },
    SyncChange { /* ... */ },
}
```

(The rest of the document continues with model definitions and other details which remain conceptually unchanged from the original design).