# Pragmatic Sync System Design

## Overview

This document outlines the new sync system for Spacedrive Core v2 that prioritizes pragmatism over theoretical perfection. The system is built on Spacedrive's job architecture and networking infrastructure, focusing on three distinct sync domains: **index sync** (filesystem mirroring), **user metadata sync** (tags, ratings), and **file operations** (separate from sync).

## Sync Domain Separation

Spacedrive distinguishes between three separate data synchronization concerns:

### 1\. Index Sync (Filesystem Mirror)

- **Purpose**: Mirror each device's local filesystem index and file-specific metadata
- **Data**: Entry records, device-specific paths, file-level tags, location metadata
- **Conflicts**: Minimal - each device owns its filesystem index exclusively
- **Transport**: Via sync jobs over the networking layer
- **Source of Truth**: Local filesystem watcher events

### 2\. User Metadata Sync (Library Content)

- **Purpose**: Sync content-universal metadata across all instances of the same content within a library
- **Data**: Content-level tags, ContentIdentity metadata, library-scoped favorites
- **Conflicts**: Possible - multiple users can tag the same content simultaneously
- **Resolution**: Union merge for content tags, deterministic ContentIdentity UUIDs prevent most conflicts
- **Transport**: Real-time sync via networking + batch jobs for backfill

### 3\. File Operations (Remote Operations)

- **Purpose**: Actual file transfer, copying, and cross-device movement
- **Protocol**: Separate from sync - uses dedicated file transfer protocol
- **Trigger**: User-initiated operations (Spacedrop, cross-device copy/move)
- **Relationship**: File operations trigger filesystem changes → watcher events → index sync

> **Key Insight**: Index sync is largely conflict-free because devices only modify their own filesystem indices. User metadata sync operates on library-scoped ContentIdentity, enabling content-universal tagging that follows the content across devices within the same library.

## Core Principles

1.  **Universal Dependency Awareness** - Every sync operation automatically respects foreign key constraints and dependency order
2.  **Job-Based Architecture** - All sync operations run as Spacedrive jobs with progress tracking, resumability, and error handling
3.  **Networking Integration** - Built on the persistent networking layer with automatic device connection management
4.  **Library-Scoped ContentIdentity** - Content is addressable within each library via deterministic UUIDs derived from content_id hash
5.  **Dual Tagging System** - Users can tag individual files (Entry-level) or all instances of content (ContentIdentity-level)
6.  **Domain Separation** - Index, user metadata, and file operations are distinct protocols with different conflict resolution
7.  **One Leader Per Library** - Each library has a designated leader device that maintains the sync log
8.  **Hybrid Change Tracking** - SeaORM hooks with async queuing + event system for comprehensive coverage
9.  **Intelligent Conflicts** - Union merge for content tags, deterministic UUIDs prevent ContentIdentity conflicts
10. **Sync Readiness** - UUIDs optional until content identification complete, preventing premature sync of incomplete data
11. **Declarative Dependencies** - Simple `depends_on = ["location", "device"]` syntax with automatic circular resolution

## Architecture

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
│  │ │Phase 2:     │ │────▶│ │Phase 3:     │ │                   │
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

┌─────────────────────────────────────────────────────────────────┐
│                  Library Setup & Merging                        │
│                                                                 │
│  Device A (Photos.sdlibrary)    Device B (Documents.sdlibrary) │
│         │                              │                       │
│         └─────── User Choice ──────────┘                       │
│                      │                                         │
│            ┌─────────▼─────────┐                               │
│            │  Sync Setup UI    │                               │
│            │ - Choose leader   │                               │
│            │ - Merge libraries │                               │
│            │ - Sync settings   │                               │
│            └─────────┬─────────┘                               │
│                      │                                         │
│              ┌───────▼───────┐                                 │
│              │ Merged Library │                                 │
│              │   + Sync Jobs  │                                 │
│              └───────────────┘                                 │
└─────────────────────────────────────────────────────────────────┘

Each library can have a different leader device. When enabling sync between
devices with existing libraries, users choose to merge or keep separate.
```

## Implementation

### 1\. Job-Based Sync Architecture

All sync operations are implemented as Spacedrive jobs, providing automatic progress tracking, resumability, and error handling:

#### Initial Sync Job

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct InitialSyncJob {
    pub library_id: Uuid,
    pub target_device_id: Uuid,
    pub sync_options: SyncOptions,

    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<InitialSyncState>,
}

impl Job for InitialSyncJob {
    const NAME: &'static str = "initial_sync";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Initial synchronization with paired device");
}

#[async_trait::async_trait]
impl JobHandler for InitialSyncJob {
    type Output = SyncOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Phase 1: Establish connection
        let networking = ctx.networking_service()
            .ok_or(JobError::Other("Networking not available".into()))?;

        // Phase 2: Exchange sync metadata
        ctx.progress(Progress::message("Exchanging sync metadata"));
        let remote_seq = self.negotiate_sync_position(&networking).await?;

        // Phase 3: Pull changes from leader (they're already in dependency order)
        ctx.progress(Progress::percentage(0.1));
        self.pull_changes_from_leader(&ctx, &networking, remote_seq).await?;

        // Phase 4: Apply changes using follower ingest phase (buffer and reorder)
        ctx.progress(Progress::percentage(0.8));
        self.apply_changes_with_follower_buffering(&ctx).await?;

        ctx.checkpoint().await?;
        Ok(self.generate_output())
    }
}
```

#### Live Sync Job

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct LiveSyncJob {
    pub library_id: Uuid,
    pub device_ids: Vec<Uuid>,

    #[serde(skip)]
    state: Option<LiveSyncState>,
}

impl Job for LiveSyncJob {
    const NAME: &'static str = "live_sync";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Continuous synchronization with connected devices");
}

// Runs continuously, processes real-time sync messages
```

### 2\. Universal Dependency-Aware Sync Trait

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

    /// Which fields should be synced (None = all fields)
    fn sync_fields() -> Option<Vec<&'static str>> {
        None // Sync all fields by default
    }

    /// Get sync domain dynamically (for models with conditional domains)
    fn get_sync_domain(&self) -> SyncDomain {
        Self::SYNC_DOMAIN
    }

    /// Custom merge logic for conflicts
    fn merge(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model> {
        match Self::SYNC_DOMAIN {
            SyncDomain::Index => MergeResult::NoConflict(remote), // Device owns its index
            SyncDomain::UserMetadata => Self::merge_user_metadata(local, remote),
            SyncDomain::None => MergeResult::NoConflict(local), // Shouldn't happen
        }
    }

    /// Whether this model should sync at all (includes UUID readiness check)
    fn should_sync(&self) -> bool {
        self.get_sync_domain() != SyncDomain::None
    }

    /// Handle circular dependencies (override for special cases)
    fn resolve_circular_dependency() -> Option<CircularResolution> {
        None
    }
}

/// Strategy for resolving circular dependencies
#[derive(Debug, Clone)]
pub enum CircularResolution {
    /// Create without these fields, update later
    OmitFields(Vec<&'static str>),
    /// Use nullable foreign key, update after dependency sync
    NullableReference(&'static str),
}

pub enum SyncDomain {
    None,         // No sync (temp files, device-specific data)
    Index,        // Filesystem index (device-owned, no conflicts)
    UserMetadata, // Cross-device user data (potential conflicts)
}

pub enum MergeResult<T> {
    NoConflict(T),
    Merged(T),
    Conflict(T, T, ConflictType),
}
```

### 3\. Library Sync Setup & Merging

When users enable sync between two devices, the system handles existing libraries intelligently:

#### Sync Enablement Workflow

```rust
pub struct SyncSetupJob {
    pub local_device_id: Uuid,
    pub remote_device_id: Uuid,
    pub setup_options: SyncSetupOptions,
}

pub struct SyncSetupOptions {
    pub action: LibraryAction,
    pub conflict_resolution: ConflictResolution,
    pub sync_enabled_types: Vec<SyncDomain>,
}

pub enum LibraryAction {
    /// Merge remote library into local (local becomes leader)
    MergeIntoLocal { remote_library_id: Uuid },
    /// Merge local library into remote (remote becomes leader)
    MergeIntoRemote { local_library_id: Uuid },
    /// Create new shared library (choose leader)
    CreateShared { leader_device_id: Uuid, name: String },
    /// Keep libraries separate, sync only user metadata
    SyncMetadataOnly {
        local_library_id: Uuid,
        remote_library_id: Uuid
    },
}
```

#### Library Merging Process

```rust
impl SyncSetupJob {
    async fn merge_libraries(&mut self, ctx: JobContext<'_>) -> JobResult<Library> {
        match &self.setup_options.action {
            LibraryAction::MergeIntoLocal { remote_library_id } => {
                // 1. Export remote library data with device mapping
                ctx.progress(Progress::message("Exporting remote library data"));
                let remote_data = self.export_library_data(*remote_library_id).await?;

                // 2. Merge into local library
                ctx.progress(Progress::percentage(0.3));
                self.merge_library_data(&ctx, remote_data).await?;

                // 3. Deduplicate files by CAS ID
                ctx.progress(Progress::percentage(0.6));
                self.deduplicate_files(&ctx).await?;

                // 4. Reconcile device records and sync roles
                ctx.progress(Progress::percentage(0.8));
                self.reconcile_devices(&ctx).await?;

                // 5. Start sync jobs
                self.start_sync_jobs(&ctx).await?;

                Ok(ctx.library().clone())
            }
            // ... other merge strategies
        }
    }
}
```

### 4\. Networking Integration

Sync jobs leverage the persistent networking layer for device communication:

```rust
impl InitialSyncJob {
    async fn pull_changes_from_leader(
        &mut self,
        ctx: &JobContext<'_>,
        networking: &NetworkingService,
        from_seq: u64
    ) -> JobResult<()> {
        // Use existing networking message protocol
        let pull_request = DeviceMessage::SyncPullRequest {
            library_id: self.library_id,
            from_seq,
            limit: Some(1000),
            domains: vec![SyncDomain::Index, SyncDomain::UserMetadata],
        };

        let response = networking.send_to_device(
            self.target_device_id,
            pull_request
        ).await?;

        if let DeviceMessage::SyncPullResponse { changes, latest_seq } = response {
            // Store received changes for follower processing
            // (Changes from leader are already in dependency order)
            for change in changes {
                self.received_changes.push(change);
            }

            // Update sync position
            self.update_sync_position(latest_seq).await?;
        }

        Ok(())
    }

    async fn apply_changes_with_follower_buffering(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        // Use the follower ingest phase to apply buffered changes
        let mut follower_service = SyncFollowerService::new();

        for change in &self.received_changes {
            // This handles out-of-order delivery and dependency buffering
            follower_service.receive_sync_change(change.seq, change.clone()).await?;
        }

        Ok(())
    }
}
```

### 5\. Three-Phase Sync Architecture

The sync system operates in three distinct phases, each with different dependency handling requirements:

#### Phase 1: Creating Sync Operations (Local Change Capture)

When changes occur locally, we capture them without dependency ordering concerns:

```rust
impl ActiveModelBehavior for EntryActiveModel {
    fn after_save(self, insert: bool) -> Result<Self, DbErr> {
        // PHASE 1: CAPTURE - No dependency ordering needed yet
        // Just record that a change happened, don't worry about order
        if <EntryActiveModel as Syncable>::should_sync(&self) && self.uuid.as_ref().is_some() {
            let change_type = if insert {
                ChangeType::Insert
            } else {
                ChangeType::Update
            };

            // Queue change in memory for async processing (synchronous operation)
            SYNC_QUEUE.queue_change(SyncChange {
                model_type: Entry::SYNC_ID,
                domain: self.get_sync_domain(),
                record_id: self.uuid.clone().unwrap(),
                change_type,
                data: serde_json::to_value(&self).ok(),
                timestamp: Utc::now(),
                was_sync_ready: true,
                // NOTE: No dependency ordering at capture time
            });
        }
        Ok(self)
    }
}
```

#### Phase 2: Storing Sync Operations (Leader Log Management)

The leader device processes captured changes and stores them in dependency order:

```rust
pub struct SyncLeaderService {
    sync_log: SyncLog,
    dependency_resolver: DependencyResolver,
}

impl SyncLeaderService {
    /// PHASE 2: STORE - Apply dependency ordering when writing to the leader log
    pub async fn process_captured_changes(&self, changes: Vec<SyncChange>) -> Result<()> {
        // Group changes by dependency level
        let batched_changes = self.dependency_resolver.batch_by_dependencies(changes);

        // Write to sync log in dependency order with proper sequence numbers
        for batch in batched_changes {
            // Within each dependency level, we can process in parallel
            let futures: Vec<_> = batch.priority_order.iter().map(|model_id| {
                let model_changes = batch.get_changes_for_model(model_id);
                self.write_model_changes_to_log(model_changes)
            }).collect();

            // Wait for entire dependency level to complete before moving to next
            futures::future::try_join_all(futures).await?;
        }

        Ok(())
    }

    async fn write_model_changes_to_log(&self, changes: Vec<SyncChange>) -> Result<()> {
        for change in changes {
            // Handle circular dependencies during log storage
            let processed_change = if let Some(resolution) = change.get_circular_resolution() {
                self.apply_circular_resolution_to_log_entry(change, resolution).await?
            } else {
                change
            };

            // Assign sequence number and persist to leader log
            let seq = self.sync_log.append(processed_change).await?;

            // Broadcast to followers immediately (they'll apply in their own dependency order)
            self.broadcast_change_to_followers(seq, processed_change).await?;
        }
        Ok(())
    }
}
```

#### Phase 3: Ingesting Sync Operations (Follower Application)

Followers receive changes and must apply them in dependency order, even if they arrive out of order:

