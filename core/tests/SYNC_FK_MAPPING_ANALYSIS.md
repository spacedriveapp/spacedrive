# Sync Foreign Key Mapping Problem

## The Fundamental Issue

**Auto-incrementing primary keys are local to each database instance.**

### Concrete Example

**Device A's database.db**:
```sql
devices:
  id=1, uuid=aaaa-aaaa-aaaa-aaaa (Device A - registered first)
  id=2, uuid=bbbb-bbbb-bbbb-bbbb (Device B - registered second)

locations:
  id=1, uuid=loc-123, device_id=1, entry_id=5
```

**Device B's database.db**:
```sql
devices:
  id=1, uuid=bbbb-bbbb-bbbb-bbbb (Device B - registered itself first!)
  id=2, uuid=aaaa-aaaa-aaaa-aaaa (Device A - registered second!)

locations:
  id=?, uuid=loc-123, device_id=?, entry_id=?
```

### When Syncing Location from A→B

**What gets sent** (current broken approach):
```json
{
  "uuid": "loc-123",
  "device_id": 1,     // ← Device A's local ID
  "entry_id": 5,      // ← Device A's local entry ID
  "name": "Photos"
}
```

**What Device B tries to do**:
```sql
INSERT INTO locations (uuid, device_id, entry_id, ...)
VALUES ('loc-123', 1, 5, ...);
--                 ^  ^
--                 |  |
--  Device B, not A! |
--     Entry probably doesn't exist
```

**Result**: FOREIGN KEY constraint failed ❌

## Solution Options

### Option 1: ✅ Sync UUIDs, Map on Apply (RECOMMENDED)

**Principle**: Integer IDs are local DB implementation details. UUIDs are the global truth.

#### Implementation

**In `to_sync_json()` - Include UUIDs**:
```rust
impl Syncable for location::Model {
    fn to_sync_json(&self) -> Result<serde_json::Value> {
        // Look up device UUID from local device_id
        let device = devices::Entity::find_by_id(self.device_id)
            .one(db).await?
            .ok_or_else(|| Error::DeviceNotFound)?;

        // Look up entry UUID from local entry_id
        let entry = entries::Entity::find_by_id(self.entry_id)
            .one(db).await?
            .ok_or_else(|| Error::EntryNotFound)?;

        Ok(json!({
            "uuid": self.uuid,
            "device_uuid": device.uuid,  // ← Sync UUID, not local ID
            "entry_uuid": entry.uuid,     // ← Sync UUID, not local ID
            "name": self.name,
            "index_mode": self.index_mode,
            // ... other fields (no FKs)
        }))
    }
}
```

**In `apply_state_change()` - Map UUIDs to Local IDs**:
```rust
impl Model {
    pub async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<()> {
        // 1. Extract UUIDs from sync data
        let location_uuid: Uuid = data["uuid"].as_str()?.parse()?;
        let device_uuid: Uuid = data["device_uuid"].as_str()?.parse()?;
        let entry_uuid: Uuid = data["entry_uuid"].as_str()?.parse()?;

        // 2. Map device UUID → local device ID
        let device = devices::Entity::find()
            .filter(devices::Column::Uuid.eq(device_uuid))
            .one(db)
            .await?
            .ok_or_else(|| Error::DeviceNotFoundForUuid(device_uuid))?;

        // 3. Map entry UUID → local entry ID
        let entry = entries::Entity::find()
            .filter(entries::Column::Uuid.eq(entry_uuid))
            .one(db)
            .await?
            .ok_or_else(|| Error::EntryNotFoundForUuid(entry_uuid))?;

        // 4. Build model with LOCAL IDs
        let location = ActiveModel {
            id: NotSet, // Will be set by upsert
            uuid: Set(location_uuid),
            device_id: Set(device.id),  // ← Mapped to local ID
            entry_id: Set(entry.id),     // ← Mapped to local ID
            name: Set(data["name"].as_str().map(String::from)),
            // ... other non-FK fields
            ..Default::default()
        };

        // 5. Upsert by UUID (idempotent)
        Entity::insert(location)
            .on_conflict(
                OnConflict::column(Column::Uuid)
                    .update_columns([
                        Column::DeviceId,
                        Column::EntryId,
                        Column::Name,
                        // ...
                    ])
                    .to_owned()
            )
            .exec(db)
            .await?;

        Ok(())
    }
}
```

