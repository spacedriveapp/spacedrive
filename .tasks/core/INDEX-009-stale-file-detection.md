---
id: INDEX-009
title: Intelligent Stale Detection Service with Modified-Time Optimization
status: To Do
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, stale-detection, offline-recovery, sync, service, modified-time]
whitepaper: Section 4.3.4
last_updated: 2025-12-23
related_tasks: [INDEX-004, LSYNC-020, LOC-000]
---

## Executive Summary

Implement an intelligent stale detection service that leverages the existing indexer infrastructure with a new **modified-time pruning mode**. When enabled, the indexer's discovery phase compares directory modified times between filesystem and database, **pruning unchanged branches** to avoid unnecessary scanning. This dramatically reduces overhead compared to full re-indexing.

This task also establishes a **service-per-location configuration architecture** where locations can individually enable/disable services (watcher, stale detector, sync) with custom settings, managed globally by the core and configured via UI.

## Problem Statement

The real-time change detection system (ChangeHandler trait) only captures events while Spacedrive is running and actively watching locations. When the app is:

- Stopped/offline
- Crashed unexpectedly
- Watcher paused or disabled
- Running on a different device

...filesystem changes are not immediately detected. Traditional full re-indexing is slow and wasteful for large directories when only a small subset has changed.

## Core Innovation: Modified-Time Pruning in Discovery Phase

### Visualization of How It Works

Imagine visualizing the indexing process as a tree traversal animation:

```
/Photos (mtime: 2025-12-20, DB: 2025-12-20) ✓ UNCHANGED
  ↓ Stop here - no need to explore children

/Documents (mtime: 2025-12-22, DB: 2025-12-10) ✗ CHANGED
  ↓ Continue down this branch
  /Reports (mtime: 2025-12-22, DB: 2025-12-10) ✗ CHANGED
    ↓ Mark for indexing
    /Q4 (mtime: 2025-12-22, DB: 2025-12-10) ✗ CHANGED
      ↓ Add to indexing paths: [/Documents/Reports/Q4]
  /Archives (mtime: 2025-11-01, DB: 2025-11-01) ✓ UNCHANGED
    ↓ Stop here

/Videos (mtime: 2025-12-23, DB: 2025-12-01) ✗ CHANGED
  ↓ Add to indexing paths: [/Videos]
```

**Result**: Discovery phase skips `[/Photos, /Documents/Archives]` entirely, only processes `[/Documents/Reports/Q4, /Videos]`.

### Algorithm (Integrated into Discovery Phase)

**Key Insight**: Leverage existing indexer infrastructure - don't reimplement tree walking!

1. **Add `IndexMode::Stale` variant** that wraps the location's index mode
2. **In discovery phase** (`core/src/ops/indexing/phases/discovery.rs`):
   - When encountering a directory entry
   - If mode is `IndexMode::Stale(_)`:
     - Query database for existing entry's modified time
     - Compare with filesystem modified time (1-second tolerance)
     - If times match → **Skip enqueuing** this directory (pruning)
     - If times differ → Continue as normal (enqueue for exploration)
   - Track pruning statistics (directories pruned vs explored)
   - Index changed parts using the wrapped mode (Shallow/Content/Deep)

3. **StaleDetectionService** spawns `IndexerJob` with:
   ```rust
   // Get location's configured index mode
   let location = self.get_location(location_id).await?;

   IndexerJobConfig {
       location_id: Some(location_id),
       path: location_root_path,
       mode: IndexMode::Stale(Box::new(location.index_mode)), // Respects location setting!
       scope: IndexScope::Recursive,
       persistence: IndexPersistence::Persistent,
       max_depth: None,
       rule_toggles: Default::default(),
   }
   ```

This leverages all existing indexer infrastructure: parallel workers, batching, progress tracking, change detection, etc.

## Proposed Architecture

### 1. IndexMode Extension

#### Add Stale Variant to IndexMode

```rust
// Location: core/src/domain/location.rs

/// Indexing depth and strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IndexMode {
    /// Don't index this location
    None,

    /// Index filesystem metadata only (name, size, dates)
    Shallow,

    /// Index metadata + content hashes for deduplication
    Content,

    /// Index metadata + content + thumbnails + text extraction
    Deep,

    /// NEW: Stale detection mode - uses mtime pruning with wrapped mode for changed parts
    /// Wraps the actual indexing mode to use (respects location's configured depth)
    Stale(Box<IndexMode>),
}

impl IndexMode {
    /// Check if this mode enables mtime pruning
    pub fn uses_mtime_pruning(&self) -> bool {
        matches!(self, IndexMode::Stale(_))
    }

    /// Get the inner mode for indexing changed parts
    pub fn inner_mode(&self) -> &IndexMode {
        match self {
            IndexMode::Stale(inner) => inner,
            other => other,
        }
    }
}
```

