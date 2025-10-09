# FK Mapping: 90% Automatic

## How It Works

### What You Write (Per Model)

**For a model WITH foreign keys** (like Location):
```rust
impl Syncable for location::Model {
    // 1. Declare FKs (3 lines):
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            FKMapping::new("device_id", "devices"),
            FKMapping::new("entry_id", "entries"),
        ]
    }

    // 2. Use generic helpers in apply (5 lines):
    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        // Map UUIDs → local IDs (automatic based on declarations!)
        let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;

        // Deserialize with local IDs
        let location: Model = serde_json::from_value(data)?;

        // Standard upsert
        Entity::insert(location.into())
            .on_conflict(OnConflict::column(Column::Uuid).update_all().to_owned())
            .exec(db)
            .await?;
        Ok(())
    }
}
```

**For a model WITHOUT foreign keys** (like Tag):
```rust
impl Syncable for tag::Model {
    // No foreign_key_mappings() needed (defaults to empty vec)

    // Direct deserialization (no mapping needed!):
    async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
        let tag: Model = serde_json::from_value(entry.data)?;
        Entity::insert(tag.into())
            .on_conflict(OnConflict::column(Column::Uuid).update_all().to_owned())
            .exec(db)
            .await?;
        Ok(())
    }
}
```

## What Happens Automatically

### Sending (Device A → Device B)

**Before sync (Device A's DB)**:
```sql
locations:
  uuid='loc-123', device_id=1, entry_id=5

devices:
  id=1, uuid='aaaa'

entries:
  id=5, uuid='entry-456'
```

**to_sync_json() does**:
```rust
// 1. Serialize model
let mut json = serde_json::to_value(location)?;
// json = { "uuid": "loc-123", "device_id": 1, "entry_id": 5, ... }

// 2. For each FK mapping:
for fk in foreign_key_mappings() {  // ["device_id" → "devices", "entry_id" → "entries"]
    convert_fk_to_uuid(&mut json, &fk, db).await?;
}

// Result:
// json = {
//   "uuid": "loc-123",
//   "device_uuid": "aaaa",      ← device_id=1 converted to UUID
//   "entry_uuid": "entry-456",  ← entry_id=5 converted to UUID
//   "name": "Photos"
// }
```

**What gets sent over the wire**:
```json
{
  "uuid": "loc-123",
  "device_uuid": "aaaa",     // ← No local IDs!
  "entry_uuid": "entry-456", // ← Only UUIDs!
  "name": "Photos"
}
```

### Receiving (Device B)

**Before (Device B's DB)**:
```sql
devices:
  id=2, uuid='aaaa'  ← Same device, different local ID!

entries:
  id=7, uuid='entry-456'  ← Same entry, different local ID!
```

**apply_state_change() does**:
```rust
// 1. Map UUIDs back to Device B's local IDs
let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;

// Internally:
// - Looks up: devices WHERE uuid='aaaa' → finds id=2
// - Looks up: entries WHERE uuid='entry-456' → finds id=7
// - Replaces: device_uuid='aaaa' → device_id=2
// - Replaces: entry_uuid='entry-456' → entry_id=7

// data = { "uuid": "loc-123", "device_id": 2, "entry_id": 7, "name": "Photos" }

// 2. Deserialize with Device B's local IDs
let location: Model = serde_json::from_value(data)?;

// 3. Insert
location.insert(db).await?;
```

**After (Device B's DB)**:
```sql
locations:
  uuid='loc-123', device_id=2, entry_id=7  ← Mapped to local IDs!
```

## The Magic: Generic Helpers Do All The Work

### convert_fk_to_uuid() (reusable!)

```rust
// Called once per FK during serialization
pub async fn convert_fk_to_uuid(json: &mut Value, fk: &FKMapping, db: &DatabaseConnection) -> Result<()> {
    let local_id = json[fk.local_field].as_i64()? as i32;

    // Generic lookup based on table name
    let uuid = match fk.target_table {
        "devices" => devices::find_by_id(local_id).one(db).await?.uuid,
        "entries" => entries::find_by_id(local_id).one(db).await?.uuid.unwrap(),
        "locations" => locations::find_by_id(local_id).one(db).await?.uuid,
        _ => unreachable!("Add table to fk_mapper.rs")
    };

    json[format!("{}_uuid", field)] = json!(uuid);
    json.remove(fk.local_field);
    Ok(())
}
```

### map_sync_json_to_local() (reusable!)

```rust
// Called once per model during apply
pub async fn map_sync_json_to_local(mut data: Value, mappings: Vec<FKMapping>, db: &DatabaseConnection) -> Result<Value> {
    for fk in mappings {
        let uuid: Uuid = data[format!("{}_uuid", field)].as_str()?.parse()?;

        // Generic lookup based on table name
        let local_id = match fk.target_table {
            "devices" => devices::find().filter(uuid.eq(uuid)).one(db).await?.id,
            "entries" => entries::find().filter(uuid.eq(Some(uuid))).one(db).await?.id,
            "locations" => locations::find().filter(uuid.eq(uuid)).one(db).await?.id,
            _ => unreachable!("Add table to fk_mapper.rs")
        };

        data[fk.local_field] = json!(local_id);
        data.remove(&uuid_field);
    }
    Ok(data)
}
```

## Per-Model Code Required

### Location (has FKs)

**Total: 15 lines**
```rust
fn foreign_key_mappings() -> Vec<FKMapping> {  // 3 lines
    vec![
        FKMapping::new("device_id", "devices"),
        FKMapping::new("entry_id", "entries"),
    ]
}

async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {  // 12 lines
    let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
    let location: Model = serde_json::from_value(data)?;

    Entity::insert(location.into())
        .on_conflict(OnConflict::column(Column::Uuid).update_all().to_owned())
        .exec(db)
        .await?;
    Ok(())
}
```

### Tag (no FKs)

**Total: 8 lines**
```rust
async fn apply_shared_change(entry: SharedChangeEntry, db: &DatabaseConnection) -> Result<()> {
    let tag: Model = serde_json::from_value(entry.data)?;
    Entity::insert(tag.into())
        .on_conflict(OnConflict::column(Column::Uuid).update_all().to_owned())
        .exec(db)
        .await?;
    Ok(())
}
```

## Summary: Yes, It's Automatic!

**You write**: FK declarations (static data)
**Generic code does**: All the actual mapping logic

**Per model**:
- Models with FKs: ~15 lines (mostly boilerplate)
- Models without FKs: ~8 lines (just upsert)

**Shared across ALL models**:
- `convert_fk_to_uuid()` - works for any FK
- `map_sync_json_to_local()` - works for any model
- `lookup_uuid_for_local_id()` - works for any table
- `lookup_local_id_for_uuid()` - works for any table

### Adding a New FK-Heavy Model

```rust
impl Syncable for complex_model::Model {
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            FKMapping::new("parent_id", "complex_models"),
            FKMapping::new("owner_id", "devices"),
            FKMapping::new("category_id", "categories"),
        ]
    }

    async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
        let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
        let model: Model = serde_json::from_value(data)?;
        upsert(model).await?;
        Ok(())
    }
}
```

The mapping is **automatic** - you just declare what needs mapping, not how to do it!

**Want me to update the Syncable trait to include the `foreign_key_mappings()` method with a default implementation?**

