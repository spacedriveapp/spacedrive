<!--CREATED: 2025-10-11-->
# Sync + Transaction Manager + Normalized Cache: Mini Spec

## Scope
A concise specification aligning TransactionManager, Syncable/Identifiable traits, bulk change handling, raw query compatibility, and leader election. Includes a concrete Albums example with minimal boilerplate.

## Goals
- Zero manual sync-log creation in application code
- Keep raw SQL for complex reads; writes go through TransactionManager
- Bulk = mechanism (generic changeset), not hard-coded enum cases
- Clear trait-based configuration, minimal boilerplate
- Compatible with existing SeaORM patterns

## Core Traits

```rust
// client-facing identity for cache normalization
pub trait Identifiable {
    type Id: Into<Uuid> + Copy + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>;
    fn id(&self) -> Self::Id;
    fn resource_type() -> &'static str;
}

// persistence-facing for sync logging
pub trait Syncable {
    // stable model type name used in sync log
    const SYNC_MODEL: &'static str;

    // globally unique logical id for sync (Uuid recommended)
    fn sync_id(&self) -> Uuid;

    // optimistic concurrency
    fn version(&self) -> i64;

    // minimal payload for replication (defaults to full serde)
    fn to_sync_json(&self) -> serde_json::Value where Self: Serialize {
        serde_json::to_value(self).unwrap_or(serde_json::json!({}))
    }

    // optional field allow/deny (minimize boilerplate: both optional)
    fn include_fields() -> Option<&'static [&'static str]> { None }
    fn exclude_fields() -> Option<&'static [&'static str]> { None }
}
```

Notes:
- App code should not construct sync logs; TransactionManager derives them from `Syncable`.
- `include_fields`/`exclude_fields` are optional knobs. If both None, default to `to_sync_json()`.

## TransactionManager Responsibilities

- Enforce atomic DB write + sync log creation
- Emit rich events post-commit for client cache
- Support single, batch, and bulk change sets
- Provide a transaction-bound context for raw SQL when needed

### API (sketch)
```rust
pub struct TransactionManager { /* event bus, seq allocator, leader state */ }

pub struct ChangeSet<T> { pub items: Vec<T> } // generic mechanism for bulk

impl TransactionManager {
    // single model
    pub async fn commit<M: Syncable + IntoActiveModel>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<M, TxError>;

    // micro-batch (10–1k), produces per-item sync entries
    pub async fn commit_batch<M: Syncable + IntoActiveModel>(
        &self,
        library: Arc<Library>,
        models: Vec<M>,
    ) -> Result<Vec<M>, TxError>;

    // bulk (1k+), produces ONE metadata sync entry with ChangeSet descriptor
    pub async fn commit_bulk<M: Syncable + IntoActiveModel>(
        &self,
        library: Arc<Library>,
        changes: ChangeSet<M>,
    ) -> Result<BulkAck, TxError>;
}

pub struct BulkAck { pub affected: usize, pub token: Uuid }
```

### Sync Log Semantics
- commit: one sync entry per item
- commit_batch: one per item (same txn), event may be batched
- commit_bulk: ONE metadata sync entry:
```json
{
  "sequence": 1234,
  "model_type": "bulk_changeset",
  "token": "uuid-token",
  "affected": 1000000,
  "model": "entry",           // derived from Syncable::SYNC_MODEL
  "mode": "insert|update|delete",
  "hints": { "location_id": "..." }
}
```
Followers treat this as a notification; they DO NOT pull all items. They trigger local indexing where applicable.

## Raw Query Compatibility
- Reads: unrestricted (SeaORM query builder or raw SQL)
- Writes: perform inside TM-provided transaction handle
  - TM exposes `with_tx(|txn| async { /* raw SQL writes */ })` that auto sync-logs via `Syncable` wrappers or explicit `commit_*` calls.

## Leader Election (Minimum)
- Single leader per library for assigning sync sequences
- Election strategy per SYNC_DESIGN.md (initial leader = creator; re-elect via heartbeat timeout)
- TM refuses sync-log creation if not leader (or buffers and requests lease)

## Albums Example (Concrete)

