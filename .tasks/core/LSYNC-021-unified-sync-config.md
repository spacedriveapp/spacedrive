---
id: LSYNC-021
title: Unified Sync Configuration System
status: Done
assignee: james
priority: Medium
tags: [sync, core, config]
last_updated: 2025-12-02
related_tasks: [LSYNC-020]
---

# Unified Sync Configuration System

## Problem Statement

Sync behavior is controlled by magic numbers scattered across the codebase. This makes it hard to understand defaults, impossible for users to tune sync performance, and difficult to test edge cases.

### Current State

```rust
// In backfill.rs
const DEFAULT_BATCH_SIZE: usize = 10_000;
const REQUEST_TIMEOUT_SECS: u64 = 60;

// In peer.rs
const SYNC_MESSAGE_TIMEOUT_SECS: u64 = 30;
const LOG_PRUNER_INTERVAL_SECS: u64 = 300;
const SYNC_LOOP_INTERVAL_SECS: u64 = 5;

// Scattered throughout
Duration::days(7)
Duration::days(25)
Duration::days(30)
```

**Problems:**
- No single source of truth
- Can't adjust sync behavior without code changes
- Different defaults across files
- No way to optimize for network conditions
- Testing requires modifying constants

## Design Goals

1. **Single source of truth** for all sync timing and batching parameters
2. **User-configurable** via CLI, UI, or config file
3. **Environment-aware** (LAN vs WAN, mobile vs desktop)
4. **Testable** (easily override for integration tests)
5. **Well-documented** defaults with clear rationale

## Proposed Solution: SyncConfig Structure

Centralize all sync configuration in a typed, serializable structure with presets for common scenarios.

## Technical Design

### Configuration Structure

```rust
// core/src/infra/sync/config.rs

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unified configuration for library sync behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub batching: BatchingConfig,
    pub retention: RetentionConfig,
    pub network: NetworkConfig,
    pub monitoring: MonitoringConfig,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            batching: BatchingConfig::default(),
            retention: RetentionConfig::default(),
            network: NetworkConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}
```

### Batching Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
    /// Records per batch for backfill requests
    pub backfill_batch_size: usize,

    /// Records per batch for state broadcast
    pub state_broadcast_batch_size: usize,

    /// Records per batch for shared resource broadcast
    pub shared_broadcast_batch_size: usize,

    /// Maximum snapshot size for current state
    pub max_snapshot_size: usize,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            backfill_batch_size: 10_000,
            state_broadcast_batch_size: 1_000,
            shared_broadcast_batch_size: 100,
            max_snapshot_size: 100_000,
        }
    }
}
```

### Retention Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Pruning strategy for sync coordination data
    pub strategy: PruningStrategy,

    /// Maximum retention for tombstones (days)
    pub tombstone_max_retention_days: u32,

    /// Maximum retention for peer log entries (days)
    pub peer_log_max_retention_days: u32,

    /// Force full sync if watermark older than this (days)
    pub force_full_sync_threshold_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            strategy: PruningStrategy::AcknowledgmentBased,
            tombstone_max_retention_days: 7,
            peer_log_max_retention_days: 7,
            force_full_sync_threshold_days: 25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PruningStrategy {
    /// Prune as soon as all devices acknowledge
    AcknowledgmentBased,

    /// Keep for minimum duration even if acknowledged
    Conservative { min_retention_days: u32 },

    /// Always keep for fixed duration
    TimeBased { retention_days: u32 },
}
```