```rust
pub struct SyncFollowerService {
    pending_changes: BTreeMap<u64, SyncChange>, // Buffer for out-of-order changes
    dependency_resolver: DependencyResolver,
    last_applied_seq: u64,
}

impl SyncFollowerService {
    /// PHASE 3: INGEST - Apply dependency ordering when consuming from the leader log
    pub async fn receive_sync_change(&mut self, seq: u64, change: SyncChange) -> Result<()> {
        // Buffer the change - don't apply immediately
        self.pending_changes.insert(seq, change);

        // Try to apply as many consecutive changes as possible in dependency order
        self.try_apply_pending_changes().await
    }

    async fn try_apply_pending_changes(&mut self) -> Result<()> {
        // Collect consecutive changes we can apply
        let mut applicable_changes = Vec::new();
        let mut next_seq = self.last_applied_seq + 1;

        while let Some(change) = self.pending_changes.remove(&next_seq) {
            applicable_changes.push(change);
            next_seq += 1;
        }

        if applicable_changes.is_empty() {
            return Ok(()); // Nothing to apply yet
        }

        // CRITICAL: Re-order changes by dependency graph before applying
        let dependency_batches = self.dependency_resolver.batch_by_dependencies(applicable_changes);

        // Apply each dependency level in order
        for batch in dependency_batches {
            self.apply_dependency_batch(batch).await?;
        }

        self.last_applied_seq = next_seq - 1;
        Ok(())
    }

    async fn apply_dependency_batch(&self, batch: SyncBatch) -> Result<()> {
        // Within a dependency level, apply changes in priority order
        for model_id in &batch.priority_order {
            let changes = batch.get_changes_for_model(model_id);

            for change in changes {
                // Apply individual change with circular dependency handling
                if let Some(resolution) = change.get_circular_resolution() {
                    self.apply_change_with_circular_resolution(change, resolution).await?;
                } else {
                    self.apply_change_directly(change).await?;
                }
            }
        }
        Ok(())
    }
}
```

### Key Differences Between Phases

The three phases have fundamentally different requirements:

| Phase       | Dependency Ordering | Performance Priority | Error Handling          |
| ----------- | ------------------- | -------------------- | ----------------------- |
| **Capture** | Not needed          | Minimal latency      | Never fail              |
| **Store**   | Required            | Consistency          | Retry with backoff      |
| **Ingest**  | Critical            | Batch efficiency     | Out-of-order resilience |

#### Why This Separation Matters

**1. Capture Phase Simplicity:**

- Must be synchronous and fast (called from SeaORM hooks)
- Can't afford dependency graph calculations
- Just records "something changed" without ordering

**2. Leader Store Phase Consistency:**

- Can be asynchronous and more expensive
- Must establish canonical dependency order
- Handles circular dependency resolution once
- Assigns authoritative sequence numbers

**3. Follower Ingest Phase Resilience:**

- Must handle network delays and out-of-order delivery
- Re-applies dependency ordering on received changes
- Buffers changes until dependencies are satisfied

#### Example: Creating an Entry with UserMetadata

```rust
// Phase 1: CAPTURE (happens synchronously in transaction)
let entry = EntryActiveModel.insert(db).await?;
// -> Queues SyncChange for "entry" (no dependency ordering)

let metadata = UserMetadataActiveModel {
    entry_uuid: entry.uuid,
}.insert(db).await?;
// -> Queues SyncChange for "user_metadata" (no dependency ordering)

// Phase 2: STORE (happens asynchronously on leader)
// Leader processes queue and discovers:
// - Entry depends on: ["location", "content_identity"]
// - UserMetadata depends on: ["entry", "content_identity"]
// - UserMetadata has circular resolution: entry.metadata_id nullable
//
// Leader writes to sync log:
// Seq 100: Device record (no deps)
// Seq 101: Location record (depends on device)
// Seq 102: ContentIdentity record (no deps)
// Seq 103: Entry record with metadata_id=null (circular resolution)
// Seq 104: UserMetadata record
// Seq 105: Entry update with metadata_id=<uuid> (circular resolution completion)

// Phase 3: INGEST (happens on followers)
// Follower receives changes possibly out of order:
// Receives seq 104 (UserMetadata) before seq 103 (Entry)
// -> Buffers UserMetadata until Entry is applied
// -> Applies in dependency order regardless of receipt order

// SyncLeaderJob processes captured changes on leader device (Phase 2: STORE)
impl SyncLeaderJob {
async fn process*captured_changes(&mut self, ctx: JobContext<'*>) -> JobResult<()> {
// Collect all pending changes from capture phase
let captured_changes = SYNC_QUEUE.drain_pending();

        if !captured_changes.is_empty() {
            // PHASE 2: Apply dependency ordering and store to sync log
            let dependency_batches = SYNC_REGISTRY.batch_changes_by_dependencies(captured_changes);

            // Process each dependency batch in order
            for batch in dependency_batches {
                self.store_dependency_batch(&ctx, batch).await?;
            }

            ctx.checkpoint().await?;
        }
        Ok(())
    }

    async fn store_dependency_batch(&mut self, ctx: &JobContext<'_>, batch: SyncBatch) -> JobResult<()> {
        // Within each dependency level, we can process in parallel
        let futures: Vec<_> = batch.priority_order.iter().map(|model_id| {
            let model_changes = batch.get_changes_for_model(model_id);
            self.store_model_changes_to_log(model_changes)
        }).collect();

        // Wait for entire dependency level to complete before moving to next
        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    async fn store_model_changes_to_log(&self, changes: Vec<SyncChange>) -> JobResult<()> {
        for change in changes {
            // Handle circular dependencies during log storage
            let processed_change = if let Some(resolution) = change.get_circular_resolution() {
                self.apply_circular_resolution_to_log_entry(change, resolution).await?
            } else {
                change
            };

            // Assign sequence number and persist to leader sync log
            let seq = self.sync_log.append(processed_change.clone()).await?;

            // Broadcast to followers immediately (they'll buffer and reorder)
            self.broadcast_change_to_followers(seq, processed_change).await?;
        }
        Ok(())
    }

}

// SyncFollowerJob ingests changes from leader (Phase 3: INGEST)
impl SyncFollowerJob {
async fn process*received_changes(&mut self, ctx: JobContext<'*>) -> JobResult<()> {
// PHASE 3: Buffer and apply changes in dependency order
// (Uses the SyncFollowerService from the three-phase architecture)

        while let Some((seq, change)) = self.receive_change_from_leader().await? {
            self.follower_service.receive_sync_change(seq, change).await?;
        }

        ctx.checkpoint().await?;
        Ok(())
    }

}
```

### 6\. Sync Log Structure

Domain-aware append-only log on the leader device:

```rust
pub struct SyncLogEntry {
    /// Auto-incrementing sequence number
    pub seq: u64,

    /// Which library this change belongs to
    pub library_id: Uuid,

    /// Sync domain for conflict resolution strategy
    pub domain: SyncDomain,

    /// When this change occurred
    pub timestamp: DateTime<Utc>,

    /// Which device made the change
    pub device_id: Uuid,

    /// Model type identifier
    pub model_type: String,

    /// Record identifier (UUID for models that have it)
    pub record_id: String,

    /// Type of change
    pub change_type: ChangeType,

    /// Serialized model data (JSON)
    pub data: Option<serde_json::Value>,

    /// Whether this record had UUID at time of change (sync readiness)
    pub was_sync_ready: bool,
}

pub enum ChangeType {
    Upsert, // Insert or Update
    Delete,
}
```

### 7\. Sync Protocol (Networking Integration)

Built on the existing networking message protocol:

```rust
// Sync messages integrated into DeviceMessage enum
pub enum DeviceMessage {
    // ... existing messages ...

    // Sync protocol messages
    SyncPullRequest {
        library_id: Uuid,
        from_seq: u64,
        limit: Option<usize>,
        domains: Vec<SyncDomain>, // Filter by domain
    },

    SyncPullResponse {
        library_id: Uuid,
        changes: Vec<SyncLogEntry>,
        latest_seq: u64,
    },

    // Real-time sync messages
    SyncChange {
        library_id: Uuid,
        change: SyncLogEntry,
    },

    // Library merging protocol
    LibraryMergeRequest {
        source_library_id: Uuid,
        target_library_id: Uuid,
        merge_strategy: LibraryAction,
    },

    LibraryMergeResponse {
        success: bool,
        merged_library_id: Option<Uuid>,
        conflicts: Vec<MergeConflict>,
    },
}
```

### 8\. Model Examples with Elegant Dependency Declarations

#### Device (Independent)

```rust
impl Syncable for device::ActiveModel {
    const SYNC_ID: &'static str = "device";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;
    // No dependencies - devices sync first
}
```

#### Tag (Independent)

```rust
impl Syncable for tag::ActiveModel {
    const SYNC_ID: &'static str = "tag";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::UserMetadata;
    // No dependencies - tag definitions sync early
}
```

#### ContentIdentity (Independent within Library)

```rust
impl Syncable for content_identity::ActiveModel {
    const SYNC_ID: &'static str = "content_identity";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::UserMetadata;
    // No dependencies - deterministic UUIDs prevent conflicts within library

    fn should_sync(&self) -> bool {
        // Only sync after content identification assigns UUID
        self.uuid.as_ref().is_some()
    }
}
```

#### Location (Depends on Device)

```rust
impl Syncable for location::ActiveModel {
    const SYNC_ID: &'static str = "location";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;
    const DEPENDENCIES: &'static [&'static str] = &["device"];

    // location.device_id -> device.id
}
```

#### Entry (Depends on Location, Optional ContentIdentity)

```rust
impl Syncable for entry::ActiveModel {
    const SYNC_ID: &'static str = "entry";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;
    const DEPENDENCIES: &'static [&'static str] = &["location", "content_identity"];

    fn should_sync(&self) -> bool {
        // Only sync entries that have UUID assigned (content identification complete or immediate assignment)
        self.uuid.as_ref().is_some()
    }

    fn resolve_circular_dependency() -> Option<CircularResolution> {
        // Handle Entry ↔ UserMetadata circular reference
        Some(CircularResolution::NullableReference("metadata_id"))
    }
}
```

#### UserMetadata (Depends on Entry OR ContentIdentity + Circular Resolution)

```rust
impl Syncable for user_metadata::ActiveModel {
    const SYNC_ID: &'static str = "user_metadata";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::None; // Dynamic based on scope
    const DEPENDENCIES: &'static [&'static str] = &["entry", "content_identity"];

    fn should_sync(&self) -> bool {
        // Must have one UUID set to be syncable
        self.entry_uuid.as_ref().is_some() || self.content_identity_uuid.as_ref().is_some()
    }

    fn get_sync_domain(&self) -> SyncDomain {
        match (self.entry_uuid.as_ref(), self.content_identity_uuid.as_ref()) {
            (Some(_), None) => SyncDomain::Index,     // Entry-scoped
            (None, Some(_)) => SyncDomain::UserMetadata, // Content-scoped
            _ => SyncDomain::None
        }
    }

    fn resolve_circular_dependency() -> Option<CircularResolution> {
        // Will be created after entries exist (circular reference resolved by nullable entry.metadata_id)
        None
    }
}
```

#### UserMetadataTag Junction (Depends on UserMetadata + Tag)

```rust
impl Syncable for user_metadata_tag::ActiveModel {
    const SYNC_ID: &'static str = "user_metadata_tag";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::None; // Inherits domain from parent UserMetadata
    const DEPENDENCIES: &'static [&'static str] = &["user_metadata", "tag"];
    const SYNC_PRIORITY: u8 = 90; // Low priority - sync after main entities

    fn get_sync_domain(&self) -> SyncDomain {
        // Domain determined by parent UserMetadata scope:
        // - Entry-scoped metadata tags sync in Index domain
        // - Content-scoped metadata tags sync in UserMetadata domain
        // This is resolved during sync by looking up the UserMetadata
        SyncDomain::UserMetadata // Default to UserMetadata domain
    }
}
```

#### Library-Scoped ContentIdentity (Deterministic within Library)

```rust
impl Syncable for content_identity::ActiveModel {
    const SYNC_ID: &'static str = "content_identity";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::UserMetadata;

    fn should_sync(&self) -> bool {
        // Only sync ContentIdentity that has UUID assigned (content identification complete)
        Self::SYNC_DOMAIN != SyncDomain::None && self.uuid.as_ref().is_some()
    }

    fn merge_user_metadata(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model> {
        // ContentIdentity UUIDs are deterministic from content_hash + library_id
        // This ensures same content in different libraries has different UUIDs
        // Maintains library isolation while enabling deterministic sync
        if local.uuid != remote.uuid {
            return MergeResult::Conflict(
                local, remote,
                ConflictType::InvalidState("ContentIdentity UUID mismatch")
            );
        }

        // Merge statistics from both devices within this library
        MergeResult::Merged(Self::Model {
            entry_count: local.entry_count + remote.entry_count,
            total_size: local.total_size, // Same content = same size
            first_seen_at: std::cmp::min(local.first_seen_at, remote.first_seen_at),
            last_verified_at: std::cmp::max(local.last_verified_at, remote.last_verified_at),
            ..local
        })
    }
}
```

#### Tags (via UserMetadata Junction Table)

```rust
impl Syncable for user_metadata_tag::ActiveModel {
    const SYNC_ID: &'static str = "user_metadata_tag";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::None; // Syncs with parent UserMetadata

    // Tags sync as part of their parent UserMetadata
    // The domain (Index or UserMetadata) depends on the UserMetadata scope:
    // - Entry-scoped UserMetadata tags sync in Index domain
    // - Content-scoped UserMetadata tags sync in UserMetadata domain

    // Examples of entry-scoped tags: "desktop-shortcut", "work-presentation-draft"
    // Examples of content-scoped tags: "family-photos", "important-documents"
}
```

#### Tag Entity

```rust
impl Syncable for tag::ActiveModel {
    const SYNC_ID: &'static str = "tag";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::UserMetadata;

    // Tag definitions sync across all devices
    // The actual tag applications sync via UserMetadata relationships
}
```

#### UserMetadata (Hierarchical Scoping)

