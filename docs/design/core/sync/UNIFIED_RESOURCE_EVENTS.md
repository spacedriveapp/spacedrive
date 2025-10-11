<!--CREATED: 2025-10-11-->
# Unified Resource Event System

## Problem Statement

Current event system has ~40 specialized variants (`EntryCreated`, `VolumeAdded`, `JobStarted`, etc.), leading to:
- Manual event emission scattered across codebase
- No type safety between events and resources
- Clients must handle each variant specifically
- Adding new resources requires new event variants
- TransactionManager cannot automatically emit events

**Observation from code**: Line 353 has a TODO: "events should have an envelope that contains the library_id instead of this"

## Solution: Generic Resource Events

All resources implementing `Identifiable` can use a unified event structure. TransactionManager emits these automatically.

### Design

```rust
// core/src/infra/event/mod.rs

/// Unified event envelope wrapping all resource events
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Event {
    /// Event metadata
    pub envelope: EventEnvelope,

    /// The actual event payload
    pub kind: EventKind,
}

/// Standard envelope for all events
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventEnvelope {
    /// Event ID for deduplication/tracking
    pub id: Uuid,

    /// When this event was created
    pub timestamp: DateTime<Utc>,

    /// Library context (if applicable)
    pub library_id: Option<Uuid>,

    /// Sequence number for ordering (optional)
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum EventKind {
    // ========================================
    // GENERIC RESOURCE EVENTS
    // ========================================

    /// A resource was created/updated (single)
    ResourceChanged {
        /// Resource type identifier (from Identifiable::resource_type)
        resource_type: String,

        /// The full resource data (must implement Identifiable)
        #[specta(skip)] // Clients reconstruct from JSON
        resource: serde_json::Value,
    },

    /// Multiple resources changed in a batch
    ResourceBatchChanged {
        resource_type: String,
        resources: Vec<serde_json::Value>,
        operation: BatchOperation,
    },

    /// A resource was deleted
    ResourceDeleted {
        resource_type: String,
        resource_id: Uuid,
    },

    /// Bulk operation completed (notification only, no data transfer)
    BulkOperationCompleted {
        /// Type of resource affected
        resource_type: String,

        /// Summary info
        affected_count: usize,
        operation_token: Uuid,
        hints: serde_json::Value, // location_id, etc.
    },

    // ========================================
    // LIFECYCLE EVENTS (no resources)
    // ========================================

    CoreStarted,
    CoreShutdown,

    LibraryOpened { id: Uuid, name: String },
    LibraryClosed { id: Uuid },

    // ========================================
    // INFRASTRUCTURE EVENTS
    // ========================================

    /// Job lifecycle (not a domain resource)
    Job {
        job_id: String,
        status: JobStatus,
        progress: Option<f64>,
        message: Option<String>,
    },

    /// Raw filesystem changes (before DB resolution)
    FsRawChange {
        kind: FsRawEventKind,
    },

    /// Log streaming
    LogMessage {
        timestamp: DateTime<Utc>,
        level: String,
        target: String,
        message: String,
        job_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum BatchOperation {
    Index,
    Search,
    Update,
    WatcherBatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum JobStatus {
    Queued,
    Started,
    Progress,
    Completed { output: JobOutput },
    Failed { error: String },
    Cancelled,
    Paused,
    Resumed,
}
```

### TransactionManager Integration