Schema (SeaORM model):
```rust
#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "albums")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub name: String,
    pub cover_entry_uuid: Option<Uuid>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Implement traits:
```rust
impl Syncable for albums::Model {
    const SYNC_MODEL: &'static str = "album";
    fn sync_id(&self) -> Uuid { self.uuid }
    fn version(&self) -> i64 { self.version }
    fn exclude_fields() -> Option<&'static [&'static str]> {
        // example: exclude timestamps from replication
        Some(&["created_at", "updated_at", "id"])
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album { pub id: Uuid, pub name: String, pub cover: Option<Uuid> }

impl Identifiable for Album {
    type Id = Uuid;
    fn id(&self) -> Self::Id { self.id }
    fn resource_type() -> &'static str { "album" }
}
```

Create action (no manual sync logging):
```rust
pub async fn create_album(
    tm: &TransactionManager,
    library: Arc<Library>,
    name: String,
) -> Result<Album, TxError> {
    let model = albums::Model {
        id: 0,
        uuid: Uuid::new_v4(),
        name,
        cover_entry_uuid: None,
        version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // TM writes + sync logs atomically
    let saved = tm.commit(library.clone(), model).await?;

    // Build client model (query layer)
    let album = Album { id: saved.uuid, name: saved.name, cover: saved.cover_entry_uuid };

    // TM (post-commit) emits Event::AlbumUpdated { album } automatically
    Ok(album)
}
```

Bulk import albums:
```rust
pub async fn import_albums(
    tm: &TransactionManager,
    library: Arc<Library>,
    names: Vec<String>,
) -> Result<usize, TxError> {
    let models: Vec<albums::Model> = names.into_iter().map(|n| albums::Model {
        id: 0,
        uuid: Uuid::new_v4(),
        name: n,
        cover_entry_uuid: None,
        version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }).collect();

    let ack = tm.commit_bulk(library, ChangeSet { items: models }).await?;
    Ok(ack.affected)
}
```

## Boilerplate Minimization
- Derive macros can implement `Syncable` and `Identifiable` from annotations:
```rust
#[derive(Syncable)]
#[syncable(model="album", id="uuid", version="version", exclude=["created_at","updated_at","id"])]
struct albums::Model { /* ... */ }

#[derive(Identifiable)]
#[identifiable(resource="album", id="id")]
struct Album { /* ... */ }
```

## Event Emission (Unified System)

See `UNIFIED_RESOURCE_EVENTS.md` for complete design.

**Key Points**:
- TM emits generic `ResourceChanged` events automatically
- No manual `event_bus.emit()` in application code
- Clients handle resources generically via `resource_type` field
- Event structure:
  ```rust
  Event {
    envelope: { id, timestamp, library_id, sequence },
    kind: ResourceChanged { resource_type, resource }
         | ResourceBatchChanged { resource_type, resources, operation }
         | BulkOperationCompleted { resource_type, affected_count, token, hints }
         | ResourceDeleted { resource_type, resource_id }
  }
  ```

**Example**:
```rust
// Rust: Automatic emission
let album = tm.commit::<albums::Model, Album>(library, model).await?;
// → Emits: ResourceChanged { resource_type: "album", resource: album }

// Swift: Generic handling
case .ResourceChanged(let type, let json):
    switch type {
    case "album": cache.updateEntity(try decode(Album.self, json))
    case "file": cache.updateEntity(try decode(File.self, json))
    // Add new resources without changing event code!
    }
```

Benefits:
- Zero boilerplate for new resources
- Type-safe on both ends
- Cache integration automatic
- ~35 specialized event variants eliminated

## Consistency Rules
- All sync-worthy writes go through TM
- Reads, including raw SQL, remain unrestricted
- Followers treat bulk metadata as notification; they re-index locally if applicable

## Appendix: Raw SQL inside TM
```rust
tm.with_tx(library, |txn| async move {
    // raw SQL writes
    txn.execute(Statement::from_sql_and_values(DbBackend::Sqlite, "UPDATE albums SET name=? WHERE uuid=?", vec![name.into(), uuid.into()])).await?;
    // tell TM to record sync for this model change
    tm.sync_log_for::<albums::Model>(txn, uuid).await?;
    Ok(())
}).await?;
```
