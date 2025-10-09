# Issue: to_sync_json() Needs Database Access for FK Mapping

## The Problem

Current `to_sync_json()` signature:
```rust
fn to_sync_json(&self) -> Result<serde_json::Value, serde_json::Error>
```

But to convert FKs to UUIDs, we need to query the database!

```rust
let device = devices::Entity::find_by_id(self.device_id)
    .one(db).await?;  // ← Need database connection!
```

## Solution: Make to_sync_json() Async with DB

```rust
async fn to_sync_json(&self, db: &DatabaseConnection) -> Result<serde_json::Value> {
    let mut json = serde_json::to_value(self)?;

    // Convert FKs to UUIDs (requires DB lookups)
    for fk in Self::foreign_key_mappings() {
        convert_fk_to_uuid(&mut json, &fk, db).await?;
    }

    Ok(json)
}
```

This means:
1. trait method becomes async
2. Requires `&DatabaseConnection` parameter
3. Can do FK → UUID lookups

**Impact**: Minimal - only affects TransactionManager and backfill code that calls it.