```rust
impl TransactionManager {
    /// Emit a resource changed event (automatic)
    fn emit_resource_changed<R: Identifiable + Serialize>(
        &self,
        library_id: Uuid,
        resource: &R,
    ) {
        let event = Event {
            envelope: EventEnvelope {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                library_id: Some(library_id),
                sequence: None,
            },
            kind: EventKind::ResourceChanged {
                resource_type: R::resource_type().to_string(),
                resource: serde_json::to_value(resource).unwrap(),
            },
        };

        self.event_bus.emit(event);
    }

    /// Commit single resource (emits ResourceChanged)
    pub async fn commit<M: Syncable + IntoActiveModel, R: Identifiable + From<M>>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<R, TxError> {
        let library_id = library.id();

        // Atomic: DB + sync log
        let saved = /* transaction logic */;

        // Build client resource
        let resource = R::from(saved);

        // Auto-emit
        self.emit_resource_changed(library_id, &resource);

        Ok(resource)
    }

    /// Commit batch (emits ResourceBatchChanged)
    pub async fn commit_batch<M, R>(
        &self,
        library: Arc<Library>,
        models: Vec<M>,
    ) -> Result<Vec<R>, TxError>
    where
        M: Syncable + IntoActiveModel,
        R: Identifiable + From<M>,
    {
        let library_id = library.id();

        // Atomic batch transaction
        let saved_models = /* batch transaction */;

        // Build resources
        let resources: Vec<R> = saved_models.into_iter().map(R::from).collect();

        // Emit batch event
        let event = Event {
            envelope: EventEnvelope {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                library_id: Some(library_id),
                sequence: None,
            },
            kind: EventKind::ResourceBatchChanged {
                resource_type: R::resource_type().to_string(),
                resources: resources.iter()
                    .map(|r| serde_json::to_value(r).unwrap())
                    .collect(),
                operation: BatchOperation::Update,
            },
        };

        self.event_bus.emit(event);

        Ok(resources)
    }

    /// Bulk operation (emits BulkOperationCompleted)
    pub async fn commit_bulk<M: Syncable>(
        &self,
        library: Arc<Library>,
        changes: ChangeSet<M>,
    ) -> Result<BulkAck, TxError> {
        let library_id = library.id();

        // Atomic bulk insert + metadata sync log
        let token = /* bulk transaction */;

        // Emit summary event (no resource data!)
        let event = Event {
            envelope: EventEnvelope {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                library_id: Some(library_id),
                sequence: None,
            },
            kind: EventKind::BulkOperationCompleted {
                resource_type: M::SYNC_MODEL.to_string(),
                affected_count: changes.items.len(),
                operation_token: token,
                hints: changes.hints,
            },
        };

        self.event_bus.emit(event);

        Ok(BulkAck { affected: changes.items.len(), token })
    }
}
```

### Client Handling (Swift Example)

```swift
// ZERO-FRICTION: Type registry (auto-generated from Rust via specta)
protocol CacheableResource: Identifiable, Codable {
    static var resourceType: String { get }
}

// Auto-generated registry (no manual maintenance!)
class ResourceTypeRegistry {
    private static var decoders: [String: (Data) throws -> any CacheableResource] = [:]

    // Called automatically when types are loaded
    static func register<T: CacheableResource>(_ type: T.Type) {
        decoders[T.resourceType] = { data in
            try JSONDecoder().decode(T.self, from: data)
        }
    }

    static func decode(resourceType: String, from data: Data) throws -> any CacheableResource {
        guard let decoder = decoders[resourceType] else {
            throw CacheError.unknownResourceType(resourceType)
        }
        return try decoder(data)
    }
}

// Types auto-register via property wrapper or extension
extension File: CacheableResource {
    static let resourceType = "file"
}

extension Album: CacheableResource {
    static let resourceType = "album"
}

extension Tag: CacheableResource {
    static let resourceType = "tag"
}

// Add new resources without touching ANY event handling code!
extension Location: CacheableResource {
    static let resourceType = "location"
}

// GENERIC event handler (ZERO switch statements!)
actor ResourceCache {
    func handleEvent(_ event: Event) async {
        switch event.kind {
        case .ResourceChanged(let resourceType, let resourceJSON):
            do {
                // Generic decode - works for ALL resources!
                let resource = try ResourceTypeRegistry.decode(
                    resourceType: resourceType,
                    from: resourceJSON
                )
                updateEntity(resource)
            } catch {
                print("Failed to decode \(resourceType): \(error)")
            }

        case .ResourceBatchChanged(let resourceType, let resourcesJSON, let operation):
            // Generic batch decode
            let resources = resourcesJSON.compactMap { json in
                try? ResourceTypeRegistry.decode(resourceType: resourceType, from: json)
            }
            resources.forEach { updateEntity($0) }

        case .BulkOperationCompleted(let resourceType, let count, let token, let hints):
            // Invalidate queries
            print("Bulk op on \(resourceType): \(count) items")
            invalidateQueriesForResource(resourceType, hints: hints)

        case .ResourceDeleted(let resourceType, let resourceId):
            // Generic deletion
            deleteEntity(resourceType: resourceType, id: resourceId)

        // Infrastructure events
        case .Job(let jobId, let status, _, _):
            updateJobStatus(jobId: jobId, status: status)

        default:
            break
        }
    }

    // Generic entity update (works for all Identifiable resources)
    func updateEntity(_ resource: any CacheableResource) {
        let cacheKey = type(of: resource).resourceType + ":" + resource.id.uuidString
        entityStore[cacheKey] = resource

        // Update all queries that reference this resource
        invalidateQueriesContaining(cacheKey)
    }

    // Generic deletion
    func deleteEntity(resourceType: String, id: UUID) {
        let cacheKey = resourceType + ":" + id.uuidString
        entityStore.removeValue(forKey: cacheKey)
        invalidateQueriesContaining(cacheKey)
    }
}
```