#### Pros:
- ✅ Keeps integer PKs (SQLite performance)
- ✅ Keeps existing schema structure
- ✅ Clear separation: UUIDs for sync, integers for local queries
- ✅ Works with existing migrations

#### Cons:
- ⚠️ Requires UUID lookup on every sync apply (minimal overhead)
- ⚠️ More complex apply logic
- ⚠️ Need to handle missing dependencies (see below)

---

### Option 2: ❌ UUIDs as Primary Keys

**Change schema**:
```sql
CREATE TABLE devices (
    uuid UUID PRIMARY KEY,  -- No auto-increment!
    name TEXT,
    ...
);

CREATE TABLE locations (
    uuid UUID PRIMARY KEY,
    device_uuid UUID REFERENCES devices(uuid),  -- Direct UUID FK
    entry_uuid UUID REFERENCES entries(uuid),
    ...
);
```

#### Pros:
- ✅ No mapping needed - UUIDs sync directly
- ✅ Simpler apply logic
- ✅ No local ID confusion

#### Cons:
- ❌ **MASSIVE migration effort** (rewrite entire schema)
- ❌ **Breaking change** (all existing DBs invalid)
- ❌ Slower JOINs on SQLite (UUID string comparison vs integer)
- ❌ More disk space (16 bytes vs 4 bytes per FK)
- ❌ Existing queries need rewriting

**Verdict**: Not worth it for a mature codebase.

---

### Option 3: ⚠️ Sync Local IDs with Translation Table

**Add translation table**:
```sql
CREATE TABLE id_mappings (
    remote_device_id UUID,    -- Which device's ID system
    model_type TEXT,          -- "device", "entry", etc.
    remote_id INTEGER,        -- Their local ID
    local_id INTEGER,         -- Our local ID
    uuid UUID,                -- Global identifier
    PRIMARY KEY (remote_device_id, model_type, remote_id)
);
```

**On sync**:
```rust
// Receive: device_id=1 from Device A
// Lookup: id_mappings WHERE remote_device_id=A AND model_type='device' AND remote_id=1
// Get: local_id=2, uuid=device-a-uuid
// Use: device_id=2 in our database
```

#### Pros:
- ✅ No schema changes to existing tables
- ✅ Can sync any integer reference

#### Cons:
- ❌ Complex! Extra table, extra lookups
- ❌ Hard to maintain consistency
- ❌ Doesn't solve the real problem (we should use UUIDs for sync anyway)

---

## Recommended Solution: Option 1 + Dependency Handling

### Phase 1: Update Syncable Trait

```rust
pub trait Syncable {
    // ... existing methods ...

    /// Convert model to sync JSON with UUIDs for all foreign keys
    ///
    /// Default implementation serializes the whole model, but models with FKs
    /// should override to include UUID mappings.
    fn to_sync_json(&self) -> Result<serde_json::Value> {
        // Default: just serialize (works for models without FKs like Tag)
        Ok(serde_json::to_value(self)?)
    }

    /// List of FK fields that need UUID mapping
    ///
    /// Example: ["device_id", "entry_id", "parent_id"]
    fn foreign_key_fields() -> &'static [(&'static str, &'static str)] {
        // Returns: [(local_id_field, uuid_field), ...]
        &[]
    }
}
```

### Phase 2: Implement for Location