### Network Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Timeout for sync message responses (seconds)
    pub message_timeout_secs: u64,

    /// Timeout for backfill requests (seconds)
    pub backfill_request_timeout_secs: u64,

    /// Interval between sync loop iterations (seconds)
    pub sync_loop_interval_secs: u64,

    /// Interval for connection health checks (seconds)
    pub connection_check_interval_secs: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            message_timeout_secs: 30,
            backfill_request_timeout_secs: 60,
            sync_loop_interval_secs: 5,
            connection_check_interval_secs: 10,
        }
    }
}
```

### Monitoring Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Interval for pruning sync coordination data (seconds)
    pub pruning_interval_secs: u64,

    /// Enable detailed sync metrics
    pub enable_metrics: bool,

    /// Log sync statistics at this interval (seconds)
    pub metrics_log_interval_secs: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            pruning_interval_secs: 3600,
            enable_metrics: true,
            metrics_log_interval_secs: 300,
        }
    }
}
```

## Preset Configurations

### Standard (Default)

Balanced for typical usage across LAN and internet connections.

```rust
SyncConfig::default()
```

### Aggressive

Fast local networks with always-online devices.

```rust
impl SyncConfig {
    pub fn aggressive() -> Self {
        Self {
            batching: BatchingConfig {
                backfill_batch_size: 5_000,
                state_broadcast_batch_size: 500,
                shared_broadcast_batch_size: 50,
                max_snapshot_size: 50_000,
            },
            retention: RetentionConfig {
                strategy: PruningStrategy::AcknowledgmentBased,
                tombstone_max_retention_days: 3,
                peer_log_max_retention_days: 3,
                force_full_sync_threshold_days: 2,
            },
            network: NetworkConfig {
                message_timeout_secs: 15,
                backfill_request_timeout_secs: 30,
                sync_loop_interval_secs: 2,
                connection_check_interval_secs: 5,
            },
            monitoring: MonitoringConfig {
                pruning_interval_secs: 1800,
                enable_metrics: true,
                metrics_log_interval_secs: 60,
            },
        }
    }
}
```

### Conservative

Unreliable networks with frequently offline devices.

```rust
impl SyncConfig {
    pub fn conservative() -> Self {
        Self {
            batching: BatchingConfig {
                backfill_batch_size: 25_000,
                state_broadcast_batch_size: 2_000,
                shared_broadcast_batch_size: 200,
                max_snapshot_size: 200_000,
            },
            retention: RetentionConfig {
                strategy: PruningStrategy::Conservative { min_retention_days: 7 },
                tombstone_max_retention_days: 30,
                peer_log_max_retention_days: 30,
                force_full_sync_threshold_days: 25,
            },
            network: NetworkConfig {
                message_timeout_secs: 60,
                backfill_request_timeout_secs: 120,
                sync_loop_interval_secs: 10,
                connection_check_interval_secs: 30,
            },
            monitoring: MonitoringConfig {
                pruning_interval_secs: 7200,
                enable_metrics: true,
                metrics_log_interval_secs: 600,
            },
        }
    }
}
```

### Mobile

Battery and bandwidth optimized for mobile devices.

```rust
impl SyncConfig {
    pub fn mobile() -> Self {
        Self {
            batching: BatchingConfig {
                backfill_batch_size: 5_000,
                state_broadcast_batch_size: 500,
                shared_broadcast_batch_size: 50,
                max_snapshot_size: 50_000,
            },
            retention: RetentionConfig {
                strategy: PruningStrategy::TimeBased { retention_days: 14 },
                tombstone_max_retention_days: 14,
                peer_log_max_retention_days: 14,
                force_full_sync_threshold_days: 10,
            },
            network: NetworkConfig {
                message_timeout_secs: 45,
                backfill_request_timeout_secs: 90,
                sync_loop_interval_secs: 30,
                connection_check_interval_secs: 60,
            },
            monitoring: MonitoringConfig {
                pruning_interval_secs: 14400,
                enable_metrics: false,
                metrics_log_interval_secs: 1800,
            },
        }
    }
}
```

## Configuration Loading

### Hybrid Approach (Recommended)

Load priority: Environment > File > Database > Default