**Key Innovation**: Type registry eliminates all switch statements!

**Adding a new resource**:
```swift
// 1. Define type (auto-generated from Rust via specta)
struct Photo: CacheableResource {
    let id: UUID
    let albumId: UUID
    let path: String
    static let resourceType = "photo"
}

// 2. That's it! Event handling automatically works.
// No changes to ResourceCache, no switch cases, nothing!
```
```

### TypeScript Client Example

```typescript
// ZERO-FRICTION: Type registry (auto-generated from Rust via specta)
interface CacheableResource {
  id: string;
}

// Auto-generated type map (from Rust types via specta)
type ResourceTypeMap = {
  file: File;
  album: Album;
  tag: Tag;
  location: Location;
  // New types added automatically by codegen!
};

// Generic decoder with type safety
class ResourceTypeRegistry {
  private static validators: Map<string, (data: unknown) => CacheableResource> = new Map();

  // Auto-register types (called during module init)
  static register<T extends CacheableResource>(
    resourceType: string,
    validator: (data: unknown) => T
  ) {
    this.validators.set(resourceType, validator);
  }

  static decode(resourceType: string, data: unknown): CacheableResource {
    const validator = this.validators.get(resourceType);
    if (!validator) {
      throw new Error(`Unknown resource type: ${resourceType}`);
    }
    return validator(data);
  }
}

// Types auto-register (could use decorators or explicit calls)
ResourceTypeRegistry.register('file', (data) => data as File);
ResourceTypeRegistry.register('album', (data) => data as Album);
ResourceTypeRegistry.register('tag', (data) => data as Tag);
// Add new types without touching event handler!

// GENERIC event handler (ZERO switch statements!)
export class NormalizedCache {
  handleEvent(event: Event) {
    switch (event.kind.type) {
      case 'ResourceChanged': {
        const { resource_type, resource } = event.kind.data;
        // Generic decode - works for ALL resources!
        const decoded = ResourceTypeRegistry.decode(resource_type, resource);
        this.updateEntity(resource_type, decoded);
        break;
      }

      case 'ResourceBatchChanged': {
        const { resource_type, resources } = event.kind.data;
        // Generic batch
        resources.forEach(r => {
          const decoded = ResourceTypeRegistry.decode(resource_type, r);
          this.updateEntity(resource_type, decoded);
        });
        break;
      }

      case 'BulkOperationCompleted': {
        const { resource_type, hints } = event.kind.data;
        this.invalidateQueries(resource_type, hints);
        break;
      }

      case 'ResourceDeleted': {
        const { resource_type, resource_id } = event.kind.data;
        this.deleteEntity(resource_type, resource_id);
        break;
      }
    }
  }