```rust
impl Syncable for user_metadata::ActiveModel {
    const SYNC_ID: &'static str = "user_metadata";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::None; // Default, overridden by get_sync_domain

    fn should_sync(&self) -> bool {
        // Has to have one UUID set to be syncable
        self.entry_uuid.as_ref().is_some() || self.content_identity_uuid.as_ref().is_some()
    }

    fn get_sync_domain(&self) -> SyncDomain {
        match (self.entry_uuid.as_ref(), self.content_identity_uuid.as_ref()) {
            (Some(_), None) => SyncDomain::Index,
            (None, Some(_)) => SyncDomain::UserMetadata,
            _ => SyncDomain::None
        }
    }

    fn merge(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model> {
        // Determine domain dynamically
        let domain = match (&local.entry_uuid, &local.content_identity_uuid) {
            (Some(_), None) => SyncDomain::Index,
            (None, Some(_)) => SyncDomain::UserMetadata,
            _ => return MergeResult::Conflict(local, remote, ConflictType::InvalidState("Invalid UUID state"))
        };

        match domain {
            SyncDomain::Index => MergeResult::NoConflict(remote), // Device owns entry metadata
            SyncDomain::UserMetadata => Self::merge_user_metadata(local, remote),
            _ => unreachable!()
        }
    }

    fn sync_fields() -> Option<Vec<&'static str>> {
        Some(vec![
            "entry_uuid",           // Entry-scoped metadata
            "content_identity_uuid", // Content-scoped metadata
            "notes",                // User notes
            "favorite",             // Favorite status
            "hidden",               // Hidden status
            "custom_data",          // Custom metadata
        ])
    }

    fn merge_user_metadata(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model> {
        // Intelligent merge for content-scoped metadata
        // Notes: keep both (displayed in hierarchy)
        // Tags: union merge via junction table
        // Favorites/Hidden: OR logic (true if either is true)
        MergeResult::Merged(Self::Model {
            favorite: local.favorite || remote.favorite,
            hidden: local.hidden || remote.hidden,
            notes: merge_notes(local.notes, remote.notes), // Keep both with timestamps
            custom_data: merge_custom_data(local.custom_data, remote.custom_data),
            updated_at: std::cmp::max(local.updated_at, remote.updated_at),
            ..local
        })
    }

    // UserMetadata can be scoped to either Entry or ContentIdentity
    // Only created when user adds notes/favorites/custom data
    // Mutual exclusivity enforced by database constraints
}
```

#### Location (Index Domain)

```rust
impl Syncable for location::ActiveModel {
    const SYNC_ID: &'static str = "location";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;

    fn sync_fields() -> Option<Vec<&'static str>> {
        Some(vec![
            "name",
            "path",
            "is_tracked",
            "display_name",
            "color",
            "icon",
        ])
        // Excludes device-specific: mount_point, available_space, is_mounted
    }
}
```

#### No Sync (TempFile)

```rust
impl Syncable for temp_file::ActiveModel {
    const SYNC_ID: &'static str = "temp_file";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::None;

    // Temp files never sync
}
```

## Sync Process

### Leader Device (Per Library)

1.  **Check Leadership**: Verify this device is the leader for the library
2.  **Capture Changes**: SeaORM hooks automatically log all changes
3.  **Serve Log**: Expose sync log via API/P2P protocol
4.  **Maintain State**: Track each device's sync position

### Follower Device

1.  **Find Leader**: Query which device is the leader for this library
2.  **Pull Changes**: Request changes since last sync from the leader
3.  **Apply Changes**: Process in order, using merge logic for conflicts
4.  **Track Position**: Remember last processed sequence number

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

### Initial Sync & Backfill Strategy

#### Full Backfill for New Devices

When a new device joins a library, it needs to backfill all existing data:

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct BackfillSyncJob {
    pub library_id: Uuid,
    pub leader_device_id: Uuid,
    pub backfill_strategy: BackfillStrategy,

    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<BackfillState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BackfillStrategy {
    /// Full backfill from sequence 0
    Full,
    /// Backfill only sync-ready entities (have UUIDs)
    SyncReadyOnly,
    /// Incremental backfill from last known position
    Incremental { from_seq: u64 },
}

#[derive(Debug, Serialize, Deserialize)]
struct BackfillState {
    current_seq: u64,
    target_seq: u64,
    processed_models: HashSet<String>,
    failed_records: Vec<FailedRecord>,
}

impl JobHandler for BackfillSyncJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        match &self.backfill_strategy {
            BackfillStrategy::Full => {
                self.full_backfill(&ctx).await?
            }
            BackfillStrategy::SyncReadyOnly => {
                self.sync_ready_backfill(&ctx).await?
            }
            BackfillStrategy::Incremental { from_seq } => {
                self.incremental_backfill(&ctx, *from_seq).await?
            }
        }
    }
}

impl BackfillSyncJob {
    async fn full_backfill(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        let networking = ctx.networking_service()
            .ok_or(JobError::Other("Networking not available".into()))?;

        // 1. Get current leader sequence number
        ctx.progress(Progress::message("Getting sync position from leader"));
        let target_seq = networking.get_latest_seq(self.leader_device_id, self.library_id).await?;

        // 2. Backfill all entities from sequence 0
        ctx.progress(Progress::message("Starting full backfill"));
        let mut current_seq = 0;
        let batch_size = 1000;

        while current_seq < target_seq {
            // Pull batch of changes
            let batch = networking.pull_changes(
                self.leader_device_id,
                self.library_id,
                current_seq,
                Some(batch_size)
            ).await?;

            // Apply changes with dependency ordering and error recovery
            let batched_changes = SYNC_REGISTRY.batch_changes_by_dependencies(batch.changes);

            for dep_batch in batched_changes {
                if let Err(e) = self.apply_batch_with_circular_resolution(dep_batch, ctx).await {
                    // Log failed batch but continue
                    self.state.as_mut().unwrap().failed_records.push(FailedRecord {
                        seq: current_seq,
                        model_type: "batch".to_string(),
                        record_id: format!("seq_{}", current_seq),
                        error: e.to_string(),
                    });
                }
            }

            current_seq = batch.latest_seq + 1;

            // Update progress
            let progress = (current_seq as f64 / target_seq as f64) * 100.0;
            ctx.progress(Progress::percentage(progress / 100.0));

            // Save checkpoint for resumability
            ctx.checkpoint().await?;
        }

        // 3. Save final sync position
        self.save_sync_position(target_seq).await?;

        // 4. Report any failed records
        if !self.state.as_ref().unwrap().failed_records.is_empty() {
            ctx.progress(Progress::message("Backfill completed with some failures"));
        } else {
            ctx.progress(Progress::message("Backfill completed successfully"));
        }

        Ok(())
    }

    async fn sync_ready_backfill(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        // Only backfill entities that have UUIDs (are sync-ready)
        // This is faster but may miss some data

        let sync_ready_entities = self.get_sync_ready_entities().await?;

        // Process entities in dependency order automatically
        let sync_order = SYNC_REGISTRY.get_sync_order();
        for batch in sync_order {
            for entity_type in &batch.models {
                ctx.progress(Progress::message(&format!("Backfilling {}", entity_type)));

                let entities = sync_ready_entities.get(*entity_type).unwrap_or(&Vec::new());

                for (i, entity_uuid) in entities.iter().enumerate() {
                    if let Err(e) = self.request_entity_from_leader(entity_type, entity_uuid).await {
                        // Log but continue
                        tracing::warn!(
                            "Failed to backfill {} {}: {}",
                            entity_type, entity_uuid, e
                        );
                    }

                    // Progress update
                    let progress = (i as f64 / entities.len() as f64) * 100.0;
                    ctx.progress(Progress::percentage(progress / 100.0));
                }
            }
        }

        Ok(())
    }

    async fn incremental_backfill(&mut self, ctx: &JobContext<'_>, from_seq: u64) -> JobResult<()> {
        // Similar to full_backfill but starts from a specific sequence
        // Used when a device has been offline and needs to catch up

        let networking = ctx.networking_service()
            .ok_or(JobError::Other("Networking not available".into()))?;

        let target_seq = networking.get_latest_seq(self.leader_device_id, self.library_id).await?;

        if from_seq >= target_seq {
            ctx.progress(Progress::message("Already up to date"));
            return Ok(());
        }

        ctx.progress(Progress::message(&format!(
            "Catching up from seq {} to {}", from_seq, target_seq
        )));

        // Use same batching logic as full_backfill
        self.batch_sync_from_sequence(from_seq, target_seq, ctx).await?;

        Ok(())
    }
}
```

#### Handling Pre-Sync Entries

For existing entries without UUIDs (created before sync was enabled):

```rust
// The indexer handles UUID assignment during normal operation
// No separate backfill job needed - just re-index locations

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct SyncReadinessJob {
    pub library_id: Uuid,
    pub location_ids: Vec<i32>,
}

impl JobHandler for SyncReadinessJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Trigger re-indexing of specified locations
        // This will assign UUIDs to entries as part of normal indexing flow

        for location_id in &self.location_ids {
            ctx.progress(Progress::message(&format!(
                "Re-indexing location {} for sync readiness", location_id
            )));

            // Schedule indexer job for this location
            let indexer_job = IndexerJob::new(
                *location_id,
                IndexMode::Metadata, // Just metadata, UUIDs assigned based on rules
                IndexScope::Recursive,
            );

            ctx.job_manager().queue(indexer_job).await?;
        }

        ctx.progress(Progress::message("Sync readiness jobs queued"));

        Ok(SyncReadinessOutput {
            locations_queued: self.location_ids.len(),
        })
    }
}

// The indexer will assign UUIDs according to the rules:
// - Directories: UUID assigned immediately
// - Empty files: UUID assigned immediately
// - Regular files: UUID assigned after content identification
// - No separate "backfill" needed - it's part of normal indexing
```

#### Backfill Scenarios Summary

1.  **New Device Joins**: Full backfill from sequence 0
2.  **Device Reconnects**: Incremental backfill from last known sequence
3.  **Sync Log Gaps**: Detect missing sequences and request specific ranges
4.  **Pre-Sync Data**: Re-index locations to assign UUIDs (not a separate backfill)
5.  **Failed Sync Operations**: Retry mechanism with exponential backoff

#### Sync Position Tracking

```rust
pub struct SyncPosition {
    pub device_id: Uuid,
    pub library_id: Uuid,
    pub last_seq: u64,
    pub updated_at: DateTime<Utc>,
    pub backfill_complete: bool,
}

// Track what each device has synced
impl SyncPositionManager {
    /// Get the last sequence a device has processed
    pub async fn get_device_position(
        &self,
        device_id: Uuid,
        library_id: Uuid
    ) -> Result<Option<u64>> {
        let position = SyncPosition::find()
            .filter(sync_position::Column::DeviceId.eq(device_id))
            .filter(sync_position::Column::LibraryId.eq(library_id))
            .one(&self.db)
            .await?;

        Ok(position.map(|p| p.last_seq))
    }

    /// Detect if a device needs backfill
    pub async fn needs_backfill(
        &self,
        device_id: Uuid,
        library_id: Uuid,
        current_leader_seq: u64
    ) -> Result<BackfillStrategy> {
        match self.get_device_position(device_id, library_id).await? {
            None => {
                // New device - needs full backfill
                Ok(BackfillStrategy::Full)
            }
            Some(last_seq) if last_seq == 0 => {
                // Never synced - needs full backfill
                Ok(BackfillStrategy::Full)
            }
            Some(last_seq) if last_seq < current_leader_seq => {
                // Behind - needs incremental backfill
                Ok(BackfillStrategy::Incremental { from_seq: last_seq + 1 })
            }
            Some(_) => {
                // Up to date - no backfill needed
                Ok(BackfillStrategy::SyncReadyOnly) // Just verify sync-ready entities
            }
        }
    }

    /// Update device sync position
    pub async fn update_position(
        &self,
        device_id: Uuid,
        library_id: Uuid,
        seq: u64
    ) -> Result<()> {
        let position = SyncPositionActiveModel {
            device_id: Set(device_id),
            library_id: Set(library_id),
            last_seq: Set(seq),
            updated_at: Set(Utc::now()),
            backfill_complete: Set(true),
        };

        // Upsert the position
        SyncPosition::insert(position)
            .on_conflict(
                OnConflict::columns([sync_position::Column::DeviceId, sync_position::Column::LibraryId])
                    .update_columns([
                        sync_position::Column::LastSeq,
                        sync_position::Column::UpdatedAt,
                        sync_position::Column::BackfillComplete,
                    ])
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }
}
```

### Universal Dependency-Aware Sync (Built Into Core Protocol)

The sync system automatically builds dependency graphs from model declarations and **always** syncs in dependency order. No special jobs or configurations needed.

#### Automatic Dependency Resolution

```rust
/// The sync registry automatically builds dependency graphs from Syncable trait implementations
pub struct SyncRegistry {
    models: HashMap<&'static str, Box<dyn SyncableInfo>>,
    dependency_graph: DependencyGraph,
}

impl SyncRegistry {
    /// Register all syncable models and build dependency graph
    pub fn initialize() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
        };

        // Auto-register all models (via macro or runtime registration)
        registry.register::<device::ActiveModel>();
        registry.register::<location::ActiveModel>();
        registry.register::<entry::ActiveModel>();
        registry.register::<content_identity::ActiveModel>();
        registry.register::<user_metadata::ActiveModel>();
        registry.register::<tag::ActiveModel>();
        registry.register::<user_metadata_tag::ActiveModel>();

        // Build dependency graph from declarations
        registry.build_dependency_graph();

        registry
    }

    /// Get sync order for all registered models (always dependency-aware)
    pub fn get_sync_order(&self) -> Vec<SyncBatch> {
        self.dependency_graph.topological_sort()
    }
}

#[derive(Debug, Clone)]
pub struct SyncBatch {
    pub models: Vec<&'static str>,
    pub priority_order: Vec<&'static str>, // Within batch, sorted by SYNC_PRIORITY
    pub circular_resolution: Vec<CircularResolution>,
}

/// Every sync operation automatically uses dependency order
impl SyncProtocol {
    /// Pull changes in dependency order (default behavior)
    pub async fn pull_changes(&self, from_seq: u64) -> Result<SyncResponse> {
        let sync_batches = SYNC_REGISTRY.get_sync_order();
        let mut all_changes = Vec::new();

        // Pull changes batch by batch in dependency order
        for batch in sync_batches {
            let batch_changes = self.pull_batch_changes(batch, from_seq).await?;
            all_changes.extend(batch_changes);
        }

        Ok(SyncResponse {
            changes: all_changes,
            dependency_ordered: true, // Always true now
        })
    }