#### IndexerJobConfig (No Changes Needed)

The existing `IndexerJobConfig` works as-is - just pass `IndexMode::Stale(...)`:

```rust
// Location: core/src/ops/indexing/job.rs

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
    pub location_id: Option<Uuid>,
    pub path: SdPath,
    pub mode: IndexMode, // Can now be IndexMode::Stale(Box<IndexMode>)
    pub scope: IndexScope,
    pub persistence: IndexPersistence,
    pub max_depth: Option<u32>,
    pub rule_toggles: RuleToggles,
}
```

### 2. Discovery Phase Modifications

#### Modified Worker Logic

```rust
// Location: core/src/ops/indexing/phases/discovery.rs

pub async fn run_discovery_phase(
    state: &mut IndexerState,
    ctx: &JobContext<'_>,
    root_path: &Path,
    rule_toggles: RuleToggles,
    index_mode: &IndexMode, // NEW: Pass index mode
    // ... other params
) -> Result<(), JobError> {
    // Check if we should use mtime pruning
    let use_mtime_pruning = index_mode.uses_mtime_pruning();

    // Pass to workers
    run_parallel_discovery(
        state,
        ctx,
        root_path,
        rule_toggles,
        use_mtime_pruning, // NEW PARAM
        // ... other params
    ).await
}

async fn discovery_worker_rayon(
    // ... existing params
    use_mtime_pruning: bool, // NEW PARAM
    db: Arc<DatabaseConnection>, // NEW PARAM (for querying)
) {
    loop {
        // ... existing work reception logic

        match read_directory(&dir_path, volume_backend, cloud_url_base).await {
            Ok(entries) => {
                let mut local_stats = LocalStats::default();

                for entry in entries {
                    // ... existing rule evaluation

                    match entry.kind {
                        EntryKind::Directory => {
                            // NEW: Check if we should prune this directory
                            if use_mtime_pruning && should_prune_directory(
                                &entry,
                                &db,
                            ).await {
                                local_stats.pruned += 1; // NEW STAT
                                // Don't enqueue - skip this subtree
                                continue;
                            }

                            local_stats.dirs += 1;
                            pending_work.fetch_add(1, Ordering::Release);
                            if work_tx.send(entry.path.clone()).await.is_err() {
                                pending_work.fetch_sub(1, Ordering::Release);
                            }
                            let _ = result_tx.send(DiscoveryResult::Entry(entry)).await;
                        }
                        // ... existing File/Symlink handling
                    }
                }

                // ... existing stats sending (now includes pruned count)
            }
            // ... existing error handling
        }
    }
}
```

#### Pruning Decision Logic

```rust
// Location: core/src/ops/indexing/phases/discovery.rs

/// Check if a directory should be pruned based on modified time comparison
async fn should_prune_directory(
    entry: &DirEntry,
    db: &DatabaseConnection,
) -> bool {
    // Get filesystem modified time
    let Some(fs_mtime) = entry.modified else {
        return false; // No mtime available, can't prune
    };

    // Query database for existing entry
    let db_entry = match query_entry_mtime(db, &entry.path).await {
        Ok(Some(entry)) => entry,
        Ok(None) => return false, // Not in DB, definitely changed
        Err(_) => return false, // Query failed, don't prune (safe default)
    };

    // Compare modified times with tolerance
    times_match(fs_mtime, db_entry.mtime)
}

/// Query database for entry's modified time using directory_paths cache
async fn query_entry_mtime(
    db: &DatabaseConnection,
    path: &Path,
) -> Result<Option<EntryMtimeRecord>> {
    // Use directory_paths table for O(1) lookup
    // SELECT entries.id, entries.modified_at
    // FROM directory_paths
    // JOIN entries ON directory_paths.entry_id = entries.id
    // WHERE directory_paths.path = ?

    use crate::infra::db::entities::{directory_paths, entries};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let path_str = path.to_string_lossy().to_string();

    let result = directory_paths::Entity::find()
        .find_also_related(entries::Entity)
        .filter(directory_paths::Column::Path.eq(path_str))
        .one(db)
        .await?;

    match result {
        Some((_, Some(entry_model))) => Ok(Some(EntryMtimeRecord {
            id: entry_model.id,
            mtime: entry_model.modified_at,
        })),
        _ => Ok(None),
    }
}

struct EntryMtimeRecord {
    id: i32,
    mtime: DateTime<Utc>,
}

/// Compare filesystem time with database time (1-second tolerance)
fn times_match(fs_time: SystemTime, db_time: DateTime<Utc>) -> bool {
    let fs_datetime: DateTime<Utc> = fs_time.into();
    let diff = (fs_datetime - db_time).num_seconds().abs();
    diff <= 1
}
```