  // Automatic cache update for ANY resource
  private updateEntity(resourceType: string, resource: CacheableResource) {
    const cacheKey = `${resourceType}:${resource.id}`;
    this.entities.set(cacheKey, resource);

    // Trigger UI updates for queries using this resource
    this.notifyQueries(cacheKey);
  }

  // Generic deletion
  private deleteEntity(resourceType: string, resourceId: string) {
    const cacheKey = `${resourceType}:${resourceId}`;
    this.entities.delete(cacheKey);
    this.notifyQueries(cacheKey);
  }
}

// Adding a new resource (Photo):
// 1. Rust: impl Identifiable for Photo { resource_type() = "photo" }
// 2. Run: cargo run --bin specta-gen (regenerates TypeScript types)
// 3. TypeScript: import { Photo } from './bindings/Photo.ts'
// 4. ResourceTypeRegistry.register('photo', (data) => data as Photo);
// 5. Done! No changes to event handling, cache logic, nothing!
```

**With Build Script Automation** (fully automatic):
```typescript
// Auto-generated file: src/bindings/resourceRegistry.ts
// This file is generated by: cargo run --bin specta-gen
// DO NOT EDIT MANUALLY

import { File } from './File';
import { Album } from './Album';
import { Tag } from './Tag';
import { Location } from './Location';
// ... all other Identifiable types

// Registry is populated at module load time
export const resourceTypeMap = {
  'file': File,
  'album': Album,
  'tag': Tag,
  'location': Location,
  // ... all other types
} as const;

// Zero-config setup
Object.entries(resourceTypeMap).forEach(([type, validator]) => {
  ResourceTypeRegistry.register(type, validator as any);
});
```

**Result**: Adding a new Identifiable resource in Rust automatically:
1. Generates TypeScript type
2. Registers in type map
3. Works with event handling
4. **Zero manual client changes!**

## Migration Strategy

### Phase 1: Add Unified Events (Additive)
- Keep existing Event variants
- Add new `ResourceChanged`, `ResourceBatchChanged`, etc.
- TransactionManager emits new events
- Clients can start consuming new events

### Phase 2: Migrate Resources One-by-One
For each resource (File, Album, Tag, Location, etc.):
1. Implement `Identifiable` trait
2. Switch from manual `event_bus.emit(Event::EntryCreated)` to TM
3. Update client to consume `ResourceChanged` for that type
4. Mark old event variant as deprecated

### Phase 3: Remove Old Events
Once all resources migrated:
- Remove `EntryCreated`, `VolumeAdded`, etc.
- Keep infrastructure events (Job, Log, FsRawChange)
- Remove manual event emission from ops code

## Benefits

### For Rust Core
**Zero boilerplate**: No manual event emission
**Type safety**: TM ensures events match resources
**Automatic**: Emit on every commit
**Uniform**: All resources handled same way

### For Clients
**ZERO switch statements**: Type registry handles all resources
**Type-safe deserialization**: JSON → typed resource
**Zero-friction scaling**: Add 100 resources, no client changes
**Auto-generated**: specta codegen creates registry automatically
**Cache-friendly**: Direct integration with normalized cache

### Horizontal Scaling
**Rust**: Add `impl Identifiable` → automatic events
**TypeScript**: Run codegen → automatic type + registry
**Swift**: Add `CacheableResource` conformance → automatic handling
**New platforms**: Implement type registry once, scales infinitely

### For Maintenance
**Less code**: ~40 variants → ~5 generic variants
**No manual updates**: Adding File → Album → Tag reuses same code
**Clear semantics**: Resource events vs infrastructure events
**Centralized**: All emission in TransactionManager

## Examples by Resource Type

### Files (Entry → File)
```rust
// Rust
let file = tm.commit::<entry::Model, File>(library, entry_model).await?;
// → Emits: ResourceChanged { resource_type: "file", resource: file }

// Swift
case .ResourceChanged("file", let json):
    let file = try decode(File.self, json)
    cache.updateEntity(file)
```

### Albums
```rust
// Rust
let album = tm.commit::<albums::Model, Album>(library, album_model).await?;
// → Emits: ResourceChanged { resource_type: "album", resource: album }