    /// Apply changes respecting dependencies (automatic)
    pub async fn apply_changes(&self, changes: Vec<SyncChange>) -> Result<()> {
        // Group changes by dependency level
        let batched_changes = SYNC_REGISTRY.batch_changes_by_dependencies(changes);

        // Apply in dependency order with transaction safety
        for batch in batched_changes {
            self.apply_batch_with_circular_resolution(batch).await?;
        }

        Ok(())
    }
}
```

#### Automatic Circular Dependency Resolution

The sync system automatically resolves circular dependencies using the model declarations:

```rust
impl SyncProtocol {
    /// Apply a batch with automatic circular dependency resolution
    async fn apply_batch_with_circular_resolution(&self, batch: SyncBatch) -> Result<()> {
        // Apply models in priority order within batch
        for model_id in &batch.priority_order {
            let changes = batch.get_changes_for_model(model_id);

            if let Some(resolution) = self.get_circular_resolution(model_id) {
                self.apply_with_circular_resolution(changes, resolution).await?;
            } else {
                self.apply_changes_directly(changes).await?;
            }
        }

        // After batch is complete, apply any deferred updates (like nullable references)
        self.apply_deferred_updates(batch).await?;

        Ok(())
    }

    async fn apply_with_circular_resolution(
        &self,
        changes: Vec<SyncChange>,
        resolution: CircularResolution
    ) -> Result<()> {
        match resolution {
            CircularResolution::NullableReference(field) => {
                // For Entry ↔ UserMetadata: create entries without metadata_id, update later
                for change in changes {
                    let mut data = change.data.clone();

                    // Temporarily set nullable field to None
                    if let Some(obj) = data.as_mut().and_then(|d| d.as_object_mut()) {
                        obj.insert(field.to_string(), serde_json::Value::Null);
                    }

                    self.apply_change_with_data(change, data).await?;
                }
            }
            CircularResolution::OmitFields(fields) => {
                // Create records without certain fields, update later
                for change in changes {
                    let mut data = change.data.clone();

                    // Remove specified fields
                    if let Some(obj) = data.as_mut().and_then(|d| d.as_object_mut()) {
                        for field in &fields {
                            obj.remove(*field);
                        }
                    }

                    self.apply_change_with_data(change, data).await?;
                }
            }
        }

        Ok(())
    }
}
```

#### Transaction Safety for Dependency Chains

```rust
// Ensure entire dependency chain is applied atomically
pub async fn apply_dependency_chain(
    changes: Vec<SyncChange>,
    db: &DatabaseConnection,
) -> Result<()> {
    // Group changes by dependency level
    let grouped_changes = group_changes_by_dependency(changes);

    // Apply in transaction to ensure consistency
    db.transaction(|txn| async move {
        for dependency_level in grouped_changes {
            for change in dependency_level {
                apply_single_change(change, txn).await?;
            }
        }
        Ok(())
    }).await?;

    Ok(())
}

fn group_changes_by_dependency(changes: Vec<SyncChange>) -> Vec<SyncBatch> {
    // Use the global sync registry for consistent dependency ordering
    SYNC_REGISTRY.batch_changes_by_dependencies(changes)
}
```

#### Sync Protocol Enhancement

```rust
// Enhanced sync protocol with universal dependency awareness
pub enum SyncRequest {
    PullChanges {
        library_id: Uuid,
        from_seq: u64,
        limit: Option<usize>,
        models: Option<Vec<String>>, // Allow filtering by model type
        // dependency_aware: true by default - always respects dependencies
    },
    PullModelBatch {
        library_id: Uuid,
        model_type: String,
        from_seq: u64,
        limit: Option<usize>,
    },
}

pub enum SyncResponse {
    ChangesResponse {
        changes: Vec<SyncLogEntry>,
        latest_seq: u64,
        dependency_ordered: bool, // Always true - changes are always in dependency order
    },
    ModelBatchResponse {
        model_type: String,
        changes: Vec<SyncLogEntry>,
        has_more: bool,
    },
}

// DependencyMetadata no longer needed - dependency ordering is automatic and universal
```

### File Change Sync Behavior

When file content changes (as described in ENTITY_REFACTOR_DESIGN.md):

1.  **Entry UUID preserved** - Maintains sync continuity
2.  **Entry-scoped metadata preserved** - Continues to sync in Index domain
3.  **Content link cleared** - `content_id = None` propagates via sync
4.  **Content-scoped metadata orphaned** - No longer referenced by this entry
5.  **New content identification** - Creates new ContentIdentity with new UUID

This ensures sync system handles the unlinking gracefully without losing entry-level data.

## Conflict Resolution Strategies

### Index Domain (Minimal Conflicts)

```
Device A: Creates entry for /photos/vacation.jpg
Device B: Creates entry for /docs/vacation.jpg (same content, different path)
Result: No conflict - different devices, different entries, same ContentIdentity
```

### Entry-Scoped Tags (Device-Specific)

```
Device A: Creates UserMetadata for "photo.jpg" with tags ["desktop-wallpaper"] (Entry-scoped)
Device B: Tags same file with ["screensaver"] via its own Entry (Entry-scoped)
Result: Device A sees ["desktop-wallpaper"], Device B sees ["screensaver"]
```

### Content-Scoped Tags (Union Merge)

```
Device A: Creates UserMetadata for content with tags ["vacation"] (Content-scoped)
Device B: Tags same content with ["family"] (Content-scoped)
Result: Both devices see content tagged with ["vacation", "family"]
```

### ContentIdentity Statistics (Additive Merge)

```
Device A: ContentIdentity has 2 entries, 10MB total (only after content identification assigns UUID)
Device B: ContentIdentity has 3 entries, 15MB total (only after content identification assigns UUID)
Result: ContentIdentity shows 5 entries, 25MB total across devices
```

### True Conflicts (Rare)

```
Device A: Sets UserMetadata notes="Important document" for entry X
Device B: Sets UserMetadata notes="Draft version" for same entry X
Result: Conflict prompt - keep which notes? (should be rare due to device ownership)
```

### File Content Changes

```
Device A: User adds UserMetadata to "report.pdf" with tag "important" (Entry-scoped)
Device A: User edits report.pdf (content changes → new ContentIdentity UUID)
Device B: Syncs changes
Result: Entry-scoped metadata with tag "important" preserved, any content-scoped metadata lost
```

## Advantages of Universal Dependency-Aware Sync Design

### Core Sync Features

1.  **Sync Safety**: UUID assignment during content identification prevents race conditions and incomplete data sync
2.  **Content-Universal Metadata**: Tag content once, appears everywhere that content exists within the library
3.  **Conflict-Free Content Identity**: Deterministic UUIDs prevent ContentIdentity conflicts within libraries
4.  **Dual Tagging System**: Users choose between file-specific tags (follow the file) and content-universal tags (follow the content)
5.  **Hierarchical Metadata**: UserMetadata supports both entry-scoped and content-scoped organization
6.  **Library Isolation**: Maintains Spacedrive's zero-knowledge principle between libraries
7.  **Clean Domain Separation**: Index sync vs content metadata sync have different conflict strategies

### Universal Dependency Management

8.  **Built-In Dependency Awareness**: Every sync operation automatically respects foreign key constraints
9.  **Declarative Dependencies**: Simple `depends_on = ["location", "device"]` syntax in model definitions
10. **Automatic Circular Resolution**: Entry ↔ UserMetadata and other circular dependencies resolved transparently
11. **Three-Phase Architecture**: Capture (no ordering), Store (dependency ordering), Ingest (out-of-order resilience)
12. **Developer Experience**: Adding sync to a model takes 3 lines with the derive macro
13. **Compile-Time Safety**: Dependencies declared at compile time, validated during sync system initialization
14. **Priority-Based Ordering**: `SYNC_PRIORITY` allows fine-grained control within dependency levels

### Technical Excellence

15. **Job-Based Reliability**: All sync operations benefit from progress tracking and resumability
16. **Transport Agnostic**: Works over any connection (HTTP, WebSocket, P2P)
17. **Incremental Sync**: Can sync partially, resume after interruption
18. **Backward Compatible**: Builds on existing hybrid ID system without breaking changes
19. **Comprehensive Change Capture**: SeaORM hooks ensure no database changes are missed
20. **Performance**: In-memory queuing minimizes sync overhead during normal operations
21. **Efficient Deduplication**: Accurate library-scoped statistics for storage optimization

### Simplicity & Maintainability

23. **Zero Configuration**: Sync system builds dependency graph automatically from model declarations
24. **Self-Documenting**: Dependencies are visible in the model definition, not hidden in separate files
25. **Consistent Behavior**: All sync operations follow the same dependency-aware pattern
26. **Reduced Complexity**: No separate dependency-aware sync jobs, batching logic, or coordination code
27. **Easy Testing**: Dependency order is deterministic and can be unit tested
28. **Preserves UX Patterns**: UserMetadata stays optional, tags work before/during/after indexing

## Migration Path

1.  **Phase 1**: Implement sync traits on core models
2.  **Phase 2**: Implement hybrid change tracking (SeaORM hooks + in-memory queue + transaction flushing)
3.  **Phase 3**: Build simple HTTP-based sync for testing
4.  **Phase 4**: Add P2P transport when ready
5.  **Phase 5**: Consider multi-leader for advanced users

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
// 1. Add to domain model with dependency declaration
#[derive(DeriveEntityModel, Syncable)]
#[sea_orm(table_name = "locations")]
#[sync(id = "location", domain = "Index", depends_on = ["device"])]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub device_id: Uuid, // Dependency: must sync after device
    pub updated_at: DateTime<Utc>,
}

// 2. That's it! Sync happens automatically with dependency ordering
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

## Hybrid Change Tracking: SeaORM Hooks + Async Processing

### Why Hybrid Approach

We use both SeaORM hooks and event system for comprehensive change tracking:

1.  **SeaORM Hooks**: Automatic capture - impossible to miss database changes
2.  **In-Memory Queue**: Bridge between sync hooks and async processing
3.  **Event System**: Manual control for complex scenarios and transaction boundaries
4.  **Transaction Safety**: Flush queues at transaction boundaries to prevent data loss

### In-Memory Sync Queue

```rust
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

// Global sync queue for collecting changes from SeaORM hooks
static SYNC_QUEUE: Lazy<SyncQueue> = Lazy::new(|| SyncQueue::new());

pub struct SyncQueue {
    pending_changes: Arc<Mutex<Vec<SyncChange>>>,
}

impl SyncQueue {
    pub fn new() -> Self {
        Self {
            pending_changes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queue a change from SeaORM hook (synchronous)
    pub fn queue_change(&self, change: SyncChange) {
        if let Ok(mut pending) = self.pending_changes.lock() {
            pending.push(change);
        }
    }

    /// Drain pending changes for async processing
    pub fn drain_pending(&self) -> Vec<SyncChange> {
        if let Ok(mut pending) = self.pending_changes.lock() {
            pending.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Flush queue at transaction boundaries (prevents data loss)
    pub async fn flush_for_transaction(&self, db: &DatabaseConnection) -> Result<()> {
        let changes = self.drain_pending();

        if !changes.is_empty() {
            // Persist to sync log immediately
            for change in changes {
                self.persist_sync_change(change, db).await?;
            }
        }
        Ok(())
    }

    async fn persist_sync_change(&self, change: SyncChange, db: &DatabaseConnection) -> Result<()> {
        let sync_entry = SyncLogEntryActiveModel {
            library_id: Set(change.library_id),
            domain: Set(change.domain),
            timestamp: Set(change.timestamp),
            device_id: Set(change.device_id),
            model_type: Set(change.model_type),
            record_id: Set(change.record_id),
            change_type: Set(change.change_type),
            data: Set(change.data),
            was_sync_ready: Set(change.was_sync_ready),
        };

        sync_entry.insert(db).await?;
        Ok(())
    }
}
```

### Transaction-Aware Database Operations

```rust
// Enhanced database operations with sync queue flushing
pub async fn create_entry_with_sync(
    entry_data: EntryData,
    db: &DatabaseConnection,
) -> Result<Entry> {
    let entry = db.transaction(|txn| async move {
        // Create entry (SeaORM hook will queue sync change)
        let entry = EntryActiveModel {
            // ... entry fields
        }.insert(txn).await?;

        // Flush sync queue at transaction boundary
        SYNC_QUEUE.flush_for_transaction(txn).await?;

        Ok(entry)
    }).await?;

    Ok(entry)
}
```

### Event System for Complex Scenarios

```rust
// Event system for scenarios requiring manual control
pub enum CoreEvent {
    // Sync-specific events
    SyncQueueFlushRequested { library_id: Uuid },
    EntryContentIdentified { library_id: Uuid, entry_uuid: Uuid },
    ContentChangeDetected { library_id: Uuid, entry_uuid: Uuid, old_content_id: Option<i32> },
}

// Use events for complex scenarios
pub async fn handle_content_identification(
    entry: &mut Entry,
    content_identity: ContentIdentity,
    events: &EventBus,
) -> Result<()> {
    // Update entry (hook will queue basic change)
    entry.content_id = Some(content_identity.id);
    entry.uuid = Some(Uuid::new_v4()); // Now sync-ready!
    entry.update(db).await?;

    // Emit event for additional processing
    events.emit(CoreEvent::EntryContentIdentified {
        library_id,
        entry_uuid: entry.uuid.unwrap(),
    }).await?;

    Ok(())
}
```

### Background Queue Processing

```rust
// Background task processes queue continuously
impl LiveSyncJob {
    async fn process_sync_queue(&mut self, ctx: JobContext<'_>) -> JobResult<()> {
        loop {
            // Process any pending changes from hooks
            let changes = SYNC_QUEUE.drain_pending();

            for change in changes {
                self.broadcast_sync_change(&ctx, change).await?;
            }

            // Also process explicit events
            while let Some(event) = self.event_receiver.try_recv() {
                self.handle_sync_event(&ctx, event).await?;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
            ctx.checkpoint().await?;
        }
    }
}
```

### Elegant Declarative API for Sync

Choose between derive macro or explicit implementation:

#### Option 1: Derive Macro (Recommended)

```rust
#[derive(Syncable)]
#[sync(
    id = "entry",
    domain = "Index",
    depends_on = ["location", "content_identity"],
    priority = 50,
    circular = "nullable:metadata_id"
)]
pub struct Entry {
    #[sync(uuid_field)]
    pub uuid: Option<Uuid>,

    #[sync(skip)] // Don't sync this field
    pub local_cache: Option<String>,

    // ... other fields sync automatically
}
```

#### Option 2: Manual Implementation

```rust
impl Syncable for entry::ActiveModel {
    const SYNC_ID: &'static str = "entry";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;
    const DEPENDENCIES: &'static [&'static str] = &["location", "content_identity"];
    const SYNC_PRIORITY: u8 = 50;

    fn should_sync(&self) -> bool {
        self.uuid.as_ref().is_some()
    }

    fn resolve_circular_dependency() -> Option<CircularResolution> {
        Some(CircularResolution::NullableReference("metadata_id"))
    }
}
```

#### Complete Working Example

```rust
// Simple case - no dependencies
#[derive(Syncable)]
#[sync(id = "device", domain = "Index")]
pub struct Device {
    pub uuid: Uuid,
    pub name: String,
    // All fields sync by default
}

// Complex case - with dependencies and circular resolution
#[derive(Syncable)]
#[sync(
    id = "entry",
    domain = "Index",
    depends_on = ["location", "content_identity"],
    circular = "nullable:metadata_id",
    uuid_field = "uuid"
)]
pub struct Entry {
    pub uuid: Option<Uuid>,    // Sync readiness indicator
    pub location_id: i32,      // Foreign key dependency
    pub content_id: Option<i32>, // Optional foreign key
    pub metadata_id: Option<Uuid>, // Nullable for circular resolution