#### Statistics Tracking

```rust
// Location: core/src/ops/indexing/state.rs

#[derive(Default)]
struct LocalStats {
    files: u64,
    dirs: u64,
    symlinks: u64,
    bytes: u64,
    pruned: u64, // NEW: Directories skipped via mtime pruning
}

// Update IndexerStats to include pruning metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IndexerStats {
    pub files: u64,
    pub dirs: u64,
    pub symlinks: u64,
    pub bytes: u64,
    pub skipped: u64,
    pub pruned: u64, // NEW
}
```

### 3. Service Infrastructure

#### StaleDetectionService

```rust
// Location: core/src/service/stale_detector/mod.rs

pub struct StaleDetectionService {
    db: Arc<DatabaseConnection>,
    job_manager: Arc<JobManager>,
    context: Arc<CoreContext>,
    config: StaleDetectorServiceConfig,

    // Per-location worker tasks
    location_workers: Arc<RwLock<HashMap<Uuid, LocationWorker>>>,

    // Shutdown signal
    shutdown: Arc<Notify>,
}

impl StaleDetectionService {
    /// Trigger stale detection for a location
    pub async fn detect_stale(
        &self,
        location_id: Uuid,
        location_path: PathBuf,
        trigger: StaleDetectionTrigger,
    ) -> Result<String> {
        info!("Triggering stale detection for location {}", location_id);

        // Get location's configured index mode
        let location = self.get_location(location_id).await?;

        // Spawn IndexerJob with Stale mode (wraps location's mode)
        let config = IndexerJobConfig {
            location_id: Some(location_id),
            path: SdPath::from_path(&location_path)?,
            mode: IndexMode::Stale(Box::new(location.index_mode)), // Respects location setting!
            scope: IndexScope::Recursive,
            persistence: IndexPersistence::Persistent,
            max_depth: None,
            rule_toggles: Default::default(),
        };

        let job_id = self.job_manager
            .dispatch(IndexerJob::new(config))
            .await?;

        // Record run in history
        self.record_detection_run(location_id, &job_id, trigger).await?;

        Ok(job_id)
    }

    /// Check if location needs stale detection
    async fn should_detect_stale(
        &self,
        location_id: Uuid,
    ) -> Result<bool> {
        // Get watcher state
        let watcher_state = self.get_watcher_state(location_id).await?;

        // Get location settings
        let settings = self.get_location_settings(location_id).await?;

        // Decision logic
        if watcher_state.watch_interrupted {
            return Ok(true);
        }

        let offline_duration = Utc::now() - watcher_state.last_watch_stop;
        let threshold = Duration::seconds(settings.offline_threshold_secs as i64);

        Ok(offline_duration > threshold)
    }
}
```

**Key Point**: The service is simple - it just decides when to trigger, then spawns an `IndexerJob` with `enable_mtime_pruning: true`. All the actual work happens in the existing indexer infrastructure.

### 4. Location Service Settings

#### Database Schema Extensions

```sql
-- New table for service settings per location
CREATE TABLE location_service_settings (
    location_id INTEGER PRIMARY KEY REFERENCES locations(id),

    -- Watcher settings
    watcher_enabled BOOLEAN NOT NULL DEFAULT true,
    watcher_config TEXT, -- JSON: { "debounce_ms": 150, "batch_size": 10000 }

    -- Stale detector settings
    stale_detector_enabled BOOLEAN NOT NULL DEFAULT true,
    stale_detector_config TEXT, -- JSON: { "check_interval_secs": 3600, "aggressiveness": "normal" }

    -- Sync settings (file sync per location)
    sync_enabled BOOLEAN NOT NULL DEFAULT false,
    sync_config TEXT, -- JSON: { "mode": "mirror", "conflict_resolution": "newest_wins" }

    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Watcher lifecycle tracking
CREATE TABLE location_watcher_state (
    location_id INTEGER PRIMARY KEY REFERENCES locations(id),
    last_watch_start TIMESTAMP,
    last_watch_stop TIMESTAMP,
    last_successful_event TIMESTAMP,
    watch_interrupted BOOLEAN DEFAULT false,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Stale detection history
CREATE TABLE stale_detection_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    location_id INTEGER NOT NULL REFERENCES locations(id),
    job_id TEXT NOT NULL, -- Reference to IndexerJob
    triggered_by TEXT NOT NULL, -- "startup", "periodic", "manual", "offline_threshold"
    started_at TIMESTAMP NOT NULL,
    completed_at TIMESTAMP,
    status TEXT NOT NULL, -- "running", "completed", "failed"
    directories_pruned INTEGER DEFAULT 0, -- NEW: Pruning efficiency metric
    directories_scanned INTEGER DEFAULT 0,
    changes_detected INTEGER DEFAULT 0,
    error_message TEXT
);
```

