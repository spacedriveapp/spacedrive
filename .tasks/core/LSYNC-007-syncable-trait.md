---
id: LSYNC-007
title: Syncable Trait (Device Ownership Aware)
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, trait, codegen, macro]
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-10-14
---

## Description

Create the `Syncable` trait that database models implement to enable automatic sync. In the leaderless model, the trait must distinguish between device-owned and shared resources.

**Architecture Update**: Trait now indicates ownership to determine sync strategy (state-based vs log-based).

## Implementation Steps

1. Define `Syncable` trait with:
   - `SYNC_MODEL: &'static str` - Model identifier
   - `sync_id() -> Uuid` - Global resource ID
   - `is_device_owned() -> bool` - Determines sync strategy
   - `device_id() -> Option<Uuid>` - Owner device (if device-owned)
   - `exclude_fields()` - Optional field exclusion
2. Create `#[derive(Syncable)]` macro
3. Implement for device-owned models: Location, Entry, Volume
4. Implement for shared models: Tag, Album
5. Document ownership patterns

## Technical Details

- Location: `core/src/infra/sync/syncable.rs`
- Macro location: `crates/sync-derive/src/lib.rs`
- Must integrate with SeaORM models
- Ownership determines: state-based (device-owned) vs log-based (shared)

## Example Usage

### Device-Owned Resource

```rust
impl Syncable for locations::Model {
    const SYNC_MODEL: &'static str = "location";

    fn sync_id(&self) -> Uuid { self.uuid }

    fn is_device_owned(&self) -> bool { true }

    fn device_id(&self) -> Option<Uuid> { Some(self.device_id) }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "created_at", "updated_at"])
    }
}

// Sync strategy: State broadcast (no log)
```

### Shared Resource

```rust
impl Syncable for tags::Model {
    const SYNC_MODEL: &'static str = "tag";

    fn sync_id(&self) -> Uuid { self.uuid }

    fn is_device_owned(&self) -> bool { false }  // Shared!

    fn device_id(&self) -> Option<Uuid> { None }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "created_at"])
    }
}

// Sync strategy: HLC-based log
```

## TransactionManager Integration

```rust
impl TransactionManager {
    pub async fn commit<M: Syncable>(&self, model: M) -> Result<M> {
        if model.is_device_owned() {
            self.commit_device_owned(model).await  // State-based
        } else {
            self.commit_shared(model).await        // Log-based with HLC
        }
    }
}
```

## Acceptance Criteria

- [x] `Syncable` trait defined with ownership methods
- [x] Works with SeaORM models
- [x] Device-owned models: location, entry, volume, device
- [x] Shared models: tag, collection, content_identity, user_metadata
- [x] Field exclusion functional
- [x] FK mappings for integer FKs to UUIDs
- [x] Registry for dynamic dispatch
- [x] Integration tests validate sync (10 tests passing)

## References

- `core/src/infra/sync/NEW_SYNC.md` - Data ownership classification
- Device-owned examples: Lines 58-126
- Shared examples: Lines 130-179