```rust
impl Syncable for location::Model {
    fn to_sync_json(&self) -> Result<serde_json::Value> {
        // Query for UUIDs
        let device = devices::Entity::find_by_id(self.device_id)
            .one(db).await?.unwrap();
        let entry = entries::Entity::find_by_id(self.entry_id)
            .one(db).await?.unwrap();

        Ok(json!({
            "uuid": self.uuid,
            "device_uuid": device.uuid,      // ← UUID instead of ID
            "entry_uuid": entry.uuid.unwrap(), // ← UUID instead of ID
            "name": self.name,
            "index_mode": self.index_mode,
            "scan_state": self.scan_state,
            // ... all non-FK fields as-is
        }))
    }

    fn foreign_key_fields() -> &'static [(&'static str, &'static str)] {
        &[
            ("device_id", "device_uuid"),
            ("entry_id", "entry_uuid"),
        ]
    }

    async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<()> {
        #[derive(Deserialize)]
        struct LocationSyncData {
            uuid: Uuid,
            device_uuid: Uuid,  // ← FK as UUID
            entry_uuid: Uuid,    // ← FK as UUID
            name: Option<String>,
            index_mode: String,
            // ... other fields
        }

        let sync_data: LocationSyncData = serde_json::from_value(data)?;

        // Map UUIDs to local IDs
        let device = devices::Entity::find()
            .filter(devices::Column::Uuid.eq(sync_data.device_uuid))
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("Device {} not found", sync_data.device_uuid))?;

        let entry = entries::Entity::find()
            .filter(entries::Column::Uuid.eq(Some(sync_data.entry_uuid)))
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("Entry {} not found", sync_data.entry_uuid))?;

        // Build with local IDs
        let location = ActiveModel {
            id: NotSet,
            uuid: Set(sync_data.uuid),
            device_id: Set(device.id),   // ← Local ID
            entry_id: Set(entry.id),      // ← Local ID
            name: Set(sync_data.name),
            index_mode: Set(sync_data.index_mode),
            ..Default::default()
        };

        // Upsert by UUID
        Entity::insert(location)
            .on_conflict(
                OnConflict::column(Column::Uuid)
                    .update_columns([/* all fields */])
                    .to_owned()
            )
            .exec(db)
            .await?;

        Ok(())
    }
}
```

### Phase 3: Handle Missing Dependencies

What if the entry doesn't exist yet?

```rust
let entry = entries::Entity::find()
    .filter(entries::Column::Uuid.eq(Some(sync_data.entry_uuid)))
    .one(db)
    .await?;

match entry {
    Some(e) => {
        // Dependency exists, proceed
        location.entry_id = Set(e.id);
        insert_location(location).await?;
    }
    None => {
        // Dependency missing - two options:

        // Option A: Queue for retry (recommended)
        return Err(ApplyError::MissingDependency {
            model: "entry",
            uuid: sync_data.entry_uuid,
        });
        // Caller will catch this and queue for retry after entries sync

        // Option B: Create stub entry (risky)
        let stub = create_stub_entry(sync_data.entry_uuid).await?;
        location.entry_id = Set(stub.id);
        insert_location(location).await?;
    }
}
```

## The Big Question: Should We Use UUID PKs?

I think the answer is **NO** for existing tables, but here's the analysis:

### Current Approach (Integer PKs + UUIDs)

**Schema**:
```sql
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,     -- Local, auto-increment
    uuid UUID UNIQUE NOT NULL,  -- Global, for sync
    ...
);
```

**Pros**:
- ✅ SQLite JOINs are fast (integer comparison)
- ✅ Less disk space (4 bytes vs 16 bytes per FK)
- ✅ Existing codebase works as-is
- ✅ Clear separation: IDs for local, UUIDs for global

**Cons**:
- ⚠️ Sync must map UUIDs ↔ local IDs
- ⚠️ Apply functions are more complex

### UUID-Only PKs

**Schema**:
```sql
CREATE TABLE devices (
    uuid UUID PRIMARY KEY,  -- Both local AND global
    ...
);
```

**Pros**:
- ✅ Sync is trivial (no mapping needed)
- ✅ No ID translation bugs
- ✅ Cleaner mental model