#### Domain Models

```rust
// Location: core/src/domain/location.rs

#[derive(Clone, Debug)]
pub struct LocationServiceSettings {
    pub location_id: Uuid,
    pub watcher: WatcherSettings,
    pub stale_detector: StaleDetectorSettings,
    pub sync: SyncSettings,
}

#[derive(Clone, Debug)]
pub struct WatcherSettings {
    pub enabled: bool,
    pub config: WatcherConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub debounce_ms: u64,
    pub batch_size: usize,
    pub recursive: bool,
}

#[derive(Clone, Debug)]
pub struct StaleDetectorSettings {
    pub enabled: bool,
    pub config: StaleDetectorConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaleDetectorConfig {
    /// How often to check this location (seconds)
    pub check_interval_secs: u64,

    /// "conservative" | "normal" | "aggressive"
    pub aggressiveness: String,

    /// Run on startup if offline > this duration (seconds)
    pub offline_threshold_secs: u64,

    /// Enable verbose logging for this location
    pub verbose_logging: bool,
}

#[derive(Clone, Debug)]
pub struct SyncSettings {
    pub enabled: bool,
    pub config: SyncConfig,
}

pub enum StaleDetectionTrigger {
    Startup,
    Periodic,
    Manual,
    OfflineThreshold,
}
```

### 5. Service-Per-Location Management

#### ServiceCoordinator

```rust
// Location: core/src/service/coordinator.rs

/// Coordinates service lifecycle based on location settings
pub struct ServiceCoordinator {
    db: Arc<DatabaseConnection>,
    watcher_service: Arc<FsWatcherService>,
    stale_detector_service: Arc<StaleDetectionService>,
    sync_service: Option<Arc<SyncService>>,
}

impl ServiceCoordinator {
    /// Apply service settings to a location
    pub async fn apply_location_settings(
        &self,
        location_id: Uuid,
        settings: LocationServiceSettings,
    ) -> Result<()> {
        // Update database
        self.save_location_settings(location_id, &settings).await?;

        // Watcher
        if settings.watcher.enabled {
            self.watcher_service.watch_location_with_config(
                location_id,
                settings.watcher.config
            ).await?;
        } else {
            self.watcher_service.unwatch_location(&location_id).await?;
        }

        // Stale Detector
        if settings.stale_detector.enabled {
            self.stale_detector_service.enable_for_location(
                location_id,
                settings.stale_detector.config
            ).await?;
        } else {
            self.stale_detector_service.disable_for_location(&location_id).await?;
        }

        // Sync (if available)
        if let Some(sync) = &self.sync_service {
            if settings.sync.enabled {
                sync.enable_for_location(location_id, settings.sync.config).await?;
            } else {
                sync.disable_for_location(&location_id).await?;
            }
        }

        Ok(())
    }

    /// Get current settings for a location
    pub async fn get_location_settings(
        &self,
        location_id: Uuid,
    ) -> Result<LocationServiceSettings> {
        todo!("Query location_service_settings table")
    }

    /// Initialize default settings when location is created
    pub async fn initialize_default_settings(
        &self,
        location_id: Uuid,
    ) -> Result<()> {
        let default_settings = LocationServiceSettings {
            location_id,
            watcher: WatcherSettings {
                enabled: true,
                config: WatcherConfig::default(),
            },
            stale_detector: StaleDetectorSettings {
                enabled: true,
                config: StaleDetectorConfig::default(),
            },
            sync: SyncSettings {
                enabled: false,
                config: SyncConfig::default(),
            },
        };

        self.apply_location_settings(location_id, default_settings).await
    }
}
```

### 6. Integration with Existing Systems

#### LocationManager Integration

