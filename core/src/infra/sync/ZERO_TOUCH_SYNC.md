# Zero-Touch Sync Architecture ✨

**The Problem We Solved**: Adding sync support to a model shouldn't require modifying core sync infrastructure files.

---

## ❌ Before (Central Applier Anti-Pattern)

```rust
// applier.rs - MUST be modified for every new model
match entry.model_type.as_str() {
    "location" => apply_location(...),
    "tag" => apply_tag(...),           // Add this line
    "album" => apply_album(...),       // Add this line
    "collection" => apply_collection(...), // Add this line
    // Every new model = modify this file!
}
```

**Problems**:
- Central bottleneck
- Breaks encapsulation
- Merge conflicts
- Not DDD-aligned

---

## ✅ After (Registry + Trait Pattern)

### Step 1: Implement Syncable on Your Model

```rust
// core/src/infra/db/entities/location.rs

impl Syncable for location::Model {
    const SYNC_MODEL: &'static str = "location";

    fn sync_id(&self) -> Uuid { self.uuid }
    fn version(&self) -> i64 { self.version }
    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "scan_state", "error_message"])
    }

    // Implement how to apply sync entries
    async fn apply_sync_entry(
        entry: &SyncLogEntry,
        db: &DatabaseConnection,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match entry.change_type {
            ChangeType::Insert => {
                let data: Self = serde_json::from_value(entry.data)?;
                // Insert logic
            }
            ChangeType::Update => {
                // Update logic with version checking
            }
            ChangeType::Delete => {
                // Delete logic
            }
        }
        Ok(())
    }
}
```

### Step 2: Register the Model (ONE LINE!)

```rust
// At the bottom of location.rs
crate::register_syncable_model!(location::Model);
```

### Step 3: Done! 🎉

The applier automatically picks up your model via the registry:

```rust
// applier.rs - NEVER needs modification!
pub async fn apply_entry(&self, entry: &SyncLogEntry) -> Result<()> {
    // Registry looks up model_type and calls its apply_sync_entry
    crate::infra::sync::registry::apply_sync_entry(entry, self.db.conn()).await
}
```

---

## How It Works (Registry Pattern)

Uses the `inventory` crate (same as actions/queries):

```rust
// 1. Macro expands to:
inventory::submit! {
    SyncableModelRegistration {
        model_type: "location",
        apply_fn: |entry, db| {
            Box::pin(async move {
                location::Model::apply_sync_entry(entry, db).await
            })
        },
    }
}

// 2. At runtime, registry collects all registrations:
static REGISTRY: OnceLock<HashMap<&str, ApplyFn>> = OnceLock::new();

// 3. Applier looks up by model_type string:
let apply_fn = registry.get("location")?;
apply_fn(entry, db).await  // Calls location::Model::apply_sync_entry
```

---

## Adding a New Syncable Model

**Example: Add Tag sync support**

```rust
// core/src/infra/db/entities/tag.rs

impl Syncable for tag::Model {
    const SYNC_MODEL: &'static str = "tag";

    fn sync_id(&self) -> Uuid { self.uuid }
    fn version(&self) -> i64 { self.version }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "created_at", "updated_at"])
    }

    async fn apply_sync_entry(
        entry: &SyncLogEntry,
        db: &DatabaseConnection,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Tag-specific insert/update/delete logic
        Ok(())
    }
}

// Register it
crate::register_syncable_model!(tag::Model);
```

**That's it!** No other files need modification:
- ❌ Don't touch `applier.rs`
- ❌ Don't touch sync service
- ❌ Don't touch protocol handler
- ✅ Just implement trait + one line macro

---

## Benefits

### 1. **True Decoupling**
Each model is completely self-contained for sync:
- Knows what to sync (`to_sync_json()`)
- Knows what to exclude (`exclude_fields()`)
- Knows how to apply (`apply_sync_entry()`)

### 2. **Zero Core Modifications**
Adding a new syncable model:
- ✅ Implement trait in model file
- ✅ Add one line: `register_syncable_model!(MyModel);`
- ❌ NO modifications to sync infrastructure

