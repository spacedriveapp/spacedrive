# UUID Mapping Design: Automatic vs Manual

## The Challenge

Can we automatically map integer FKs to UUIDs across all models, or does each model need custom logic?

## Option 1: ❌ Fully Automatic (Not Possible in Rust)

```rust
// What we'd LOVE to have:
impl Syncable for location::Model {
    // Magic! Automatically detects device_id and entry_id are FKs
    // Automatically looks up their UUIDs
    // Automatically maps back on apply
}
```

**Why not possible**:
- Rust has no runtime reflection
- SeaORM entities don't expose FK metadata at runtime
- Would need proc macros to analyze struct fields

**Verdict**: Too complex for the benefit.

---

## Option 2: ✅ Semi-Automatic with FK Declarations (RECOMMENDED)

Models declare their FKs, generic code handles the mapping.

### Step 1: Extend Syncable Trait

```rust
pub trait Syncable: Sized {
    const SYNC_MODEL: &'static str;

    fn sync_id(&self) -> Uuid;

    /// Declare which fields are FKs and where they point
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![] // Default: no FKs
    }

    /// Convert to sync JSON (uses FK declarations)
    async fn to_sync_json(&self, db: &DatabaseConnection) -> Result<serde_json::Value> {
        // Default implementation that works for most models
        let mut json = serde_json::to_value(self)?;

        // Automatically convert declared FKs to UUIDs
        for fk in Self::foreign_key_mappings() {
            convert_fk_to_uuid(&mut json, &fk, db).await?;
        }

        Ok(json)
    }

    /// Apply from sync data (uses FK declarations)
    async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<()>;

    async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
        Self::apply_state_change(entry.data, db).await
    }
}

/// FK mapping declaration
pub struct FKMapping {
    pub local_field: &'static str,      // "device_id" in the model
    pub target_table: &'static str,     // "devices"
    pub target_entity: fn() -> (),      // Type marker for the target entity
}
```

### Step 2: Implement for Location (Simple Declaration)

```rust
impl Syncable for location::Model {
    const SYNC_MODEL: &'static str = "location";

    fn sync_id(&self) -> Uuid {
        self.uuid
    }

    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            FKMapping {
                local_field: "device_id",
                target_table: "devices",
            },
            FKMapping {
                local_field: "entry_id",
                target_table: "entries",
            },
        ]
    }

    // to_sync_json() uses default implementation that auto-converts FKs!

    async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<()> {
        // Use generic FK mapper
        let mut data = data;
        map_uuids_to_local_ids(&mut data, Self::foreign_key_mappings(), db).await?;

        // Now data has local IDs, can deserialize
        let location: Model = serde_json::from_value(data)?;

        // Upsert
        Entity::insert(location.into())
            .on_conflict(/* ... */)
            .exec(db)
            .await?;

        Ok(())
    }
}
```

### Step 3: Generic FK Mapping Helpers

```rust
// In core/src/infra/sync/fk_mapper.rs

/// Convert a local FK integer ID to its UUID
pub async fn convert_fk_to_uuid(
    json: &mut serde_json::Value,
    fk: &FKMapping,
    db: &DatabaseConnection,
) -> Result<()> {
    let local_id: i32 = json[fk.local_field]
        .as_i64()
        .ok_or(SyncError::InvalidFK)? as i32;

    // Look up UUID based on target table
    let uuid = match fk.target_table {
        "devices" => {
            devices::Entity::find_by_id(local_id)
                .one(db)
                .await?
                .ok_or(SyncError::FKNotFound)?
                .uuid
        }
        "entries" => {
            entries::Entity::find_by_id(local_id)
                .one(db)
                .await?
                .ok_or(SyncError::FKNotFound)?
                .uuid
                .ok_or(SyncError::EntryMissingUuid)?
        }
        _ => return Err(SyncError::UnknownTable(fk.target_table.to_string()))
    };

    // Replace integer ID with UUID in JSON
    json[format!("{}_uuid", fk.local_field)] = json!(uuid);
    json.as_object_mut().unwrap().remove(fk.local_field); // Remove integer ID

    Ok(())
}

/// Convert UUID FK back to local integer ID
pub async fn map_uuids_to_local_ids(
    json: &mut serde_json::Value,
    mappings: Vec<FKMapping>,
    db: &DatabaseConnection,
) -> Result<()> {
    for fk in mappings {
        let uuid_field = format!("{}_uuid", fk.local_field);
        let uuid: Uuid = json[&uuid_field]
            .as_str()
            .ok_or(SyncError::MissingUUID)?
            .parse()?;

        // Look up local ID
        let local_id = match fk.target_table {
            "devices" => {
                devices::Entity::find()
                    .filter(devices::Column::Uuid.eq(uuid))
                    .one(db)
                    .await?
                    .ok_or(ApplyError::MissingDependency {
                        model: "device",
                        uuid
                    })?
                    .id
            }
            "entries" => {
                entries::Entity::find()
                    .filter(entries::Column::Uuid.eq(Some(uuid)))
                    .one(db)
                    .await?
                    .ok_or(ApplyError::MissingDependency {
                        model: "entry",
                        uuid
                    })?
                    .id
            }
            _ => return Err(ApplyError::UnknownTable(fk.target_table.to_string()))
        };

        // Replace UUID with local ID
        json[fk.local_field] = json!(local_id);
        json.as_object_mut().unwrap().remove(&uuid_field);
    }

    Ok(())
}
```