// Swift
case .ResourceChanged("album", let json):
    let album = try decode(Album.self, json)
    cache.updateEntity(album)
```

### Tags
```rust
// Rust
let tag = tm.commit::<tags::Model, Tag>(library, tag_model).await?;
// → Emits: ResourceChanged { resource_type: "tag", resource: tag }

// Swift
case .ResourceChanged("tag", let json):
    let tag = try decode(Tag.self, json)
    cache.updateEntity(tag)
```

### Locations
```rust
// Rust
let location = tm.commit::<locations::Model, Location>(library, location_model).await?;
// → Emits: ResourceChanged { resource_type: "location", resource: location }

// Swift
case .ResourceChanged("location", let json):
    let location = try decode(Location.self, json)
    cache.updateEntity(location)
```

## Infrastructure Events (Not Resources)

Some events are not domain resources:
- **Jobs**: Ephemeral, not cached, different lifecycle
- **Logs**: Streaming, not state
- **FsRawChange**: Pre-database, becomes Entry later
- **Core lifecycle**: System-level

These keep specialized variants under `EventKind`.

## Comparison: Before vs After

### Before (Current)
```rust
// Scattered manual emission
pub async fn create_album(library: Arc<Library>, name: String) -> Result<Album> {
    let model = albums::ActiveModel { /* ... */ };
    let saved = model.insert(db).await?;

    // Manual event emission
    event_bus.emit(Event::AlbumCreated {
        library_id: library.id(),
        album_id: saved.uuid,
    });

    Ok(album)
}

// Client must handle specific variant + switch case
case .AlbumCreated(let libraryId, let albumId):
    // Fetch album data separately
    let album = await client.query("albums.get", albumId)
    cache.updateEntity(album)
```

### After (Unified + Type Registry)
```rust
// Automatic emission via TransactionManager
pub async fn create_album(
    tm: &TransactionManager,
    library: Arc<Library>,
    name: String,
) -> Result<Album> {
    let model = albums::ActiveModel { /* ... */ };

    // TM emits ResourceChanged automatically
    let album = tm.commit::<albums::Model, Album>(library, model).await?;

    Ok(album)
}

// Client: ZERO resource-specific code!
case .ResourceChanged(let resourceType, let json):
    // Works for Album, File, Tag, Location, everything!
    let resource = try ResourceTypeRegistry.decode(resourceType, json)
    cache.updateEntity(resource)
    // Add 100 new resources: this code never changes!
```

**Adding a 101st resource**:
- Rust: `impl Identifiable for NewResource` (3 lines)
- Client: Nothing! (codegen handles it)

**Horizontal scaling achieved!** 

## Event Size Considerations

**Concern**: Sending full resources in events increases bandwidth

**Mitigations**:
1. **Gzip compression**: Event bus can compress large payloads
2. **Client caching**: Only send if resource changed
3. **Delta events** (future): Send only changed fields
4. **Bulk events**: Don't send individual resources (just metadata)

**Measurement**:
- File resource: ~500 bytes JSON
- Album resource: ~200 bytes JSON
- Tag resource: ~150 bytes JSON

Even with 100 concurrent updates: 500 bytes × 100 = 50KB (negligible)

## Alternative: Lightweight Events

If bandwidth becomes an issue, use two-tier system:

```rust
pub enum EventKind {
    // Lightweight: just ID
    ResourceChanged {
        resource_type: String,
        resource_id: Uuid,
        // Client fetches if needed
    },

    // Rich: full data (opt-in)
    ResourceChangedRich {
        resource_type: String,
        resource: serde_json::Value,
    },
}
```

But start with rich events (simpler, better cache consistency).

## Conclusion

This unified event system:
- Eliminates ~35 specialized event variants
- Makes TransactionManager sole event emitter
- Enables generic client handling
- Reduces boilerplate to zero
- Scales to infinite resource types
- Aligns perfectly with Identifiable/Syncable design

**Next Step**: Implement `Event` refactor alongside TransactionManager in mini-spec.