    #[sync(skip)]
    pub local_temp_data: String, // Not synced
}

// That's it! The macro generates:
// - Syncable trait implementation
// - ActiveModelBehavior hooks
// - Dependency declarations
// - Circular resolution logic
// - Automatic sync queue integration
```

#### Macro-Generated Implementation (Internal)

```rust
// What the macro generates internally:
impl Syncable for entry::ActiveModel {
    const SYNC_ID: &'static str = "entry";
    const SYNC_DOMAIN: SyncDomain = SyncDomain::Index;
    const DEPENDENCIES: &'static [&'static str] = &["location", "content_identity"];

    fn should_sync(&self) -> bool {
        self.uuid.as_ref().is_some() // UUID field check
    }

    fn resolve_circular_dependency() -> Option<CircularResolution> {
        Some(CircularResolution::NullableReference("metadata_id"))
    }

    fn sync_fields() -> Option<Vec<&'static str>> {
        Some(vec![
            "uuid", "location_id", "content_id", "metadata_id", "name", "size"
            // Excludes "local_temp_data" marked with #[sync(skip)]
        ])
    }
}

impl ActiveModelBehavior for entry::ActiveModel {
    fn after_save(self, insert: bool) -> Result<Self, DbErr> {
        if self.should_sync() {
            SYNC_QUEUE.queue_change(SyncChange {
                model_type: Self::SYNC_ID,
                domain: self.get_sync_domain(),
                record_id: self.uuid.as_ref().unwrap().to_string(),
                change_type: if insert { ChangeType::Insert } else { ChangeType::Update },
                data: self.to_sync_json(), // Only includes sync_fields()
                timestamp: Utc::now(),
                was_sync_ready: true,
            });
        }
        Ok(self)
    }

    // Similar for after_delete...
}

// Auto-registration with sync system
inventory::submit! {
    SyncableModel::new::<entry::ActiveModel>()
}

```

### Comprehensive Sync Logging

Following the pattern from the networking logger, the sync system provides structured logging across all three phases:

#### Sync Logger Trait

```rust
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

/// Trait for sync operation logging
#[async_trait]
pub trait SyncLogger: Send + Sync {
    async fn info(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>);
    async fn warn(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>);
    async fn error(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>);
    async fn debug(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>);

    // Specialized sync logging methods
    async fn log_dependency_resolution(&self, model: &str, dependencies: &[&str], resolution_time: Duration);
    async fn log_circular_dependency(&self, cycle: &[&str], resolution: &CircularResolution);
    async fn log_phase_transition(&self, from: SyncPhase, to: SyncPhase, context: SyncContext);
    async fn log_batch_processing(&self, batch: &SyncBatch, processing_time: Duration);
    async fn log_conflict_resolution(&self, model: &str, conflict_type: &str, resolution: &str);
}

#[derive(Debug, Clone, Copy)]
pub enum SyncPhase {
    Capture,
    Store,
    Ingest,
}

#[derive(Debug, Clone)]
pub struct SyncContext {
    pub library_id: Uuid,
    pub device_id: Uuid,
    pub model_type: Option<String>,
    pub record_id: Option<String>,
    pub sequence_number: Option<u64>,
    pub batch_size: Option<usize>,
    pub dependency_level: Option<usize>,
    pub metadata: Value, // Additional context as JSON
}
```

#### Production Sync Logger

```rust
use tracing::{info, warn, error, debug, instrument};

/// Production logger using the tracing crate for structured logging
pub struct ProductionSyncLogger;

#[async_trait]
impl SyncLogger for ProductionSyncLogger {
    #[instrument(skip(self, context))]
    async fn info(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        if let Some(ctx) = context {
            info!(
                phase = ?phase,
                library_id = %ctx.library_id,
                device_id = %ctx.device_id,
                model_type = ctx.model_type,
                record_id = ctx.record_id,
                sequence_number = ctx.sequence_number,
                batch_size = ctx.batch_size,
                dependency_level = ctx.dependency_level,
                metadata = %ctx.metadata,
                "{}", message
            );
        } else {
            info!(phase = ?phase, "{}", message);
        }
    }

    #[instrument(skip(self, context))]
    async fn warn(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        if let Some(ctx) = context {
            warn!(
                phase = ?phase,
                library_id = %ctx.library_id,
                device_id = %ctx.device_id,
                model_type = ctx.model_type,
                record_id = ctx.record_id,
                sequence_number = ctx.sequence_number,
                batch_size = ctx.batch_size,
                dependency_level = ctx.dependency_level,
                metadata = %ctx.metadata,
                "{}", message
            );
        } else {
            warn!(phase = ?phase, "{}", message);
        }
    }

    #[instrument(skip(self, context))]
    async fn error(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        if let Some(ctx) = context {
            error!(
                phase = ?phase,
                library_id = %ctx.library_id,
                device_id = %ctx.device_id,
                model_type = ctx.model_type,
                record_id = ctx.record_id,
                sequence_number = ctx.sequence_number,
                batch_size = ctx.batch_size,
                dependency_level = ctx.dependency_level,
                metadata = %ctx.metadata,
                "{}", message
            );
        } else {
            error!(phase = ?phase, "{}", message);
        }
    }

    #[instrument(skip(self, context))]
    async fn debug(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        if let Some(ctx) = context {
            debug!(
                phase = ?phase,
                library_id = %ctx.library_id,
                device_id = %ctx.device_id,
                model_type = ctx.model_type,
                record_id = ctx.record_id,
                sequence_number = ctx.sequence_number,
                batch_size = ctx.batch_size,
                dependency_level = ctx.dependency_level,
                metadata = %ctx.metadata,
                "{}", message
            );
        } else {
            debug!(phase = ?phase, "{}", message);
        }
    }

    #[instrument(skip(self))]
    async fn log_dependency_resolution(&self, model: &str, dependencies: &[&str], resolution_time: Duration) {
        info!(
            sync_event = "dependency_resolution",
            model = model,
            dependencies = ?dependencies,
            resolution_time_ms = resolution_time.as_millis(),
            "Resolved dependencies for model"
        );
    }

    #[instrument(skip(self))]
    async fn log_circular_dependency(&self, cycle: &[&str], resolution: &CircularResolution) {
        warn!(
            sync_event = "circular_dependency",
            cycle = ?cycle,
            resolution_strategy = ?resolution,
            "Detected and resolved circular dependency"
        );
    }

    #[instrument(skip(self))]
    async fn log_phase_transition(&self, from: SyncPhase, to: SyncPhase, context: SyncContext) {
        info!(
            sync_event = "phase_transition",
            from_phase = ?from,
            to_phase = ?to,
            library_id = %context.library_id,
            device_id = %context.device_id,
            sequence_number = context.sequence_number,
            "Sync phase transition"
        );
    }

    #[instrument(skip(self))]
    async fn log_batch_processing(&self, batch: &SyncBatch, processing_time: Duration) {
        info!(
            sync_event = "batch_processed",
            models = ?batch.models,
            priority_order = ?batch.priority_order,
            batch_size = batch.models.len(),
            processing_time_ms = processing_time.as_millis(),
            has_circular_resolution = !batch.circular_resolution.is_empty(),
            "Processed sync batch"
        );
    }

    #[instrument(skip(self))]
    async fn log_conflict_resolution(&self, model: &str, conflict_type: &str, resolution: &str) {
        warn!(
            sync_event = "conflict_resolution",
            model = model,
            conflict_type = conflict_type,
            resolution_strategy = resolution,
            "Resolved sync conflict"
        );
    }
}
```

#### Development Sync Logger

```rust
/// Development logger with detailed console output (like NetworkLogger::ConsoleLogger)
pub struct ConsoleSyncLogger;

#[async_trait]
impl SyncLogger for ConsoleSyncLogger {
    async fn info(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        let phase_str = match phase {
            SyncPhase::Capture => "CAPTURE",
            SyncPhase::Store => "STORE",
            SyncPhase::Ingest => "INGEST",
        };

        if let Some(ctx) = context {
            println!("[SYNC {} INFO] {} | lib:{} dev:{} model:{:?} seq:{:?}",
                phase_str, message,
                ctx.library_id.to_string()[..8].to_string(),
                ctx.device_id.to_string()[..8].to_string(),
                ctx.model_type,
                ctx.sequence_number
            );
        } else {
            println!("[SYNC {} INFO] {}", phase_str, message);
        }
    }

    async fn warn(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        let phase_str = match phase {
            SyncPhase::Capture => "CAPTURE",
            SyncPhase::Store => "STORE",
            SyncPhase::Ingest => "INGEST",
        };

        eprintln!("⚠️  [SYNC {} WARN] {}", phase_str, message);
        if let Some(ctx) = context {
            eprintln!("   Context: lib:{} dev:{} model:{:?} seq:{:?}",
                ctx.library_id.to_string()[..8].to_string(),
                ctx.device_id.to_string()[..8].to_string(),
                ctx.model_type,
                ctx.sequence_number
            );
        }
    }

    async fn error(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        let phase_str = match phase {
            SyncPhase::Capture => "CAPTURE",
            SyncPhase::Store => "STORE",
            SyncPhase::Ingest => "INGEST",
        };

        eprintln!("❌ [SYNC {} ERROR] {}", phase_str, message);
        if let Some(ctx) = context {
            eprintln!("   Context: lib:{} dev:{} model:{:?} seq:{:?}",
                ctx.library_id.to_string()[..8].to_string(),
                ctx.device_id.to_string()[..8].to_string(),
                ctx.model_type,
                ctx.sequence_number
            );
        }
    }

    async fn debug(&self, phase: SyncPhase, message: &str, context: Option<SyncContext>) {
        let phase_str = match phase {
            SyncPhase::Capture => "CAPTURE",
            SyncPhase::Store => "STORE",
            SyncPhase::Ingest => "INGEST",
        };

        if let Some(ctx) = context {
            println!("🔍 [SYNC {} DEBUG] {} | lib:{} dev:{} model:{:?} seq:{:?}",
                phase_str, message,
                ctx.library_id.to_string()[..8].to_string(),
                ctx.device_id.to_string()[..8].to_string(),
                ctx.model_type,
                ctx.sequence_number
            );
        } else {
            println!("🔍 [SYNC {} DEBUG] {}", phase_str, message);
        }
    }

    async fn log_dependency_resolution(&self, model: &str, dependencies: &[&str], resolution_time: Duration) {
        println!("🔗 [SYNC DEP] Resolved {} dependencies: {:?} in {}ms",
            model, dependencies, resolution_time.as_millis());
    }

    async fn log_circular_dependency(&self, cycle: &[&str], resolution: &CircularResolution) {
        eprintln!("🔄 [SYNC CIRCULAR] Detected cycle: {:?} -> Resolved with: {:?}", cycle, resolution);
    }

    async fn log_phase_transition(&self, from: SyncPhase, to: SyncPhase, context: SyncContext) {
        println!("📋 [SYNC PHASE] {:?} -> {:?} | lib:{} seq:{:?}",
            from, to,
            context.library_id.to_string()[..8].to_string(),
            context.sequence_number
        );
    }

    async fn log_batch_processing(&self, batch: &SyncBatch, processing_time: Duration) {
        println!("📦 [SYNC BATCH] Processed {} models in {}ms: {:?}",
            batch.models.len(), processing_time.as_millis(), batch.models);
    }