### Usage Example

**Location** (3 lines of declaration, rest is automatic):
```rust
impl Syncable for location::Model {
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            FKMapping { local_field: "device_id", target_table: "devices" },
            FKMapping { local_field: "entry_id", target_table: "entries" },
        ]
    }

    // to_sync_json() - uses default implementation (automatic!)

    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        // Generic mapper handles the FK conversion
        let mut data = data;
        map_uuids_to_local_ids(&mut data, Self::foreign_key_mappings(), db).await?;

        // Deserialize with local IDs
        let location: Model = serde_json::from_value(data)?;

        // Standard upsert
        upsert_by_uuid(location).await?;
        Ok(())
    }
}
```

**Tag** (no FKs - even simpler!):
```rust
impl Syncable for tag::Model {
    // No foreign_key_mappings() override needed - defaults to empty vec

    // to_sync_json() - default works perfectly (no FKs to convert)

    async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
        // No mapping needed, just deserialize and insert
        let tag: Model = serde_json::from_value(entry.data)?;
        upsert_by_uuid(tag).await?;
        Ok(())
    }
}
```

---

## Option 3: ⚠️ Derive Macro (Over-Engineering?)

```rust
#[derive(Syncable)]
#[syncable(
    model = "location",
    fk(device_id -> devices.uuid),
    fk(entry_id -> entries.uuid)
)]
pub struct Model {
    pub id: i32,
    pub uuid: Uuid,
    pub device_id: i32,  // ← Macro detects this
    pub entry_id: i32,    // ← Macro detects this
    // ...
}
```

**Pros**:
- Truly automatic
- Compile-time validation

**Cons**:
- Complex proc macro code
- Harder to debug
- Overkill for ~10 models

**Verdict**: Option 2 (declarative) is sweet spot.

---

## Recommended Implementation Strategy

### Phase 1: Core Infrastructure (1-2 hours)

```rust
// File: core/src/infra/sync/fk_mapper.rs

pub struct FKMapping {
    pub local_field: &'static str,
    pub target_table: &'static str,
}

pub async fn convert_fk_to_uuid(...) -> Result<()> { /* ... */ }
pub async fn map_uuids_to_local_ids(...) -> Result<()> { /* ... */ }
```

### Phase 2: Update Syncable Trait (30 min)

Add:
- `foreign_key_mappings()` method with default impl
- Update `to_sync_json()` default to use FK mappings

### Phase 3: Implement Per-Model (2-3 hours)

For each model with FKs:
1. Override `foreign_key_mappings()` - 3 lines
2. Implement `apply_state_change()` - 10-15 lines using helpers

Models without FKs (Tag, Album): Nothing to do! ✅

### Phase 4: Watch Tests Pass

Your integration tests will immediately validate the mapping works correctly.

## The Beauty of This Approach

**90% automatic, 10% declarative**:
```rust
// All you write:
fn foreign_key_mappings() -> Vec<FKMapping> {
    vec![
        FKMapping { local_field: "device_id", target_table: "devices" },
    ]
}

// Everything else is generic helper code that works for ALL models!
```

This gives you:
- ✅ Type safety
- ✅ Compile-time checks
- ✅ Minimal boilerplate
- ✅ Debuggable (no magic)
- ✅ Testable (clear transformation)

Want me to implement the `fk_mapper.rs` module with the generic helpers?
