---
id: LSYNC-007
title: Syncable Trait & Derive Macros
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, trait, codegen, macro]
---

## Description

Create the `Syncable` trait that database models implement to enable automatic sync log creation. Includes derive macro for ergonomic implementation.

## Implementation Steps

1. Define `Syncable` trait with required methods:
   - `SYNC_MODEL: &'static str` - Model identifier
   - `sync_id() -> Uuid` - Global resource ID
   - `version() -> i64` - For conflict resolution
   - `exclude_fields()` - Optional field exclusion
   - `to_sync_json()` - Optional custom serialization
2. Create `#[derive(Syncable)]` macro
3. Implement for initial models: Album, Tag, Location
4. Add validation that `sync_id` is unique across model
5. Document field exclusion patterns (db IDs, timestamps)

## Technical Details

- Location: `core/src/infra/sync/syncable.rs`
- Macro location: `crates/sync-derive/src/lib.rs`
- Must integrate with SeaORM models
- `exclude_fields()` prevents platform-specific data from syncing

## Example Usage

```rust
impl Syncable for albums::Model {
    const SYNC_MODEL: &'static str = "album";

    fn sync_id(&self) -> Uuid { self.uuid }
    fn version(&self) -> i64 { self.version }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "created_at", "updated_at"])
    }
}
```

## Acceptance Criteria

- [ ] `Syncable` trait defined
- [ ] Derive macro implemented
- [ ] Works with SeaORM models
- [ ] Field exclusion functional
- [ ] Documentation with examples
- [ ] Unit tests for derive macro

## References

- `docs/core/sync.md` lines 60-118