**Cons**:
- ❌ **Breaking schema change** (huge migration)
- ❌ Slower JOINs (UUID string comparison)
- ❌ More disk space
- ❌ All existing queries need updates
- ❌ Specta/API layer might assume integer IDs

## Recommended Approach

**Keep integer PKs, always sync UUIDs for FKs.**

### Step 1: Update Syncable Trait

```rust
pub trait Syncable {
    // Existing...
    const SYNC_MODEL: &'static str;
    fn sync_id(&self) -> Uuid;

    // NEW: Convert to sync format
    /// Convert to sync JSON with UUIDs for all foreign keys.
    ///
    /// Models with FK relationships MUST override this to include UUID mappings.
    /// Models without FKs (like Tag) can use the default implementation.
    async fn to_sync_json(&self, db: &DatabaseConnection) -> Result<serde_json::Value, SyncError> {
        // Default: serialize as-is (only works for models without FKs)
        Ok(serde_json::to_value(self)?)
    }

    // NEW: Helper for FK mapping
    /// Declare FK fields that need UUID mapping.
    /// Format: (local_id_column, uuid_column_in_referenced_table, referenced_table)
    fn foreign_key_mappings() -> Vec<ForeignKeyMapping> {
        vec![]
    }
}

pub struct ForeignKeyMapping {
    pub local_field: &'static str,    // "device_id"
    pub uuid_field: &'static str,      // "device_uuid" (what to call it in sync JSON)
    pub target_table: &'static str,    // "devices"
    pub target_uuid_column: &'static str, // "uuid"
}
```

### Step 2: Implement for Each Model

**Location** (has device_id, entry_id FKs):
```rust
impl Syncable for location::Model {
    async fn to_sync_json(&self, db: &DatabaseConnection) -> Result<serde_json::Value> {
        let device = devices::Entity::find_by_id(self.device_id).one(db).await?.unwrap();
        let entry = entries::Entity::find_by_id(self.entry_id).one(db).await?.unwrap();

        Ok(json!({
            "uuid": self.uuid,
            "device_uuid": device.uuid,
            "entry_uuid": entry.uuid.unwrap(),
            "name": self.name,
            // ... all other non-FK fields
        }))
    }

    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        let device_uuid: Uuid = /* extract */;
        let entry_uuid: Uuid = /* extract */;

        // Map to local IDs
        let device_id = uuid_to_local_id("devices", device_uuid, db).await?;
        let entry_id = uuid_to_local_id("entries", entry_uuid, db).await?;

        // Insert with local IDs
        // ...
    }
}
```

**Tag** (no FKs - simpler!):
```rust
impl Syncable for tag::Model {
    async fn to_sync_json(&self, _db: &DatabaseConnection) -> Result<serde_json::Value> {
        // No FKs, just serialize directly
        Ok(serde_json::to_value(self)?)
    }

    async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
        // No ID mapping needed!
        let tag: Model = serde_json::from_value(entry.data)?;

        Entity::insert(tag.into())
            .on_conflict(/* ... */)
            .exec(db)
            .await?;

        Ok(())
    }
}
```

**Entry** (has parent_id FK - self-referential!):
```rust
impl Syncable for entry::Model {
    async fn to_sync_json(&self, db: &DatabaseConnection) -> Result<serde_json::Value> {
        let parent_uuid = if let Some(parent_id) = self.parent_id {
            let parent = entries::Entity::find_by_id(parent_id).one(db).await?.unwrap();
            parent.uuid
        } else {
            None
        };

        Ok(json!({
            "uuid": self.uuid,
            "parent_uuid": parent_uuid,  // ← Self-referential FK as UUID
            "name": self.name,
            // ... other fields
        }))
    }

    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        let parent_uuid: Option<Uuid> = /* extract */;

        let parent_id = if let Some(uuid) = parent_uuid {
            Some(uuid_to_local_id("entries", uuid, db).await?)
        } else {
            None
        };

        // Insert with local parent_id
        // ...
    }
}
```

