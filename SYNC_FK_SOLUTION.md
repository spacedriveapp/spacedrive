# Sync FK Mapping Solution: 90% Automatic

## TL;DR

**✅ YES, it's mostly automatic!**

You declare FKs (3 lines), generic helpers do all the mapping.

## The Pattern

### For Models WITH FKs (Location, Entry, Album, etc.)

```rust
impl Syncable for location::Model {
    // ========================================
    // STEP 1: Declare FKs (one-time, 3 lines)
    // ========================================
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            FKMapping::new("device_id", "devices"),
            FKMapping::new("entry_id", "entries"),
        ]
    }

    // ========================================
    // STEP 2: Use generic mapper (5 lines)
    // ========================================
    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
        // ↑ This ONE line does all the UUID→ID mapping!

        let location: Model = serde_json::from_value(data)?;
        upsert_by_uuid(location, db).await?;
        Ok(())
    }
}
```

### For Models WITHOUT FKs (Tag, Album, UserMetadata)

```rust
impl Syncable for tag::Model {
    // No foreign_key_mappings() needed!

    async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
        let tag: Model = serde_json::from_value(entry.data)?; // Direct!
        upsert_by_uuid(tag, db).await?;
        Ok(())
    }
}
```

## How Automatic Mapping Works

### Example: Location Sync

**Device A's database.db**:
```
devices:    id=1, uuid='aaaa'
            id=2, uuid='bbbb'
entries:    id=5, uuid='entry-456'
locations:  id=3, uuid='loc-123', device_id=1, entry_id=5
```

**Device B's database.db** (different local IDs!):
```
devices:    id=1, uuid='bbbb'  ← Device B registered first!
            id=2, uuid='aaaa'  ← Device A registered second!
entries:    id=7, uuid='entry-456'  ← Entry synced earlier
```

### Sending from A

```rust
// 1. Location has device_id=1, entry_id=5
let location = /* from DB */;

// 2. Call to_sync_json() (uses default implementation)
let json = location.to_sync_json(db).await?;

// 3. Generic helper converts FKs based on declarations:
// foreign_key_mappings() = [("device_id", "devices"), ("entry_id", "entries")]

// For "device_id":
//   - local_id = 1
//   - Look up devices WHERE id=1 → uuid='aaaa'
//   - json["device_uuid"] = "aaaa"
//   - Remove json["device_id"]

// For "entry_id":
//   - local_id = 5
//   - Look up entries WHERE id=5 → uuid='entry-456'
//   - json["entry_uuid"] = "entry-456"
//   - Remove json["entry_id"]

// Result: { "uuid": "loc-123", "device_uuid": "aaaa", "entry_uuid": "entry-456", ... }
```

### Receiving on B

```rust
// 1. Received data: { "uuid": "loc-123", "device_uuid": "aaaa", "entry_uuid": "entry-456" }

// 2. Call map_sync_json_to_local()
let data = map_sync_json_to_local(data, foreign_key_mappings(), db).await?;

// 3. Generic helper converts UUIDs based on declarations:

// For "device_id":
//   - uuid = "aaaa"
//   - Look up devices WHERE uuid='aaaa' → finds id=2 (Device B's local ID for Device A!)
//   - json["device_id"] = 2
//   - Remove json["device_uuid"]

// For "entry_id":
//   - uuid = "entry-456"
//   - Look up entries WHERE uuid='entry-456' → finds id=7 (Device B's local ID!)
//   - json["entry_id"] = 7
//   - Remove json["entry_uuid"]

// Result: { "uuid": "loc-123", "device_id": 2, "entry_id": 7, ... }

// 4. Deserialize and insert with Device B's local IDs
let location: Model = serde_json::from_value(data)?;
location.insert(db).await?;  // Works! FKs are correct!
```

## Code Reuse

### Generic Helpers (Works for ALL models)

**In `fk_mapper.rs`** (~200 lines total, written once):
```rust
pub async fn convert_fk_to_uuid(...)    // ← Used by ALL models during send
pub async fn map_sync_json_to_local(...) // ← Used by ALL models during receive
pub async fn lookup_uuid_for_local_id(...)   // ← Generic table lookup
pub async fn lookup_local_id_for_uuid(...)   // ← Generic table lookup
```

### Per-Model Code (Minimal!)

**Models with FKs**: ~15 lines
- 3 lines: FK declarations
- 12 lines: apply function (mostly boilerplate)

**Models without FKs**: ~8 lines
- Just the apply function (no mapping!)

## What You Need to Add

### 1. Extend Syncable Trait

```rust
// In syncable.rs
pub trait Syncable {
    // ... existing methods ...

    /// Declare foreign key mappings (override if model has FKs)
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![] // Default: no FKs
    }
}
```

### 2. Update Each Model's apply_state_change()

Just use the generic helper:
```rust
async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
    let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
    let model: Model = serde_json::from_value(data)?;
    // ... upsert logic
}
```

### 3. Handle Missing Dependencies

```rust
match map_sync_json_to_local(data, mappings, db).await {
    Ok(data) => {
        // Dependencies exist, proceed
        apply_to_db(data).await?;
    }
    Err(e) if e.is_missing_dependency() => {
        // Dependency hasn't synced yet, queue for retry
        return Err(ApplyError::MissingDependency { /* ... */ });
    }
    Err(e) => return Err(e),
}
```

## Answer to Your Question

> "can we do this automatically for all domain models or do we need a Syncable method for this"

**Answer**: **Both!**

1. **Syncable method for declaration** (what to map):
   ```rust
   fn foreign_key_mappings() -> Vec<FKMapping> { /* ... */ }
   ```

2. **Automatic for execution** (how to map):
   ```rust
   map_sync_json_to_local(data, mappings, db).await  // ← Generic!
   ```

**Result**:
- Models just declare "device_id points to devices table"
- Generic code figures out the UUID lookups, queries, conversions
- Same helper code works for location, entry, album, everything!

## Next Step

Update the Syncable trait to include `foreign_key_mappings()` with a default implementation?

```rust
pub trait Syncable {
    // ... existing ...

    /// Declare FK mappings for automatic UUID conversion
    fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
        vec![] // Models without FKs use this default
    }
}
```

Then each model just overrides if it has FKs!