```rust
// Location: core/src/location/manager.rs

impl LocationManager {
    pub async fn add_location(
        &self,
        // ... existing params
    ) -> LocationResult<(Uuid, String)> {
        // ... existing location creation logic

        // NEW: Initialize service settings
        self.service_coordinator
            .initialize_default_settings(location_id)
            .await?;

        // ... rest of existing logic
    }

    pub async fn remove_location(
        &self,
        library: &Library,
        location_id: Uuid,
    ) -> LocationResult<()> {
        // NEW: Stop all services for this location
        self.service_coordinator
            .stop_location_services(location_id)
            .await?;

        // ... existing removal logic
    }
}
```

#### Library Initialization

```rust
// Location: core/src/library/mod.rs

impl Library {
    pub async fn open(/* ... */) -> Result<Self> {
        // ... existing initialization

        // NEW: Start stale detection service
        let stale_detector = StaleDetectionService::new(
            db.clone(),
            job_manager.clone(),
            context.clone(),
        );
        stale_detector.start().await?;

        // NEW: On startup, check for stale locations
        self.check_stale_on_startup().await?;

        // ... rest of initialization
    }

    async fn check_stale_on_startup(&self) -> Result<()> {
        let locations = self.location_manager.list_locations().await?;

        for location in locations {
            // Get service settings
            let settings = self.service_coordinator
                .get_location_settings(location.id)
                .await?;

            if !settings.stale_detector.enabled {
                continue;
            }

            // Check if stale detection needed
            if self.stale_detector.should_detect_stale(location.id).await? {
                info!("Running startup stale detection for location {}", location.name);

                // Trigger detection (spawns IndexerJob with mtime pruning)
                self.stale_detector.detect_stale(
                    location.id,
                    location.path,
                    StaleDetectionTrigger::Startup,
                ).await?;
            }
        }

        Ok(())
    }
}
```

### 7. UI Integration

#### RSPC Router Extensions

```rust
// Location: core/src/api/locations.rs

router.mutation("locations.updateServiceSettings", |t| {
    t(|ctx, input: UpdateLocationServicesInput| async move {
        ctx.service_coordinator
            .apply_location_settings(input.location_id, input.settings)
            .await
    })
});

router.query("locations.getServiceSettings", |t| {
    t(|ctx, location_id: Uuid| async move {
        ctx.service_coordinator
            .get_location_settings(location_id)
            .await
    })
});

router.mutation("locations.triggerStaleDetection", |t| {
    t(|ctx, location_id: Uuid| async move {
        let location = ctx.get_location(location_id).await?;

        let job_id = ctx.stale_detector.detect_stale(
            location_id,
            location.path,
            StaleDetectionTrigger::Manual,
        ).await?;

        Ok(job_id)
    })
});
```

#### Interface Components

**Location Inspector - Service Settings Tab**

```tsx
// Location: packages/interface/src/components/LocationInspector/ServiceSettings.tsx

export function LocationServiceSettings({ locationId }: { locationId: string }) {
  const { data: settings } = useQuery({
    queryKey: ['locations.getServiceSettings', locationId],
    queryFn: () => bridge.query(['locations.getServiceSettings', locationId])
  });

  const updateSettings = useMutation({
    mutationFn: (input: UpdateLocationServicesInput) =>
      bridge.mutation(['locations.updateServiceSettings', input])
  });

  return (
    <div className="space-y-6">
      <ServiceCard
        title="File Watcher"
        description="Real-time monitoring of filesystem changes"
        enabled={settings?.watcher.enabled}
        onToggle={(enabled) => updateSettings.mutate({ locationId, watcher: { ...settings.watcher, enabled } })}
      >
        {/* watcher config controls */}
      </ServiceCard>

      <ServiceCard
        title="Stale Detection"
        description="Automatic scanning for offline changes using modified-time pruning"
        enabled={settings?.stale_detector.enabled}
      >
        <ConfigRow label="Check Interval">
          <Select
            value={settings?.stale_detector.config.check_interval_secs}
            options={[
              { label: '30 minutes', value: 1800 },
              { label: '1 hour', value: 3600 },
              { label: '6 hours', value: 21600 },
            ]}
          />
        </ConfigRow>

        <Button onClick={() => bridge.mutation(['locations.triggerStaleDetection', locationId])}>
          Run Stale Detection Now
        </Button>
      </ServiceCard>

      <ServiceCard
        title="Multi-Device Sync"
        description="Keep this location synced across devices"
        enabled={settings?.sync.enabled}
      >
        {/* sync config controls */}
      </ServiceCard>
    </div>
  );
}
```

## Implementation Plan

### Phase 1: IndexMode Extension & Discovery Pruning

**Files**:
- `core/src/domain/location.rs` - Add `IndexMode::Stale` variant
- `core/src/ops/indexing/phases/discovery.rs` - Implement pruning logic
- `core/src/ops/indexing/state.rs` - Add pruned statistics