### Step 3: Dependency Ordering

This is why `compute_sync_order()` exists!

**Sync order** (from dependency graph):
```
1. devices  (no dependencies)
2. entries  (self-referential, but handled)
3. locations (depends on devices, entries)
4. tags (no dependencies)
```

**During backfill**:
```rust
for model_type in sync_order {
    sync_model(model_type).await?;
}
// Guarantees: Parents always exist before children
```

**During real-time sync**:
```rust
match apply_state_change(data, db).await {
    Ok(()) => { /* Success */ }
    Err(ApplyError::MissingDependency { model, uuid }) => {
        // Queue for retry after dependency arrives
        retry_queue.push_until_dependency_exists(model, uuid, data);
    }
    Err(e) => { /* Real error */ }
}
```

## Practical Implementation Plan

### 1. Helper Function (Add to registry.rs)

```rust
/// Map UUID to local integer ID for any table
pub async fn uuid_to_local_id(
    table: &str,
    uuid: Uuid,
    db: &DatabaseConnection,
) -> Result<i32, ApplyError> {
    match table {
        "devices" => {
            let device = devices::Entity::find()
                .filter(devices::Column::Uuid.eq(uuid))
                .one(db)
                .await?
                .ok_or(ApplyError::MissingDependency {
                    model: "device",
                    uuid
                })?;
            Ok(device.id)
        }
        "entries" => {
            let entry = entries::Entity::find()
                .filter(entries::Column::Uuid.eq(Some(uuid)))
                .one(db)
                .await?
                .ok_or(ApplyError::MissingDependency {
                    model: "entry",
                    uuid
                })?;
            Ok(entry.id)
        }
        _ => Err(ApplyError::UnknownTable(table.to_string()))
    }
}
```

### 2. Update TransactionManager to use to_sync_json()

```rust
pub async fn commit_device_owned(
    &self,
    library: &Library,  // ← Need DB access for UUID lookups
    model: &impl Syncable,
) -> Result<()> {
    // Use the model's to_sync_json() which includes UUID mappings
    let sync_data = model.to_sync_json(library.db().conn()).await?;

    self.event_bus.emit(Event::Custom {
        event_type: "sync:state_change".to_string(),
        data: json!({
            "model_type": model::SYNC_MODEL,
            "record_uuid": model.sync_id(),
            "device_id": device_id,
            "data": sync_data,  // ← Contains UUIDs for FKs
            // ...
        }),
    });

    Ok(())
}
```

### 3. Update Tests to Include UUID Assertions

```rust
#[tokio::test]
async fn test_uuid_mapping() {
    // Device A: device_id=1, uuid=aaaa
    // Device B: device_id=2, uuid=aaaa (different local ID!)

    // Create location on A with device_id=1
    let location_a = create_location(device_id=1);

    // Sync to B
    let sync_data = location_a.to_sync_json(db_a).await?;

    // Verify UUID in sync data
    assert_eq!(sync_data["device_uuid"], "aaaa");

    // Apply on B
    apply_state_change(sync_data, db_b).await?;

    // Verify it mapped to device_id=2 on B
    let location_b = find_location(uuid).await?;
    assert_eq!(location_b.device_id, 2); // Different local ID!
    assert_eq!(lookup_device_uuid(location_b.device_id), "aaaa"); // Same device!
}
```

## Why This is The Right Approach

1. **Backwards Compatible**: No schema changes
2. **Performant**: Integer JOINs stay fast
3. **Clear**: UUIDs are the sync protocol, integers are local optimization
4. **Testable**: Your integration tests will validate mapping
5. **Incremental**: Can implement model-by-model

## Next Actions

1. ✅ Tests now fail on FK constraints (showing what to fix)
2. → Implement `to_sync_json()` for location with UUID mapping
3. → Implement `apply_state_change()` with UUID→ID mapping
4. → Add `uuid_to_local_id()` helper
5. → Watch tests pass! 🎯