    async fn log_conflict_resolution(&self, model: &str, conflict_type: &str, resolution: &str) {
        eprintln!("⚡ [SYNC CONFLICT] {} conflict in {}: resolved with {}",
            conflict_type, model, resolution);
    }
}
```

#### Integration with Sync Operations

```rust
// Example usage in sync operations
impl SyncLeaderService {
    async fn process_captured_changes(&self, changes: Vec<SyncChange>) -> Result<()> {
        let start_time = Instant::now();

        self.logger.info(
            SyncPhase::Store,
            "Starting dependency resolution for captured changes",
            Some(SyncContext {
                library_id: self.library_id,
                device_id: self.device_id,
                model_type: None,
                record_id: None,
                sequence_number: None,
                batch_size: Some(changes.len()),
                dependency_level: None,
                metadata: json!({ "change_count": changes.len() }),
            })
        ).await;

        // Group changes by dependency level
        let batched_changes = self.dependency_resolver.batch_by_dependencies(changes);

        let resolution_time = start_time.elapsed();
        self.logger.log_dependency_resolution(
            "mixed_models",
            &batched_changes.iter().flat_map(|b| b.models.iter().copied()).collect::<Vec<_>>(),
            resolution_time
        ).await;

        // Process each dependency batch
        for (level, batch) in batched_changes.iter().enumerate() {
            let batch_start = Instant::now();

            self.logger.debug(
                SyncPhase::Store,
                &format!("Processing dependency level {}", level),
                Some(SyncContext {
                    library_id: self.library_id,
                    device_id: self.device_id,
                    model_type: None,
                    record_id: None,
                    sequence_number: None,
                    batch_size: Some(batch.models.len()),
                    dependency_level: Some(level),
                    metadata: json!({ "models": batch.models }),
                })
            ).await;

            // Check for circular dependencies
            if !batch.circular_resolution.is_empty() {
                for resolution in &batch.circular_resolution {
                    let cycle = self.detect_cycle_for_resolution(resolution);
                    self.logger.log_circular_dependency(&cycle, resolution).await;
                }
            }

            self.store_dependency_batch(batch).await?;

            let batch_time = batch_start.elapsed();
            self.logger.log_batch_processing(batch, batch_time).await;
        }

        self.logger.info(
            SyncPhase::Store,
            "Completed dependency-ordered storage of changes",
            Some(SyncContext {
                library_id: self.library_id,
                device_id: self.device_id,
                model_type: None,
                record_id: None,
                sequence_number: None,
                batch_size: Some(batched_changes.len()),
                dependency_level: None,
                metadata: json!({
                    "total_time_ms": start_time.elapsed().as_millis(),
                    "dependency_levels": batched_changes.len()
                }),
            })
        ).await;

        Ok(())
    }
}
```

#### Example Log Output

```
🔍 [SYNC STORE DEBUG] Starting dependency resolution for captured changes | lib:a1b2c3d4 dev:e5f6g7h8 model:None seq:None
🔗 [SYNC DEP] Resolved mixed_models dependencies: ["device", "location", "entry", "user_metadata"] in 2ms
🔍 [SYNC STORE DEBUG] Processing dependency level 0 | lib:a1b2c3d4 dev:e5f6g7h8 model:None seq:None
📦 [SYNC BATCH] Processed 2 models in 15ms: ["device", "tag"]
🔍 [SYNC STORE DEBUG] Processing dependency level 1 | lib:a1b2c3d4 dev:e5f6g7h8 model:None seq:None
📦 [SYNC BATCH] Processed 1 models in 8ms: ["location"]
🔍 [SYNC STORE DEBUG] Processing dependency level 2 | lib:a1b2c3d4 dev:e5f6g7h8 model:None seq:None
🔄 [SYNC CIRCULAR] Detected cycle: ["entry", "user_metadata"] -> Resolved with: NullableReference("metadata_id")
📦 [SYNC BATCH] Processed 2 models in 23ms: ["entry", "user_metadata"]
[SYNC STORE INFO] Completed dependency-ordered storage of changes | lib:a1b2c3d4 dev:e5f6g7h8 model:None seq:None
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

### Phase 1: Universal Sync Infrastructure (Week 1)

- [ ] Create `Syncable` trait with built-in dependency support
- [ ] Implement `#[derive(Syncable)]` macro with dependency declarations
- [ ] Build automatic dependency graph generation
- [ ] Implement sync log table and models
- [ ] Build hybrid change tracking (SeaORM hooks + in-memory queue)

### Phase 2: Core Models with Dependencies (Week 2)

- [ ] Add sync to Device model (no dependencies)
- [ ] Add sync to Tag model (no dependencies)
- [ ] Add sync to ContentIdentity model (no dependencies)
- [ ] Add sync to Location model (depends on Device)
- [ ] Add sync to Entry model (depends on Location, ContentIdentity, circular with UserMetadata)
- [ ] Add sync to UserMetadata model (depends on Entry or ContentIdentity)
- [ ] Test automatic dependency ordering

### Phase 3: Universal Sync Protocol (Week 3)

- [ ] Implement automatic dependency-aware pull/push
- [ ] Build sync client with built-in ordering
- [ ] Add automatic circular reference resolution
- [ ] Implement backfill strategies respecting dependencies
- [ ] Add sync position tracking
- [ ] Test end-to-end dependency-aware sync

### Phase 4: Production Polish (Week 4)

- [ ] Add sync priority optimization within dependency levels
- [ ] Implement selective sync with dependency validation
- [ ] Add offline queue with dependency preservation
- [ ] Build sync status UI showing dependency progress
- [ ] Performance optimization for large dependency graphs

## Conclusion

This universal dependency-aware sync design eliminates the complexity of managing foreign key constraints during synchronization by making dependency awareness a core feature, not an add-on. The elegant declarative API means developers simply declare `depends_on = ["location", "device"]` and the sync system handles all ordering automatically.

By embedding dependency management directly into the `Syncable` trait and making it the default behavior for every sync operation, we ensure that Spacedrive's relational model "just works" without developers needing to think about constraint ordering, circular references, or special sync jobs.

The `#[derive(Syncable)]` macro reduces adding sync support to a model down to 3-5 lines of declarative code, while the automatic dependency graph generation ensures all sync operations respect foreign key constraints without any manual coordination.

This approach transforms sync from a complex, error-prone subsystem into a simple, declarative feature that scales naturally with Spacedrive's data model complexity.

## Further Enhancements & Detailed Considerations

This section elaborates on key areas to provide more robust details and address potential challenges in the sync system's design and implementation.

### 1\. Enhanced Leadership Management

To ensure high availability and resilience for library leadership:

#### Initial Leader Selection

When a new library is created, the device initiating its creation automatically becomes the initial leader.
When existing libraries are merged during pairing, the user explicitly chooses which device becomes the leader (either the local device, the remote device, or a newly created shared library leader).

#### Offline Leader Detection & Reassignment