**Tasks**:
1. Add `IndexMode::Stale(Box<IndexMode>)` variant to enum
2. Add `uses_mtime_pruning()` and `inner_mode()` helper methods
3. Update all match statements on IndexMode to handle Stale variant
4. Pass index mode to `run_discovery_phase()`
5. Implement `should_prune_directory()` function
6. Implement `query_entry_mtime()` database query (uses `directory_paths` join)
7. Implement `times_match()` comparison with 1-second tolerance
8. Update worker logic to check `use_mtime_pruning` and skip enqueuing
9. Add `pruned` field to `LocalStats` and `IndexerStats`
10. Update stats aggregation to include pruning metrics
11. Unit tests for pruning logic
12. Update processing/content phases to use `inner_mode()` for actual indexing depth

### Phase 2: Database Schema & Domain Models

**Files**:
- `core/src/infra/db/migrations/` - Add new tables
- `core/src/infra/db/entities/location_service_settings.rs` - Entity model
- `core/src/domain/location.rs` - Domain models

**Tasks**:
1. Add `location_service_settings` table
2. Add `location_watcher_state` table
3. Add `stale_detection_runs` table (with `directories_pruned` column)
4. Create domain models for service settings
5. Migration for existing locations (insert default settings)

### Phase 3: StaleDetectionService

**Files**:
- `core/src/service/stale_detector/mod.rs` - Main service
- `core/src/service/stale_detector/worker.rs` - Per-location workers

**Tasks**:
1. Implement `StaleDetectionService` struct
2. Service trait implementation (start/stop)
3. `detect_stale()` method that spawns IndexerJob
4. `should_detect_stale()` decision logic
5. Load locations with stale detection enabled
6. Create per-location workers
7. Periodic checking logic
8. Integration with global Services container
9. Record runs in `stale_detection_runs` table

### Phase 4: ServiceCoordinator

**Files**:
- `core/src/service/coordinator.rs` - Service coordination

**Tasks**:
1. Create `ServiceCoordinator` struct
2. Implement `apply_location_settings`
3. Implement `get_location_settings`
4. Database CRUD for `location_service_settings`
5. Integration with LocationManager
6. Default settings initialization

### Phase 5: Watcher State Tracking

**Files**:
- `core/src/service/watcher/service.rs` - Update watcher
- `core/src/ops/indexing/handlers/persistent.rs` - Update handler

**Tasks**:
1. Record watcher start/stop in `location_watcher_state`
2. Update `last_successful_event` on each event
3. Mark `watch_interrupted` on crash
4. Query watcher state for stale detection decisions

### Phase 6: Library Integration

**Files**:
- `core/src/library/mod.rs` - Library startup logic
- `core/src/location/manager.rs` - Location lifecycle

**Tasks**:
1. Start `StaleDetectionService` on library open
2. Run `check_stale_on_startup()`
3. Initialize service settings on location creation
4. Stop location services on location removal
5. Graceful shutdown

### Phase 7: RSPC API

**Files**:
- `core/src/api/locations.rs` - Location service mutations
- `core/src/api/services.rs` - Global service queries

**Tasks**:
1. Add `locations.updateServiceSettings` mutation
2. Add `locations.getServiceSettings` query
3. Add `locations.triggerStaleDetection` mutation
4. Add `services.getConfig` query
5. Add `services.updateConfig` mutation

### Phase 8: UI Components

**Files**:
- `packages/interface/src/components/LocationInspector/ServiceSettings.tsx`
- `packages/interface/src/screens/settings/Services.tsx`
- `packages/interface/src/components/ServiceCard.tsx` (new)

**Tasks**:
1. Create `ServiceCard` reusable component
2. Implement Location Inspector service settings tab
3. Implement App Settings services screen
4. Add "Run Stale Detection Now" button
5. Service status indicators
6. Configuration controls (sliders, selects, toggles)
7. Display pruning efficiency metrics (dirs pruned vs scanned)

### Phase 9: Documentation & Visualization

**Files**:
- `docs/core/indexing.mdx` - Update with stale detection section
- `docs/core/services.mdx` - New services documentation
- Marketing materials (diagrams)

**Tasks**:
1. Update indexing docs with modified-time pruning
2. Create service architecture documentation
3. Animated diagram showing tree pruning visualization
4. Example code snippets for developers
5. Troubleshooting guide

## Acceptance Criteria