### 3. **Compile-Time Safety**
- Registry built at compile-time via `inventory`
- Type-safe dispatch (no string typos)
- Missing registration = runtime warning (not panic)

### 4. **DDD/CQRS Aligned**
- Models own their domain logic
- Sync is part of the model's responsibility
- Infrastructure is just routing

### 5. **Same Pattern as Actions/Queries**
Spacedrive already uses this for:
- `register_query!(MyQuery, "path")`
- `register_library_action!(MyAction, "path")`
- `register_syncable_model!(MyModel)` ← Now for sync!

---

## Complete Example: Tag Sync in 50 Lines

```rust
// core/src/infra/db/entities/tag.rs

impl Syncable for tag::Model {
    const SYNC_MODEL: &'static str = "tag";

    fn sync_id(&self) -> Uuid { self.uuid }
    fn version(&self) -> i64 { 1 } // TODO: Add version field

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id", "created_at", "updated_at", "created_by_device"])
    }

    async fn apply_sync_entry(
        entry: &SyncLogEntry,
        db: &DatabaseConnection,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        use sea_orm::ActiveValue;

        match entry.change_type {
            ChangeType::Insert => {
                let data: Self = serde_json::from_value(entry.data.clone())?;

                // Check existence
                if Entity::find()
                    .filter(Column::Uuid.eq(entry.record_id))
                    .one(db)
                    .await?
                    .is_some()
                {
                    return Ok(());
                }

                // Insert
                let model = ActiveModel {
                    id: ActiveValue::NotSet,
                    uuid: ActiveValue::Set(data.uuid),
                    canonical_name: ActiveValue::Set(data.canonical_name),
                    // ... other fields
                };
                model.insert(db).await?;
            }
            ChangeType::Update => { /* update logic */ }
            ChangeType::Delete => { /* delete logic */ }
            _ => {}
        }
        Ok(())
    }
}

// ONE LINE - registers automatically!
crate::register_syncable_model!(tag::Model);
```

**Total changes**: 1 file, ~50 lines. No core sync code touched!

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ Follower Device Receives Sync Entry                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  SyncLogEntry { model_type: "location", ... }              │
│         ↓                                                   │
│  SyncApplier::apply_entry()                                │
│         ↓                                                   │
│  registry::apply_sync_entry()  ← Lookup by model_type     │
│         ↓                                                   │
│  Registry: {"location" → location::Model::apply_sync_entry} │
│         ↓                                                   │
│  location::Model::apply_sync_entry(entry, db)              │
│         ↓                                                   │
│  Location-specific insert/update/delete logic              │
│         ↓                                                   │
│  ✅ Database updated                                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Key**: The registry is populated at compile-time by the `register_syncable_model!` macro.

---

## Comparison to Spacedrive's Existing Patterns

| Pattern | Registration | Dispatch |
|---------|-------------|----------|
| **Actions** | `register_library_action!(FileCopyAction, "files.copy")` | Registry by method string |
| **Queries** | `register_query!(NetworkStatusQuery, "network.status")` | Registry by method string |
| **Syncable** | `register_syncable_model!(location::Model)` | **Registry by model_type** ⭐ |

**Consistent architecture across the codebase!**

---

## Testing the Registry

```rust
#[test]
fn test_all_models_registered() {
    let registry = get_registry();

    // Verify expected models are registered
    assert!(registry.contains_key("location"));
    assert!(registry.contains_key("tag"));
    assert!(registry.contains_key("collection"));

    println!("Registered models: {:?}", registry.keys());
}
```

---

## Migration Guide (Adding Sync to Existing Models)

For each model you want to sync:

1. **Add version field** (via migration):
   ```sql
   ALTER TABLE your_table ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
   ```

2. **Implement Syncable**:
   ```rust
   impl Syncable for your_model::Model {
       const SYNC_MODEL: &'static str = "your_model";
       // ... methods ...
       async fn apply_sync_entry(...) { ... }
   }
   ```

3. **Register**:
   ```rust
   crate::register_syncable_model!(your_model::Model);
   ```

4. **Done!** Sync automatically works.

---

**This architecture scales to hundreds of models without ever touching sync infrastructure!** 🚀