- **Heartbeats**: Leader devices periodically send heartbeats to their followers over the persistent networking layer.
- **Failure Detection**: Followers continuously monitor these heartbeats. If a follower misses a configurable number of consecutive heartbeats from the leader, it will consider the leader potentially offline.
- **Leader Election Protocol**:
  1.  Upon detecting an offline leader, followers will initiate a leader election protocol. This could involve a simple deterministic rule (e.g., the device with the lexicographically smallest `device_id` among the online followers becomes the new candidate leader) or a more robust consensus algorithm (e.g., Paxos or Raft-lite adapted for a small, dynamic peer group).
  2.  The candidate leader attempts to broadcast its claim to leadership to all other known library devices.
  3.  Followers that agree on the new candidate (e.g., by verifying the previous leader's prolonged absence) update their `sync_leadership` role for that library.
  4.  The newly elected leader updates its `sync_leadership` role in its local database and notifies other devices of the transition.
- **Timeout & Retries**: The leader election process will have configurable timeouts and retry mechanisms to handle network transience.

**Rust Design for Leader Election:**

```rust
// In persistent/service.rs or a dedicated leader_election.rs
pub enum LeaderElectionMessage {
    ProposeLeader { library_id: Uuid, candidate_device_id: Uuid, epoch: u64 },
    AcknowledgeProposal { library_id: Uuid, proposed_device_id: Uuid, epoch: u64, voter_device_id: Uuid },
    ConfirmLeader { library_id: Uuid, leader_device_id: Uuid, epoch: u64 },
}

// Implement ProtocolHandler for LeaderElectionMessage
#[async_trait::async_trait]
impl ProtocolHandler for LeaderElectionHandler {
    async fn handle_message(
        &self,
        device_id: Uuid, // Sender of the message
        message: DeviceMessage,
    ) -> Result<Option<DeviceMessage>> {
        match message {
            DeviceMessage::Custom { protocol, payload, .. } if protocol == "leader-election" => {
                let election_msg: LeaderElectionMessage = serde_json::from_value(payload)?;
                // Handle different election message types (Propose, Acknowledge, Confirm)
                // Update local leader state and potentially send responses
                Ok(None)
            },
            _ => Ok(None),
        }
    }
    // ... other trait methods
}

// Function to trigger election
impl SyncLeaderService {
    pub async fn initiate_leader_election(&self, library_id: Uuid) -> Result<()> {
        let current_epoch = self.get_current_epoch(library_id).await?;
        let new_epoch = current_epoch + 1;
        let self_device_id = self.device_manager.get_local_device_id().await?;

        // Propose self as leader (or a deterministic candidate)
        let proposal = LeaderElectionMessage::ProposeLeader {
            library_id,
            candidate_device_id: self_device_id,
            epoch: new_epoch,
        };

        // Broadcast proposal to all known devices in the library
        self.networking_service.broadcast_message(
            &library_id,
            DeviceMessage::Custom {
                protocol: "leader-election".to_string(),
                version: 1,
                payload: serde_json::to_value(proposal)?,
                metadata: HashMap::new(),
            },
        ).await?;
        // Manage state for acknowledgements
        Ok(())
    }
}
```

#### Split-Brain Prevention

- **Quorum (for multi-leader support)**: While the current design is "One Leader Per Library", if future enhancements consider multi-leader or more dynamic leadership, a quorum-based approach would be necessary to prevent split-brain. This means a new leader can only be elected if a majority of the known devices (or a predefined set of trusted devices) agree.
- **Last-Write-Wins with Epochs (for single-leader)**: For the current single-leader model, each leadership transition could involve an incrementing "epoch" number. Any sync operation would carry the current epoch. If a device receives an operation from a leader with an older epoch, it would reject it and initiate a new leader election or update its knowledge of the current leader.

**Rust Design for Epochs:**

```rust
// Add epoch to SyncLogEntry
pub struct SyncLogEntry {
    // ... existing fields
    pub epoch: u64, // Epoch of the leader when the change was recorded
}

// Add epoch to SyncPosition
pub struct SyncPosition {
    // ... existing fields
    pub last_applied_epoch: u64, // Last epoch applied by this follower
}

// Leader Service: Assign current epoch
impl SyncLeaderService {
    async fn write_model_changes_to_log(&self, changes: Vec<SyncChange>) -> Result<()> {
        let current_epoch = self.get_current_epoch(self.library_id).await?;
        for mut change in changes {
            change.epoch = current_epoch; // Assign current leader epoch
            // ... assign sequence number and persist
        }
        Ok(())
    }
}

// Follower Service: Validate epoch
impl SyncFollowerService {
    async fn apply_dependency_batch(&self, batch: SyncBatch) -> Result<()> {
        let current_library_epoch = self.get_current_library_epoch(self.library_id).await?;
        for model_id in &batch.priority_order {
            let changes = batch.get_changes_for_model(model_id);
            for change in changes {
                if change.epoch < current_library_epoch {
                    // This change is from an older, potentially defunct leader. Discard or queue for re-fetch.
                    self.logger.warn(SyncPhase::Ingest, "Discarding change from older epoch", Some(SyncContext {
                        library_id: self.library_id,
                        device_id: self.device_id, // Follower's device ID
                        model_type: Some(change.model_type.clone()),
                        record_id: Some(change.record_id.clone()),
                        sequence_number: Some(change.seq),
                        metadata: json!({"change_epoch": change.epoch, "current_epoch": current_library_epoch}),
                        ..Default::default()
                    })).await;
                    continue;
                }
                // Apply the change
            }
        }
        Ok(())
    }
}
```

### 2\. Detailed Conflict Resolution & User Experience

#### Conflict Prompting and User Interface

For "True Conflicts" (e.g., `UserMetadata` notes where changes diverge):

- **Conflict Indicator**: The UI will display a clear visual indicator on the conflicting item (e.g., an icon on the `Entry` or `ContentIdentity` details view).
- **Conflict Resolution View**: Clicking the indicator will open a dedicated conflict resolution view. This view will:
  - Show the local version of the data.
  - Show the remote conflicting version of the data.
  - Display a diff (if applicable, e.g., for text notes).
  - Provide options: "Keep Local," "Keep Remote," "Merge Manually" (for text fields), or "Discard All."
- **Batch Resolution**: For multiple conflicts, the UI may offer a batch resolution interface with general rules (e.g., "Always Keep Local for all similar conflicts").
- **Background Notification**: Users will receive a system notification (e.g., a toast notification or a badge on the sync status icon) when conflicts are detected, directing them to the conflict resolution area.

#### Automatic Fallback Strategies

- **Default Behavior (User-Configurable)**: Users will be able to set a default conflict resolution strategy in settings, such as:
  - **"Latest Wins"**: The most recently modified version is automatically applied.
  - **"Local Always Wins"**: The local version is always preserved.
  - **"Remote Always Wins"**: The remote version is always applied.
  - **"Prompt Always"**: Always requires manual intervention.
- **Notes Merge Logic**: For `UserMetadata` notes, the `merge_notes` function will by default concatenate notes with timestamps, providing a historical record: `merge_notes(local.notes, remote.notes)` could result in:
  ```
  "Local notes (last modified 2025-06-24 10:00:00): Original text.
  Remote notes (last modified 2025-06-24 10:01:30): Conflicting text."
  ```
- **Custom Data Merge Logic**: The `merge_custom_data` function (for `custom_data` in `UserMetadata`) will perform a deep merge of JSON objects, prioritizing the remote value for conflicting keys, but adding new keys from both sides. For arrays, it could perform a union.

**Rust Design for Conflict Handling:**

```rust
pub enum ConflictType {
    ManualResolutionRequired,
    LatestWins,
    LocalWins,
    RemoteWins,
    UnionMerge,
    AdditiveMerge,
    // ... others
}

pub enum MergeResult<T> {
    NoConflict(T),
    Merged(T),
    Conflict(T, T, ConflictType), // Indicate conflict type for UI/automatic resolution
}

#[async_trait]
impl Syncable for user_metadata::ActiveModel {
    // ...
    fn merge(local: Self::Model, remote: Self::Model) -> MergeResult<Self::Model> {
        // ... determine domain dynamically
        match domain {
            SyncDomain::Index => MergeResult::NoConflict(remote), // Device owns entry metadata
            SyncDomain::UserMetadata => {
                // Apply intelligent merge based on fields, and return Conflict if manual resolution is needed for notes
                let merged_notes = merge_notes(local.notes.clone(), remote.notes.clone());
                let merged_custom_data = merge_custom_data(local.custom_data.clone(), remote.custom_data.clone());

                // If notes were truly conflicting and not just appended
                if merged_notes.is_conflict() { // Example: a new enum or flag on merge_notes result
                    return MergeResult::Conflict(local, remote, ConflictType::ManualResolutionRequired);
                }

                MergeResult::Merged(Self::Model {
                    favorite: local.favorite || remote.favorite,
                    hidden: local.hidden || remote.hidden,
                    notes: merged_notes.resolved_value(), // Get the resolved value (e.g., concatenated)
                    custom_data: merged_custom_data,
                    updated_at: std::cmp::max(local.updated_at, remote.updated_at),
                    ..local
                })
            },
            _ => unreachable!()
        }
    }
}

// In SyncFollowerService apply_change_directly
async fn apply_change_directly(&self, change: SyncChange) -> Result<()> {
    // ...
    if let MergeResult::Conflict(local_data, remote_data, conflict_type) = model.merge(local_model, remote_model) {
        match conflict_type {
            ConflictType::ManualResolutionRequired => {
                // Store conflict for UI resolution
                self.conflict_manager.add_conflict(local_data, remote_data, change).await?;
                self.logger.warn(SyncPhase::Ingest, "Manual conflict detected", Some(SyncContext {
                    model_type: Some(change.model_type),
                    record_id: Some(change.record_id),
                    // ... other context
                    metadata: json!({"conflict_type": "ManualResolutionRequired"}),
                })).await;
            },
            ConflictType::LatestWins => { /* apply remote if newer */ },
            // ... handle other automatic types
            _ => { /* apply merged data */ }
        }
    }
    // ...
    Ok(())
}

// A new ConflictManager struct to store conflicts for UI
pub struct ConflictManager {
    // Stores conflicts in persistent storage
}
```

### 3\. Scalability and Maintenance of Sync Log and Positions

#### Sync Log Pruning and Archiving

- **Configurable Retention**: Users/administrators can configure a retention period for `sync_log` entries (e.g., 3 months, 1 year, indefinite).
- **Archiving**: Old `sync_log` entries (beyond the retention period) could be archived to a separate, less frequently accessed storage location (e.g., compressed files) to reduce the primary database size.
- **Summarization**: Periodically, the system could run a background job to summarize change history for long-lived records, allowing older detailed entries to be pruned while retaining an aggregated view.
- **`first_seen_at` and `last_verified_at`**: These fields in `ContentIdentity` already contribute to long-term data consistency and can aid in pruning older, less relevant `SyncLogEntry` data.

**Rust Design for Log Management:**

```rust
// In a dedicated log_manager.rs
pub struct SyncLogManager {
    db: DatabaseConnection,
    retention_policy: SyncLogRetentionPolicy,
}

pub enum SyncLogRetentionPolicy {
    Days(u32),
    Months(u32),
    Indefinite,
}

impl SyncLogManager {
    pub async fn prune_old_entries(&self) -> Result<usize> {
        if let SyncLogRetentionPolicy::Days(days) = self.retention_policy {
            let cutoff_date = Utc::now() - Duration::days(days as i64);
            let deleted_count = SyncLogEntry::delete_many()
                .filter(sync_log_entry::Column::Timestamp.lt(cutoff_date))
                .exec(&self.db)
                .await?
                .rows_affected;
            Ok(deleted_count as usize)
        } else {
            Ok(0) // No pruning if policy is indefinite
        }
    }

    pub async fn start_pruning_task(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::hours(24)).await; // Run daily
                if let Err(e) = manager.prune_old_entries().await {
                    tracing::error!("Failed to prune sync log entries: {}", e);
                }
            }
        });
    }
}
```

#### `SyncPositionManager` Scalability

- The `sync_positions` table's primary key on `(device_id, library_id)` is efficient for direct lookups.
- As the number of devices and libraries scales, indexing on `updated_at` could be beneficial for quickly identifying stale positions or devices that need re-syncing.
- The actual sync log entries themselves are processed in batches, which limits the in-memory load during active sync operations, rather than needing to load the entire history.

### 4\. Performance Optimization for Backfill and Entity Requests

#### Parallelizing Backfill

- **Domain-based Parallelism**: During `full_backfill` and `incremental_backfill`, instead of strictly sequential processing of all changes from `current_seq`, the system can fetch and process changes from _different_ `SyncDomain`s in parallel, as their conflict resolution strategies are distinct and often independent at the high level.
- **Batching within Domains**: While the current design pulls batches of changes, further optimization can be achieved by allowing multiple concurrent pull requests for different sequence ranges within the same domain, provided dependencies within those ranges are respected at the application phase.
- **Network Service Enhancements**: The `NetworkingService` could expose an API to pull multiple `SyncLogEntry` batches concurrently, managing the underlying LibP2P streams efficiently.

**Rust Design for Parallel Backfill:**

```rust
impl BackfillSyncJob {
    async fn full_backfill(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        let networking = ctx.networking_service()
            .ok_or(JobError::Other("Networking not available".into()))?;

        let target_seq = networking.get_latest_seq(self.leader_device_id, self.library_id).await?;
        let mut current_seq = 0;
        let batch_size = 1000;

        while current_seq < target_seq {
            let remaining = target_seq - current_seq;
            let current_limit = std::cmp::min(batch_size, remaining as usize);

            // Fetch changes for both Index and UserMetadata domains concurrently
            let (index_changes_res, user_metadata_changes_res) = tokio::join!(
                networking.pull_changes(
                    self.leader_device_id,
                    self.library_id,
                    current_seq,
                    Some(current_limit),
                    vec![SyncDomain::Index]
                ),
                networking.pull_changes(
                    self.leader_device_id,
                    self.library_id,
                    current_seq, // Still pull from same sequence base for consistency
                    Some(current_limit),
                    vec![SyncDomain::UserMetadata]
                )
            );

            let mut all_batch_changes = Vec::new();
            if let Ok(index_batch) = index_changes_res {
                all_batch_changes.extend(index_batch.changes);
            }
            if let Ok(user_metadata_batch) = user_metadata_changes_res {
                all_batch_changes.extend(user_metadata_batch.changes);
            }

            // The SYNC_REGISTRY.batch_changes_by_dependencies will correctly reorder
            // changes from both domains based on their inter-dependencies.
            let batched_changes = SYNC_REGISTRY.batch_changes_by_dependencies(all_batch_changes);

            for dep_batch in batched_changes {
                if let Err(e) = self.apply_batch_with_circular_resolution(dep_batch, ctx).await {
                    self.state.as_mut().unwrap().failed_records.push(FailedRecord {
                        seq: current_seq, // Note: This seq might not be accurate for individual failed records
                        model_type: "batch".to_string(),
                        record_id: format!("seq_{}", current_seq),
                        error: e.to_string(),
                    });
                }
            }
            // Max of the latest_seq from individual pulls, or simply current_seq + current_limit
            current_seq = current_seq + current_limit as u64;

            ctx.progress(Progress::percentage(current_seq as f64 / target_seq as f64));
            ctx.checkpoint().await?;
        }
        Ok(())
    }
}
```

#### Batching `sync_ready_backfill` Requests

- **Batched Entity Requests**: Instead of `request_entity_from_leader` for each `entity_uuid`, the `sync_ready_backfill` strategy will gather lists of `entity_uuid`s for a given `model_type` and send a single `SyncPullModelBatch` request (or similar) to the leader. The leader would then return the full data for all requested entities in a single response. This significantly reduces round-trip times and network overhead.
- **Progress Granularity**: Progress updates for these batched operations will be based on the completion of full batches rather than individual entities.

**Rust Design for Batched Entity Requests:**

```rust
// Add new message type to DeviceMessage
pub enum DeviceMessage {
    // ... existing messages
    SyncPullModelBatchRequest {
        library_id: Uuid,
        model_type: String,
        record_ids: Vec<String>, // List of UUIDs to request
    },
    SyncPullModelBatchResponse {
        library_id: Uuid,
        model_type: String,
        changes: Vec<SyncLogEntry>, // Full SyncLogEntry for each requested record
    },
}

impl BackfillSyncJob {
    async fn sync_ready_backfill(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        let networking = ctx.networking_service()
            .ok_or(JobError::Other("Networking not available".into()))?;

        let sync_ready_entities = self.get_sync_ready_entities().await?; // Map: model_type -> Vec<Uuid>
        let sync_order = SYNC_REGISTRY.get_sync_order();
        let batch_size = 100; // Batch size for requesting entities

        for batch_info in sync_order { // batch_info has models in dependency order
            for entity_type in &batch_info.models {
                ctx.progress(Progress::message(&format!("Backfilling sync-ready {}", entity_type)));

                if let Some(entities_uuids) = sync_ready_entities.get(*entity_type) {
                    for chunk in entities_uuids.chunks(batch_size) {
                        let record_ids: Vec<String> = chunk.iter().map(|u| u.to_string()).collect();

                        let request = DeviceMessage::SyncPullModelBatchRequest {
                            library_id: self.library_id,
                            model_type: entity_type.to_string(),
                            record_ids: record_ids.clone(),
                        };

                        let response = networking.send_to_device(
                            self.leader_device_id,
                            request
                        ).await?;

                        if let DeviceMessage::SyncPullModelBatchResponse { changes, .. } = response {
                            let batched_changes = SYNC_REGISTRY.batch_changes_by_dependencies(changes);
                            for dep_batch in batched_changes {
                                if let Err(e) = self.apply_batch_with_circular_resolution(dep_batch, ctx).await {
                                    tracing::warn!(
                                        "Failed to backfill batch for {}: {:?}. Error: {}",
                                        entity_type, record_ids, e
                                    );
                                    // Log individual failures if desired, or skip the batch
                                }
                            }
                        } else {
                            return Err(JobError::Other("Unexpected response for SyncPullModelBatchRequest".into()));
                        }
                        ctx.progress(Progress::percentage( /* calculate progress based on chunks */ ));
                        ctx.checkpoint().await?;
                    }
                }
            }
        }
        Ok(())
    }
}
```

### 5\. `UserMetadataTag Junction` Sync Domain Resolution

The `user_metadata_tag::ActiveModel`'s `get_sync_domain` which returns `SyncDomain::UserMetadata` by default, requires clarification.

- **Explicit Parent Lookup**: During the "Store" and "Ingest" phases, when processing a `user_metadata_tag` change, the `SyncLeaderService` and `SyncFollowerService` will explicitly perform a lookup to its associated `UserMetadata` record.
- **Dynamic Domain Assignment**: The looked-up `UserMetadata` record's `get_sync_domain` method will then be called to determine the final `SyncDomain` (either `Index` for entry-scoped metadata or `UserMetadata` for content-scoped metadata). This ensures the tag correctly inherits the conflict resolution strategy of its parent metadata.
- **Performance Impact**: This lookup adds a minor database query overhead for each `user_metadata_tag` change during phases 2 and 3. Given that tags are typically part of a larger `UserMetadata` operation, this overhead is considered acceptable and ensures correct domain-specific merging.

**Rust Design for Dynamic Domain Lookup:**

```rust
// In user_metadata_tag::ActiveModel implementation of Syncable
impl Syncable for user_metadata_tag::ActiveModel {
    // ...
    fn get_sync_domain(&self) -> SyncDomain {
        // This method will perform the lookup at runtime when needed by the sync services.
        // It's a placeholder for the actual lookup logic which will be in the sync service.
        SyncDomain::UserMetadata // Default for trait definition, actual determined dynamically
    }
}

// In SyncLeaderService or SyncFollowerService, when processing UserMetadataTag changes:
async fn process_user_metadata_tag_change(&self, change: SyncChange) -> Result<()> {
    let tag_data: user_metadata_tag::Model = serde_json::from_value(change.data)?;

    // Look up the parent UserMetadata record
    let user_metadata_record = user_metadata::Entity::find()
        .filter(user_metadata::Column::Uuid.eq(tag_data.user_metadata_uuid))
        .one(&self.db) // or &ctx.db()
        .await?
        .ok_or_else(|| JobError::Other("UserMetadata not found for tag".into()))?;

    let actual_sync_domain = user_metadata::ActiveModel::from_entity(user_metadata_record).get_sync_domain();

    // Now process the user_metadata_tag change with the correct domain
    // (e.g., store in sync log with this domain, or apply with this domain's merge logic)
    let processed_change = SyncChange {
        domain: actual_sync_domain, // Override with the dynamically determined domain
        ..change
    };
    // Proceed with storing/applying processed_change
    Ok(())
}
```

### 6\. Offline Queue Persistence

The `OfflineQueue` for changes collected when a device is offline will be persisted to disk to prevent data loss upon application shutdown or crash:

- **Transactional Persistence**: When `SYNC_QUEUE.flush_for_transaction` is called during database operations, in addition to persisting to the `sync_log` (on the leader), or buffering for later application (on the follower), these changes will also be written to a local, append-only "offline journal" file before the transaction commits.
- **Journal Structure**: The offline journal will store serialized `SyncChange` objects in a structured, fault-tolerant format (e.g., line-delimited JSON or a simple binary log).
- **Recovery on Startup**: Upon application startup, before any new changes are captured, the system will check for and replay any pending changes from the offline journal. Successfully replayed changes will be marked as processed or removed from the journal.
- **Deduplication**: When replaying, the system will handle potential duplicates (e.g., if a change was partially synced before going offline) using the `record_id` and `timestamp` from `SyncChange`.

**Rust Design for Offline Journal:**

```rust
// In persistent/offline_journal.rs
pub struct OfflineJournal {
    path: PathBuf,
    writer: Arc<Mutex<BufWriter<File>>>,
}

impl OfflineJournal {
    pub async fn new(data_dir: &Path) -> Result<Self> {
        let journal_path = data_dir.join("offline_journal.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true) // Append to existing log
            .open(&journal_path)
            .await?;
        Ok(Self {
            path: journal_path,
            writer: Arc::new(Mutex::new(BufWriter::new(file.into_std().await))),
        })
    }

    pub async fn append_change(&self, change: &SyncChange) -> Result<()> {
        let mut writer = self.writer.lock().unwrap(); // Blocking lock for simplicity, consider async mutex for production
        let serialized = serde_json::to_string(change)?;
        writeln!(writer, "{}", serialized)?;
        writer.flush()?; // Ensure immediate write to disk
        Ok(())
    }

    pub async fn read_all_changes(&self) -> Result<Vec<SyncChange>> {
        let file = File::open(&self.path).await?;
        let reader = BufReader::new(file);
        let mut changes = Vec::new();
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await? {
            if let Ok(change) = serde_json::from_str(&line) {
                changes.push(change);
            } else {
                tracing::warn!("Corrupted line in offline journal: {}", line);
            }
        }
        Ok(changes)
    }

    // After successful flush, clear the journal
    pub async fn clear(&self) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.get_mut().set_len(0)?; // Truncate the file
        writer.flush()?;
        Ok(())
    }
}

// Modify SYNC_QUEUE to use OfflineJournal
impl SyncQueue {
    // ...
    pub async fn flush_for_transaction(&self, db: &DatabaseConnection, journal: &OfflineJournal) -> Result<()> {
        let changes = self.drain_pending();
        if changes.is_empty() {
            return Ok(());
        }

        db.transaction(|txn| async move {
            for change in changes {
                // First, append to offline journal (blocking for safety)
                journal.append_change(&change).await?; // This should be synchronous or use a dedicated task
                self.persist_sync_change(change, txn).await?; // Persist to sync_log
            }
            Ok(())
        }).await?;

        // After successful transaction, clear the journal (or mark entries as processed)
        // For simplicity here, clearing whole journal; production might clear individual entries.
        journal.clear().await?;
        Ok(())
    }
}
```

### 7\. Refined Security Considerations

- **Rate Limiting on Pairing Attempts**:
  - **Per-IP/Per-Device Limiting**: The networking service will implement rate limiting on `PairingRequest` messages. This will involve tracking incoming requests from specific IP addresses or LibP2P `PeerId`s.
  - **Sliding Window/Token Bucket**: A sliding window or token bucket algorithm will be used to limit the number of pairing attempts within a given time frame (e.g., 5 attempts per minute from a single source).
  - **Blocking**: Excessive attempts will result in temporary blocking of the source.

**Rust Design for Rate Limiting:**

```rust
// In networking/protocols/pairing/protocol.rs or a middleware
use governor::{Quota, RateLimiter};
use governor::state::keyed::Default};
use std::time::Duration;

pub struct PairingRateLimiter {
    // Limiter for global pairing requests (e.g., per IP/PeerId)
    limiter: RateLimiter<PeerId, Default>,
}

impl PairingRateLimiter {
    pub fn new() -> Self {
        Self {
            limiter: RateLimiter::keyed(Quota::per_second(5).allow_burst(1)), // 5 attempts per second, 1 burst
        }
    }

    pub fn allow_request(&self, peer_id: &PeerId) -> bool {
        self.limiter.check_key(peer_id).is_ok()
    }
}

// Integrate into PairingProtocolHandler or NetworkingService
#[async_trait::async_trait]
impl ProtocolHandler for PairingProtocolHandler {
    async fn handle_message(
        &self,
        peer_id: Uuid, // Or PeerId in libp2p context
        message: DeviceMessage,
    ) -> Result<Option<DeviceMessage>> {
        if !self.rate_limiter.allow_request(&peer_id) {
            self.logger.warn(SyncPhase::Ingest, "Rate limit exceeded for pairing request", Some(SyncContext {
                device_id: peer_id, // Assuming PeerId can be mapped to a Uuid for logging
                metadata: json!({"reason": "rate_limit"}),
                ..Default::default()
            })).await;
            return Err(NetworkError::Protocol("Rate limit exceeded".into()));
        }
        // ... proceed with message handling
        Ok(None)
    }
}
```

- **User Confirmation UI for Pairing Requests**:
  - **Explicit Approval**: After a `PairingRequest` is received and cryptographically verified, the initiator device (Alice) will _not_ automatically complete the pairing. Instead, a UI prompt will appear, asking the user to confirm the pairing with the remote device (Bob's `DeviceInfo` will be displayed).
  - **Timeout for Confirmation**: If the user does not respond within a configurable timeout, the pairing session will expire and fail.
  - **API for Confirmation**: The `NetworkingService` will expose an API (e.g., `confirm_pairing_request(session_id, accept: bool)`) that the UI can call based on user interaction.

**Rust Design for User Confirmation:**

```rust
// New state for PairingSession
pub enum PairingState {
    // ... existing states
    ConfirmationPending { remote_device_info: DeviceInfo }, // Waiting for user confirmation
}

impl PairingProtocolHandler {
    // This function would be called by the UI
    pub async fn confirm_pairing_request(&self, session_id: Uuid, accept: bool) -> Result<()> {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            match &session.state {
                PairingState::ConfirmationPending { .. } => {
                    if accept {
                        session.state = PairingState::Completed;
                        // Trigger finalization (e.g., send Complete message)
                        self.send_pairing_complete(session_id, true, None).await?;
                        self.logger.info(SyncPhase::Store, "User confirmed pairing", Some(SyncContext {
                            library_id: Uuid::nil(), // N/A for pairing
                            device_id: session.remote_device_id.unwrap_or_default(),
                            metadata: json!({"session_id": session_id}),
                            ..Default::default()
                        })).await;
                    } else {
                        session.state = PairingState::Failed { reason: "User rejected".to_string() };
                        self.send_pairing_complete(session_id, false, Some("User rejected".to_string())).await?;
                        self.logger.warn(SyncPhase::Store, "User rejected pairing", Some(SyncContext {
                            library_id: Uuid::nil(), // N/A for pairing
                            device_id: session.remote_device_id.unwrap_or_default(),
                            metadata: json!({"session_id": session_id}),
                            ..Default::default()
                        })).await;
                    }
                    self.persistence.save_sessions(&sessions.clone().into_iter().collect()).await?; // Persist new state
                    Ok(())
                }
                _ => Err(NetworkError::Protocol("Not in confirmation pending state".into())),
            }
        } else {
            Err(NetworkError::DeviceNotFound(session_id))
        }
    }
}

// When PairingProtocolHandler receives Response message, transition to ConfirmationPending
// if auto-accept is not enabled.
```

- **Device Limits**:
  - **User-Configurable Limits**: Spacedrive will allow users to configure limits on the total number of devices that can be paired to a single library or across all libraries.
  - **Policy Enforcement**: When a new pairing request is initiated, the system will check against these limits. If exceeded, the pairing will be rejected, and the user will be notified.

**Rust Design for Device Limits:**

```rust
// In Core configuration or Library settings
pub struct AppConfig {
    // ...
    pub max_paired_devices_per_library: Option<u32>,
    pub max_total_paired_devices: Option<u32>,
}

// In DeviceManager or PairingProtocolHandler before accepting a new device
impl PairingProtocolHandler {
    async fn pre_accept_pairing_checks(&self, new_device_id: Uuid) -> Result<()> {
        let config = self.config_manager.get_app_config().await?; // Get global config
        let current_paired_devices_count = self.device_registry.read().await.get_paired_devices_count().await;

        if let Some(max_total) = config.max_total_paired_devices {
            if current_paired_devices_count >= max_total {
                return Err(NetworkError::Protocol(format!("Device limit of {} exceeded.", max_total)));
            }
        }
        // Could also check per-library limits here if library context is available
        Ok(())
    }
}
```

- **Data Encryption in Sync Log**:
  - The `data` field in `SyncLogEntry`, which stores the serialized model data as JSON, will be encrypted _before_ being written to the database.
  - **Column-Level Encryption**: This can be achieved using a symmetric key derived from the library's master key (which itself is secured by the user's password) to encrypt the `data` field (e.g., using AES-256-GCM).
  - **Key Management**: The encryption key for the `sync_log` will be managed by the `SecureStorage` module, ensuring it is only accessible when the user's password unlocks the device's secure storage.

**Rust Design for Sync Log Data Encryption:**

```rust
// In sync_log_entry::ActiveModel
#[derive(Debug, Clone, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sync_log")]
pub struct Model {
    // ... existing fields
    pub data: Option<Vec<u8>>, // Store encrypted bytes instead of plain JSON
    pub encryption_iv: Option<Vec<u8>>, // Store IV if using AES-GCM
}

impl SyncLogEntryActiveModel {
    pub async fn new_encrypted(change: SyncChange, encryption_service: &EncryptionService) -> Result<Self> {
        let encrypted_data = if let Some(data) = change.data {
            let (encrypted_payload, iv) = encryption_service.encrypt_data(
                &serde_json::to_vec(&data)?, // Serialize JSON to bytes first
                &change.library_id // Use library_id for key derivation/lookup
            ).await?;
            Some(encrypted_payload)
        } else {
            None
        };

        let encryption_iv = if encrypted_data.is_some() { /* extract IV here */ } else { None };

        Ok(Self {
            library_id: Set(change.library_id),
            domain: Set(change.domain),
            timestamp: Set(change.timestamp),
            device_id: Set(change.device_id),
            model_type: Set(change.model_type),
            record_id: Set(change.record_id),
            change_type: Set(change.change_type),
            data: Set(encrypted_data),
            encryption_iv: Set(encryption_iv),
            was_sync_ready: Set(change.was_sync_ready),
            // ... epoch if added
        })
    }
}

// Decryption when reading from sync log
impl SyncLogEntry {
    pub async fn decrypt_data(&self, encryption_service: &EncryptionService) -> Result<Option<serde_json::Value>> {
        if let Some(encrypted_payload) = &self.data {
            let iv = self.encryption_iv.as_ref().ok_or_else(|| anyhow::anyhow!("Missing IV for encrypted data"))?;
            let decrypted_bytes = encryption_service.decrypt_data(
                encrypted_payload,
                iv,
                &self.library_id // Use library_id for key derivation/lookup
            ).await?;
            Ok(Some(serde_json::from_slice(&decrypted_bytes)?))
        } else {
            Ok(None)
        }
    }
}

// The EncryptionService would wrap ring for AES-GCM and integrate with SecureStorage for keys.
```

## Folder Structure

```
src/
├── sync/
│   ├── mod.rs                      # Main sync module exports (SyncService, SyncManager)
│   ├── types.rs                    # Core sync types (SyncDomain, ChangeType, SyncChange, SyncContext, SyncPhase)
│   ├── traits.rs                   # Defines Syncable trait and related enums (CircularResolution, MergeResult, ConflictType)
│   ├── registry.rs                 # Manages Syncable models and builds dependency graph (SyncRegistry)
│   ├── manager.rs                  # Orchestrates sync jobs (SyncJobManager - high-level interface for Core)
│   ├── jobs/                       # Definitions for sync-related jobs
│   │   ├── mod.rs                  # Job module exports
│   │   ├── initial_sync.rs         # InitialSyncJob implementation
│   │   ├── live_sync.rs            # LiveSyncJob implementation
│   │   ├── backfill_sync.rs        # BackfillSyncJob implementation
│   │   ├── sync_readiness.rs       # SyncReadinessJob for pre-sync entries
│   │   └── sync_setup.rs           # SyncSetupJob for library merging
│   ├── protocol/                   # Handles sync-specific message logic (client/server for sync data)
│   │   ├── mod.rs                  # Protocol exports
│   │   ├── handler.rs              # Implements ProtocolHandler for sync messages (SyncProtocolHandler)
│   │   ├── messages.rs             # Sync-specific messages (SyncPullRequest, SyncPullResponse, SyncChange, SyncPullModelBatchRequest/Response)
│   │   └── services.rs             # Encapsulates core sync logic (SyncLeaderService, SyncFollowerService)
│   ├── state/                      # Manages persistent sync state
│   │   ├── mod.rs                  # State exports
│   │   ├── sync_log.rs             # Sync log table operations (SyncLogManager)
│   │   ├── sync_position.rs        # Sync position tracking (SyncPositionManager)
│   │   ├── conflict_manager.rs     # Manages detected conflicts for UI resolution
│   │   └── offline_journal.rs      # Offline journal for unsynced changes
│   ├── logging.rs                  # Sync-specific logging (SyncLogger trait, ProductionSyncLogger, ConsoleSyncLogger)
│   └── util/                       # Utility functions for sync operations
│       ├── mod.rs                  # Utility exports
│       ├── dependency_resolver.rs  # Logic for building/traversing dependency graphs
│       ├── change_queue.rs         # In-memory queue for captured changes (SYNC_QUEUE)
│       └── conflict_merge.rs       # Specific merge logic for complex types (e.g., merge_notes, merge_custom_data)
│
├── infrastructure/
│   ├── networking/                 # Existing networking module
│   │   ├── mod.rs
│   │   ├── protocols/
│   │   │   ├── sync/               # Pointer to sync/protocol/handler.rs for integration
│   │   │   └── pairing/            # Existing pairing protocol
│   │   │       ├── mod.rs
│   │   │       ├── security.rs
│   │   │       ├── persistence.rs
│   │   │       ├── protocol.rs     # Likely contains PairingProtocolHandler logic
│   │   │       ├── rate_limiter.rs # New: for pairing request rate limiting
│   │   │       └── user_confirmation.rs # New: for user confirmation logic
│   │   └── persistent/             # Existing persistent networking components
│   │       ├── service.rs          # `NetworkingService` - will register SyncProtocolHandler
│   │       ├── manager.rs          # `PersistentConnectionManager`
│   │       ├── identity.rs         # `PersistentNetworkIdentity`
│   │       ├── storage.rs          # `SecureStorage` (used by sync for encrypted log data)
│   │       ├── messages.rs         # `DeviceMessage` enum (includes sync messages)
│   │       └── leader_election.rs  # New: Dedicated logic for leader election messages/state
│   │
│   ├── database/                   # Existing database integration
│   │   ├── mod.rs
│   │   ├── models/
│   │   │   ├── mod.rs
│   │   │   ├── sync_log_entry.rs   # Model definition for sync_log table
│   │   │   ├── sync_position.rs    # Model definition for sync_positions table
│   │   │   └── (other models that implement Syncable)
│   │   └── behaviors.rs            # SeaORM ActiveModelBehavior implementations for `after_save` hooks
│   │
│   └── config/                     # Application configuration
│       └── mod.rs                  # AppConfig (includes max_paired_devices_per_library, max_total_paired_devices, sync_log_retention_policy)
│
└── core/                           # Main application core
    ├── mod.rs
    └── services.rs                 # Initializes NetworkingService and SyncJobManager
```