### IndexMode Extension & Pruning
- [ ] `IndexMode::Stale(Box<IndexMode>)` variant added
- [ ] `uses_mtime_pruning()` helper method works
- [ ] `inner_mode()` returns wrapped mode correctly
- [ ] All existing match statements updated for Stale variant
- [ ] Discovery phase queries database for directory mtimes via `directory_paths` join
- [ ] Modified time comparison with 1-second tolerance
- [ ] Directories with matching mtimes are skipped (not enqueued)
- [ ] Statistics track directories pruned vs scanned
- [ ] Works with `directory_paths` cache for O(1) lookups
- [ ] Handles missing database entries gracefully (doesn't prune)
- [ ] Processing/Content phases use `inner_mode()` for indexing depth

### StaleDetectionService
- [ ] Implements Service trait (start/stop/is_running)
- [ ] Loads locations with stale detection enabled
- [ ] `detect_stale()` queries location's `index_mode` and wraps with `Stale`
- [ ] Spawns IndexerJob with `IndexMode::Stale(Box::new(location.index_mode))`
- [ ] Respects location's configured indexing depth (Shallow/Content/Deep)
- [ ] `should_detect_stale()` checks watcher state and thresholds
- [ ] Creates per-location workers with custom intervals
- [ ] Workers check periodically based on config
- [ ] Spawns jobs when staleness detected
- [ ] Records runs in `stale_detection_runs` table
- [ ] Graceful shutdown stops all workers

### Service Settings Architecture
- [ ] Database schema supports per-location service config
- [ ] Domain models for all service settings
- [ ] ServiceCoordinator applies settings to services
- [ ] Default settings initialized on location creation
- [ ] Settings persisted and loaded correctly

### Watcher Integration
- [ ] `location_watcher_state` tracks start/stop/events
- [ ] Timestamps updated correctly
- [ ] `watch_interrupted` flag set on crash
- [ ] Stale detection uses watcher state for decisions

### Library Lifecycle
- [ ] StaleDetectionService started on library open
- [ ] Startup check runs for offline locations
- [ ] Doesn't block app startup (background job)
- [ ] Service stopped on library shutdown
- [ ] Location services stopped on location removal

### API Layer
- [ ] `locations.updateServiceSettings` mutation works
- [ ] `locations.getServiceSettings` query returns correct data
- [ ] `locations.triggerStaleDetection` spawns job
- [ ] `services.getConfig` returns global config
- [ ] `services.updateConfig` updates global config

### UI Components
- [ ] Location Inspector has "Services" tab
- [ ] Each service has enable/disable toggle
- [ ] Service-specific config controls render
- [ ] "Run Stale Detection Now" button works
- [ ] App Settings has "Services" screen
- [ ] Global service config controls work
- [ ] Service status indicators show running/stopped
- [ ] Pruning efficiency displayed (e.g., "Pruned 90% of directories")

### Performance
- [ ] Pruning provides 10-1000x speedup depending on change density
- [ ] Doesn't block app startup
- [ ] Large locations (1M+ files) prompt user before auto-scan
- [ ] Per-location workers don't overwhelm system
- [ ] Modified time queries use `directory_paths` cache for O(1) performance

### Edge Cases
- [ ] External drive unmounted - skips stale detection
- [ ] Very long offline period - prompts user
- [ ] Multiple devices with same location - coordinate via sync
- [ ] Location with no previous index - doesn't prune (safe default)
- [ ] Crashed watcher - triggers stale detection on next startup
- [ ] Database query failure - doesn't prune (safe default)

## Testing Strategy

### Unit Tests

```rust
// Location: core/src/ops/indexing/phases/discovery.rs

#[cfg(test)]
mod tests {
    #[test]
    fn test_times_match_with_tolerance() {
        let db_time = Utc::now();
        let fs_time = SystemTime::from(db_time + Duration::milliseconds(500));

        assert!(times_match(fs_time, db_time)); // Within 1 second
    }

    #[test]
    fn test_times_dont_match() {
        let db_time = Utc::now();
        let fs_time = SystemTime::from(db_time + Duration::seconds(2));

        assert!(!times_match(fs_time, db_time)); // Beyond tolerance
    }
}
```

### Integration Tests

```rust
// Location: core/tests/stale_detection_test.rs

#[tokio::test]
async fn test_mtime_pruning_skips_unchanged_directories() {
    let harness = TestHarness::new().await;

    // Create location with nested directories
    harness.create_directory_tree("test", 3, 10).await; // 3 levels, 10 dirs per level
    let location_id = harness.create_location("test").await;
    harness.wait_for_indexing().await;

    // Modify only one subdirectory
    harness.create_file("test/level1/level2/new_file.txt").await;

    // Run stale detection with mtime pruning
    let job_id = harness.trigger_stale_detection(location_id).await;
    let stats = harness.wait_for_job(job_id).await;

    // Assert: Most directories were pruned
    assert!(stats.pruned > 900); // 90%+ of 1000 total dirs
    assert!(stats.dirs < 100); // Only changed branch scanned
}

#[tokio::test]
async fn test_stale_detection_without_pruning_scans_all() {
    let harness = TestHarness::new().await;

    // Same setup
    harness.create_directory_tree("test", 3, 10).await;
    let location_id = harness.create_location("test").await;
    harness.wait_for_indexing().await;

    harness.create_file("test/level1/level2/new_file.txt").await;

    // Run normal indexer (without pruning)
    let config = IndexerJobConfig {
        enable_mtime_pruning: false, // Pruning disabled
        // ... other config
    };
    let job_id = harness.spawn_indexer(config).await;
    let stats = harness.wait_for_job(job_id).await;

    // Assert: All directories scanned
    assert_eq!(stats.pruned, 0);
    assert!(stats.dirs >= 1000); // All dirs scanned
}
```

## Performance Characteristics

### Modified-Time Pruning Efficiency

| Scenario | Files in Location | Files Changed | Traditional Scan | With Pruning | Speedup |
|----------|-------------------|---------------|------------------|--------------|---------|
| Small edit | 10,000 | 10 | 10,000 | ~500 | 20x |
| Subdirectory | 100,000 | 1,000 | 100,000 | ~5,000 | 20x |
| Multiple dirs | 1,000,000 | 10,000 | 1,000,000 | ~50,000 | 20x |
| No changes | 1,000,000 | 0 | 1,000,000 | ~1,000 | 1000x |

**Key Insight**: Pruning provides 10-1000x speedup depending on change density. Best case (no changes) only needs to check top-level directories.

## Visualization for Marketing/Documentation

### Animated Diagram Concept

Create an animated SVG/video showing:

1. **Scene 1**: Full directory tree with modified times
   - Shows database vs filesystem modified times
   - Highlights unchanged (green) and changed (red) directories

2. **Scene 2**: Discovery workers traverse tree
   - Workers descend tree checking each directory
   - Stop at green (unchanged) nodes - "pruned" animation
   - Continue down red (changed) branches
   - Mark changed paths with icons

3. **Scene 3**: IndexerJob processes only changed paths
   - Show parallel indexing of only changed paths
   - Final database sync

4. **Scene 4**: Performance comparison
   - "Traditional: 10,000 directories scanned"
   - "Spacedrive: 247 directories scanned (97% pruned)"
   - "Speedup: 40x faster"

### Documentation Sections

**For Users**:
- "How Stale Detection Works" - explain modified-time pruning
- "Configuring Services Per Location" - UI guide
- "When to Run Manual Stale Detection" - best practices

**For Developers**:
- "Modified-Time Pruning Algorithm" - technical deep dive
- "Extending the Discovery Phase" - adding new pruning strategies
- "Service-Per-Location Architecture" - design patterns

## Related Tasks

- INDEX-004 - Change Detection System (provides ChangeDetector foundation)
- INDEX-007 - Index Verification System (provides manual verification)
- INDEX-001 - Hybrid Indexing Architecture (discovery phase infrastructure)
- INDEX-002 - Five-Phase Indexing Pipeline (phase structure)
- LSYNC-020 - Device-Owned Deletion Sync (conflict resolution for multi-device)
- LSYNC-010 - Sync Service (pattern for service-per-location architecture)
- FSYNC-003 - File Sync Service (similar service coordination)
- LOC-000 - Location Operations (watcher lifecycle)

## Open Questions

1. **Pruning Threshold**: Should we have a minimum size threshold before enabling pruning?
   - Proposal: Only enable for locations with >10K files (overhead vs benefit)

2. **Cache Warming**: Should we warm the `directory_paths` cache before pruning?
   - Proposal: Preload cache for location root path to avoid N+1 queries

3. **Aggressiveness Levels**: What do "conservative", "normal", "aggressive" actually mean?
   - Conservative: Check every 6 hours, require 2-hour offline before triggering
   - Normal: Check every 1 hour, require 1-hour offline
   - Aggressive: Check every 15 minutes, trigger immediately on startup

4. **Sync Coordination**: How does stale detection coordinate with library sync?
   - Proposal: Stale detection waits for library sync to complete first
   - Stale detection emits sync events for detected changes

5. **Visualization**: Should the animated diagram be interactive (click to explore)?
   - Proposal: Create both static (for docs) and interactive (for landing page)