```rust
impl SyncConfig {
    pub async fn load_for_library(library: &Library) -> Result<Self> {
        let mut config = SyncConfig::default();

        // 1. Load from library DB (per-library settings)
        if let Ok(db_config) = Self::load_from_db(library.id(), library.db()).await {
            config = db_config;
        }

        // 2. Load from config file (global overrides)
        let config_path = library.data_dir().join("sync_config.toml");
        if config_path.exists() {
            if let Ok(file_config) = Self::load_from_file(&config_path) {
                config = config.merge(file_config);
            }
        }

        // 3. Apply environment variable overrides
        config = config.apply_env_overrides();

        Ok(config)
    }

    async fn load_from_db(library_id: Uuid, db: &DatabaseConnection) -> Result<Self> {
        let row = entities::sync_config::Entity::find()
            .filter(Column::LibraryId.eq(library_id))
            .one(db)
            .await?;

        match row {
            Some(config) => serde_json::from_str(&config.config_json)?,
            None => Err(anyhow::anyhow!("No config in DB")),
        }
    }

    fn load_from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents)
    }

    fn apply_env_overrides(mut self) -> Self {
        if let Ok(val) = std::env::var("SD_SYNC_BATCH_SIZE") {
            if let Ok(size) = val.parse() {
                self.batching.backfill_batch_size = size;
            }
        }

        if let Ok(val) = std::env::var("SD_TOMBSTONE_RETENTION_DAYS") {
            if let Ok(days) = val.parse() {
                self.retention.tombstone_max_retention_days = days;
            }
        }

        // ... more overrides ...

        self
    }

    fn merge(self, other: Self) -> Self {
        // Merge two configs (other takes precedence for non-default values)
        Self {
            batching: other.batching,
            retention: other.retention,
            network: other.network,
            monitoring: other.monitoring,
        }
    }
}
```

## Integration Points

### SyncService Initialization

```rust
// core/src/service/sync/mod.rs

impl SyncService {
    pub async fn new(
        library: Arc<Library>,
        network: Arc<NetworkingService>,
    ) -> Result<Self> {
        // Load config for this library
        let config = SyncConfig::load_for_library(&library).await?;

        info!(
            "Initializing sync service with config: batch_size={}, retention={} days",
            config.batching.backfill_batch_size,
            config.retention.tombstone_max_retention_days
        );

        Self::new_with_config(library, network, config).await
    }

    pub async fn new_with_config(
        library: Arc<Library>,
        network: Arc<NetworkingService>,
        config: SyncConfig,
    ) -> Result<Self> {
        let config = Arc::new(config);

        // Spawn pruning task with configured interval
        tokio::spawn({
            let config = config.clone();
            let db = library.db().clone();
            async move {
                let mut interval = tokio::time::interval(
                    Duration::from_secs(config.monitoring.pruning_interval_secs)
                );

                loop {
                    interval.tick().await;
                    if let Err(e) = prune_sync_coordination_data(&db, &config.retention).await {
                        error!("Pruning failed: {}", e);
                    }
                }
            }
        });

        // ... rest of initialization using config ...

        Ok(Self {
            config,
            // ...
        })
    }

    /// Reload configuration (for runtime updates)
    pub async fn reload_config(&self, new_config: SyncConfig) -> Result<()> {
        // Atomic swap
        let old_config = self.config.clone();
        self.config = Arc::new(new_config);

        info!(
            "Reloaded sync config: batch_size {} -> {}",
            old_config.batching.backfill_batch_size,
            self.config.batching.backfill_batch_size
        );

        Ok(())
    }
}
```

### BackfillManager Usage

