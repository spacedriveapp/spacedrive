---
id: LSYNC-009
title: Hybrid Logical Clock (HLC) Implementation
status: Done
assignee: jamiepine
parent: LSYNC-000
priority: High
tags: [sync, hlc, distributed-systems, leaderless]
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-10-14
---

## Description

Implement Hybrid Logical Clocks (HLC) for ordering shared resource changes in a leaderless sync model. HLC provides total ordering and causality tracking without requiring a central leader.

**Architecture Change**: Replacing leader-based sequence numbers with distributed HLC timestamps.

## Why HLC?

**Problem**: Need global ordering for conflict resolution of shared resources (tags, albums)
**Old Solution**: Leader assigns sequences → bottleneck, offline issues
**New Solution**: Each device generates HLC independently → no bottleneck, works offline

**Key Properties**:

- Total ordering (any two HLCs comparable)
- Causality tracking (if A→B then HLC(A) < HLC(B))
- Distributed generation (no coordination needed)
- Efficient (16 bytes: timestamp + counter + device_id)

## Implementation

**File**: `core/src/infra/sync/hlc.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct HLC {
    /// Physical timestamp (milliseconds since epoch)
    pub timestamp: u64,

    /// Logical counter for same millisecond
    pub counter: u64,

    /// Device that generated this clock
    pub device_id: Uuid,
}

impl HLC {
    /// Generate new HLC
    pub fn new(last: Option<HLC>, device_id: Uuid) -> Self {
        let now = Utc::now().timestamp_millis() as u64;

        match last {
            Some(last) if last.timestamp == now => {
                Self { timestamp: now, counter: last.counter + 1, device_id }
            }
            _ => {
                Self { timestamp: now, counter: 0, device_id }
            }
        }
    }

    /// Update based on received HLC (causality)
    pub fn update(&mut self, received: HLC) {
        self.timestamp = self.timestamp.max(received.timestamp);
        if self.timestamp == received.timestamp {
            self.counter = self.counter.max(received.counter) + 1;
        }
    }
}

pub struct HLCGenerator {
    device_id: Uuid,
    last_hlc: Option<HLC>,
}

impl HLCGenerator {
    pub fn next(&mut self) -> HLC {
        let hlc = HLC::new(self.last_hlc, self.device_id);
        self.last_hlc = Some(hlc);
        hlc
    }

    pub fn update(&mut self, received: HLC) {
        if let Some(ref mut last) = self.last_hlc {
            last.update(received);
        } else {
            self.last_hlc = Some(received);
        }
    }
}
```

## Integration Points

1. **SharedChangesDb**: Store HLC instead of sequence number
2. **TransactionManager**: Generate HLC for shared changes (no leader check!)
3. **SyncProtocolHandler**: Include HLC in `SharedChange` messages
4. **Conflict Resolution**: Sort by HLC to determine order

## Acceptance Criteria

- [ ] HLC struct with proper Ord implementation
- [ ] HLCGenerator per device
- [ ] Causality tracking on message receive
- [ ] Total ordering test (1000 random HLCs sort correctly)
- [ ] Integration with shared_changes table
- [ ] Serialization/deserialization works
- [ ] No leader checks in TransactionManager

## Migration

**Remove**:

- `sync_leadership` field from devices table
- `LeadershipManager` struct
- `is_leader()` checks

**Add**:

- `HLC` type
- `HLCGenerator` in SyncService
- HLC column in `shared_changes` table

## References

- Paper: "Logical Physical Clocks and Consistent Snapshots" (Kulkarni et al.)
- `core/src/infra/sync/NEW_SYNC.md` - Leaderless architecture
- Similar implementations: CockroachDB, TiDB