```rust
// core/src/service/sync/backfill.rs

impl BackfillManager {
    async fn request_state_batch(&self, peer: Uuid) -> Result<StateResponse> {
        let request = SyncMessage::StateRequest {
            library_id: self.library_id,
            model_types: vec!["entry"],
            device_id: None,
            since: None,
            checkpoint: None,
            batch_size: self.config.batching.backfill_batch_size,  // From config!
        };

        tokio::time::timeout(
            Duration::from_secs(self.config.network.backfill_request_timeout_secs),  // From config!
            self.peer_sync.network().send_sync_request(peer, request)
        ).await?
    }

    pub async fn catch_up_from_peer(
        &self,
        peer: Uuid,
        watermark: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let watermark_age = watermark.map(|w| chrono::Utc::now() - w);

        // Use configured threshold
        let threshold = chrono::Duration::days(
            self.config.retention.force_full_sync_threshold_days as i64
        );

        let effective_watermark = if watermark_age > Some(threshold) {
            warn!(
                "Watermark is {} days old (threshold: {} days), forcing full sync",
                watermark_age.unwrap().num_days(),
                self.config.retention.force_full_sync_threshold_days
            );
            None
        } else {
            watermark
        };

        // Continue with sync...
    }
}
```

### Pruning Logic

```rust
async fn prune_sync_coordination_data(
    db: &Database,
    config: &RetentionConfig,
) -> Result<()> {
    match &config.strategy {
        PruningStrategy::AcknowledgmentBased => {
            // Get min watermark from all devices
            let min_watermark = get_min_state_watermark(db).await?;

            // Apply safety limit
            let max_retention = chrono::Utc::now()
                - chrono::Duration::days(config.tombstone_max_retention_days as i64);

            let effective_cutoff = min_watermark
                .map(|w| w.min(max_retention))
                .unwrap_or(max_retention);

            // Prune tombstones
            let tombstones_pruned = prune_tombstones_before(db, effective_cutoff).await?;

            // Prune peer log (uses same pattern)
            let peer_log_pruned = prune_peer_log_acked(config.peer_log_max_retention_days).await?;

            info!(
                "Ack-based pruning: {} tombstones, {} peer log entries",
                tombstones_pruned, peer_log_pruned
            );
        }

        PruningStrategy::Conservative { min_retention_days } => {
            let min_watermark = get_min_state_watermark(db).await?;
            let min_cutoff = chrono::Utc::now()
                - chrono::Duration::days(*min_retention_days as i64);

            // Only prune if BOTH acknowledged AND past minimum retention
            if let Some(wm) = min_watermark {
                if wm < min_cutoff {
                    prune_tombstones_before(db, wm).await?;
                }
            }
        }

        PruningStrategy::TimeBased { retention_days } => {
            let cutoff = chrono::Utc::now()
                - chrono::Duration::days(*retention_days as i64);

            prune_tombstones_before(db, cutoff).await?;
        }
    }

    Ok(())
}
```

## User Interface

### CLI Commands

```bash
# View current config
sd sync config show

# Use preset
sd sync config set --preset aggressive
sd sync config set --preset conservative
sd sync config set --preset mobile

# Set individual values
sd sync config set --batch-size 5000
sd sync config set --retention-days 14
sd sync config set --pruning-strategy time-based

# Reset to defaults
sd sync config reset

# Per-library override
sd library "My Library" sync config set --preset conservative
```

### Config File Format

```toml
# ~/.config/spacedrive/sync.toml (global)
# or
# ~/Spacedrive/libraries/{library-id}/sync_config.toml (per-library)

[batching]
backfill_batch_size = 10000
state_broadcast_batch_size = 1000
shared_broadcast_batch_size = 100
max_snapshot_size = 100000

[retention]
# Options: "AcknowledgmentBased", "Conservative", "TimeBased"
strategy = "AcknowledgmentBased"
tombstone_max_retention_days = 7
peer_log_max_retention_days = 7
force_full_sync_threshold_days = 25

[network]
message_timeout_secs = 30
backfill_request_timeout_secs = 60
sync_loop_interval_secs = 5
connection_check_interval_secs = 10

[monitoring]
pruning_interval_secs = 3600
enable_metrics = true
metrics_log_interval_secs = 300
```

### Environment Variables

```bash
# Override any config value via environment
export SD_SYNC_BATCH_SIZE=5000
export SD_TOMBSTONE_RETENTION_DAYS=14
export SD_SYNC_LOOP_INTERVAL_SECS=10
export SD_PRUNING_STRATEGY=conservative
```

## Migration Plan

### Phase 1: Configuration Structure (1-2 hours)

1. Create `core/src/infra/sync/config.rs`
2. Define `SyncConfig` and all nested config structs
3. Implement `Default`, `Serialize`, `Deserialize`
4. Add preset constructors (aggressive, conservative, mobile)
5. Add to `core/src/infra/sync/mod.rs` exports

**Deliverable:** Configuration types defined and testable.

### Phase 2: Integration (2-3 hours)

1. Update `SyncService::new()` to accept `SyncConfig`
2. Replace all magic numbers in `backfill.rs` with config references
3. Replace all magic numbers in `peer.rs` with config references
4. Replace all magic numbers in pruning logic with config references
5. Thread `Arc<SyncConfig>` through all sync components

**Deliverable:** All magic numbers eliminated, config-driven.

### Phase 3: Persistence & Loading (1-2 hours)

1. Add `sync_config` table to library.db schema
2. Implement `load_from_db()`, `save_to_db()`
3. Implement `load_from_file()`, `save_to_file()`
4. Implement `apply_env_overrides()`
5. Implement priority loading (env > file > db > default)

**Deliverable:** Configuration can be persisted and loaded.

### Phase 4: CLI & UI (2-3 hours)

1. Add `sync config` subcommands to CLI
2. Implement show, set, reset commands
3. Add validation for config values
4. Add per-library config support

**Deliverable:** Users can configure sync via CLI.

### Phase 5: Testing & Documentation (1-2 hours)

1. Write unit tests for config loading/merging
2. Write integration tests with custom configs
3. Update library-sync.mdx with configuration section
4. Add examples for common scenarios

**Deliverable:** Production-ready sync configuration system.

**Total Estimate:** 7-12 hours

## Files Requiring Modification

**New Files (3):**
1. `core/src/infra/sync/config.rs` - Configuration types
2. `core/migrations/mXXXXXXXXX_add_sync_config.rs` - Database schema
3. `apps/cli/src/domains/sync/config.rs` - CLI commands

**Modified Files (6):**
4. `core/src/infra/sync/mod.rs` - Export config types
5. `core/src/service/sync/mod.rs` - Accept and use config
6. `core/src/service/sync/backfill.rs` - Replace constants with config
7. `core/src/service/sync/peer.rs` - Replace constants with config
8. `core/src/library/mod.rs` - Add config load/save methods
9. `apps/cli/src/domains/sync/mod.rs` - Add config subcommand

**Documentation (1):**
10. `docs/core/library-sync.mdx` - Add configuration section

**Total: 10 files**

## Benefits

1. **No magic numbers** - All timing/batching configured in one place
2. **User control** - Tune sync for network conditions
3. **Environment-aware** - Presets for LAN, WAN, mobile
4. **Testable** - Easy to test with small batches/short timeouts
5. **Discoverable** - Users can see what's configurable
6. **Documented** - Clear defaults with rationale

## Success Criteria

1. **Zero magic numbers** - All constants replaced with config references
2. **Single source of truth** - SyncConfig contains all tunable parameters
3. **User-configurable** - CLI commands work for viewing and setting config
4. **Preset validation** - Aggressive/Conservative/Mobile presets work as expected
5. **Tests pass** - Integration tests use custom configs successfully

## References

- Related: LSYNC-020 (uses retention config for tombstone pruning)
- Peer Log Implementation: `/core/src/infra/sync/peer_log.rs`
- Backfill Manager: `/core/src/service/sync/backfill.rs`
- Sync Service: `/core/src/service/sync/mod.rs`

---

**Next Steps:**
1. Review unified sync config design
2. Implement Phase 1 (config structure)
3. Integrate into sync service (Phase 2)
4. Add persistence and CLI (Phases 3-4)
