# Normalized Client Cache

**Status**: Implementation Ready
**Version**: 2.0
**Last Updated**: 2025-10-08

## Overview

The Normalized Client Cache is a client-side entity store that provides instant UI updates, offline support, and massive bandwidth savings. Inspired by Apollo Client, it normalizes all resources by unique ID and updates atomically when events arrive.

## The Problem

**Traditional approach**:
```swift
// Query returns files
let files = try await client.query("files.search", input: searchParams)

// User renames file on Device B
// ...

// UI doesn't update! Must manually refetch:
let files = try await client.query("files.search", input: searchParams) // Network call
```

**Issues**:
- Stale data in UI
- Manual refetch required (slow, bandwidth-heavy)
- No offline support
- Duplicate data (same file in multiple queries)

## The Solution

**Normalized cache** + **event-driven updates**:
```swift
// Query uses cache
let files = cache.query("files.search", input: searchParams) // Instant!

// Device B renames file → Event arrives
// Event: ResourceChanged { resource_type: "file", resource: File { id, name: "new.jpg" } }

// Cache updates automatically
cache.updateEntity(file)

// UI updates instantly (ObservableObject/StateFlow)
// No refetch, no network, no user action!
```

## Cache Architecture

### Two-Level Structure

```
┌─────────────────────────────────────────────────────────────┐
│ LEVEL 1: Entity Store (normalized by ID)                   │
│                                                             │
│  "file:uuid-1"    → File { id: uuid-1, name: "photo.jpg" } │
│  "file:uuid-2"    → File { id: uuid-2, name: "doc.pdf" }   │
│  "album:uuid-3"   → Album { id: uuid-3, name: "Vacation" } │
│  "tag:uuid-4"     → Tag { id: uuid-4, name: "Important" }  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              ↑
                              │ Atomic updates
                              │
┌─────────────────────────────────────────────────────────────┐
│ LEVEL 2: Query Index (maps queries to entity IDs)          │
│                                                             │
│  "search:photos"           → ["file:uuid-1", "file:uuid-2"] │
│  "directory:/vacation"     → ["file:uuid-1"]                │
│  "albums.list"             → ["album:uuid-3"]               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Key Insight**: When `file:uuid-1` updates, we find all queries referencing it and trigger UI updates for those views.

### Swift Implementation

```swift
/// Normalized entity cache with event-driven updates
actor NormalizedCache {
    // LEVEL 1: Entity store
    private var entityStore: [String: any Identifiable] = [:]

    // LEVEL 2: Query index
    private var queryIndex: [String: QueryCacheEntry] = [:]

    // Observers for reactive UI updates
    private var queryObservers: [String: Set<UUID>] = [:]

    /// Update a single entity (called by event handler)
    func updateEntity<T: Identifiable>(_ resource: T) {
        let cacheKey = "\(T.resourceType):\(resource.id.uuidString)"

        // 1. Update entity store
        entityStore[cacheKey] = resource

        // 2. Find all queries containing this entity
        let affectedQueries = queryIndex.filter { _, entry in
            entry.entityKeys.contains(cacheKey)
        }

        // 3. Notify observers (SwiftUI views re-render)
        for (queryKey, _) in affectedQueries {
            notifyObservers(for: queryKey)
        }
    }

    /// Execute a query (with caching)
    func query<T: Identifiable>(
        _ method: String,
        input: Encodable
    ) async throws -> [T] {
        let queryKey = generateQueryKey(method, input)

        // Check cache
        if let cached = queryIndex[queryKey], !cached.isExpired {
            // Cache hit! Return from entity store
            return cached.entityKeys.compactMap { key in
                entityStore[key] as? T
            }
        }

        // Cache miss - fetch from server
        let results: [T] = try await client.query(method, input: input)

        // Store entities
        for resource in results {
            let cacheKey = "\(T.resourceType):\(resource.id.uuidString)"
            entityStore[cacheKey] = resource
        }

        // Store query index
        let entityKeys = results.map { "\(T.resourceType):\($0.id.uuidString)" }
        queryIndex[queryKey] = QueryCacheEntry(
            entityKeys: Set(entityKeys),
            fetchedAt: Date(),
            ttl: 300 // 5 minutes
        )

        return results
    }

    /// Delete entity (called by event handler)
    func deleteEntity(resourceType: String, id: UUID) {
        let cacheKey = "\(resourceType):\(id.uuidString)"

        // Remove from store
        entityStore.removeValue(forKey: cacheKey)

        // Remove from query indices
        for (queryKey, var entry) in queryIndex {
            if entry.entityKeys.remove(cacheKey) != nil {
                queryIndex[queryKey] = entry
                notifyObservers(for: queryKey)
            }
        }
    }

    /// Invalidate queries (called by bulk operation events)
    func invalidateQueriesForResource(_ resourceType: String, hints: [String: Any]) {
        // Invalidate all queries matching hints (e.g., location_id)
        let keysToInvalidate = queryIndex.keys.filter { queryKey in
            if let locationId = hints["location_id"] as? String {
                return queryKey.contains(locationId)
            }
            return queryKey.contains(resourceType)
        }

        for key in keysToInvalidate {
            queryIndex.removeValue(forKey: key)
            notifyObservers(for: key)
        }
    }
}

struct QueryCacheEntry {
    var entityKeys: Set<String>     // References to entity store
    let fetchedAt: Date
    let ttl: TimeInterval           // Time to live

    var isExpired: Bool {
        Date().timeIntervalSince(fetchedAt) > ttl
    }
}
```

### TypeScript Implementation

```typescript
/**
 * Normalized entity cache with reactive updates
 */
export class NormalizedCache {
  // LEVEL 1: Entity store
  private entityStore = new Map<string, any>();

  // LEVEL 2: Query index
  private queryIndex = new Map<string, QueryCacheEntry>();

  // Reactive subscriptions (for React hooks)
  private querySubscriptions = new Map<string, Set<() => void>>();

  /**
   * Update entity (called by event handler)
   */
  updateEntity(resourceType: string, resource: any) {
    const cacheKey = `${resourceType}:${resource.id}`;

    // 1. Update entity
    this.entityStore.set(cacheKey, resource);

    // 2. Find affected queries
    for (const [queryKey, entry] of this.queryIndex.entries()) {
      if (entry.entityKeys.has(cacheKey)) {
        this.notifySubscribers(queryKey);
      }
    }
  }

  /**
   * Query with caching
   */
  async query<T>(method: string, input: any): Promise<T[]> {
    const queryKey = this.generateQueryKey(method, input);

    // Check cache
    const cached = this.queryIndex.get(queryKey);
    if (cached && !cached.isExpired()) {
      // Cache hit!
      return Array.from(cached.entityKeys)
        .map(key => this.entityStore.get(key))
        .filter(Boolean) as T[];
    }

    // Cache miss - fetch
    const results: T[] = await this.client.query(method, input);

    // Store entities
    const entityKeys = new Set<string>();
    for (const resource of results) {
      const cacheKey = `${(resource as any).__resourceType}:${(resource as any).id}`;
      this.entityStore.set(cacheKey, resource);
      entityKeys.add(cacheKey);
    }

    // Store query
    this.queryIndex.set(queryKey, {
      entityKeys,
      fetchedAt: Date.now(),
      ttl: 300000, // 5 minutes
    });

    return results;
  }

  /**
   * Subscribe to query changes (for React hooks)
   */
  subscribe(queryKey: string, callback: () => void): () => void {
    if (!this.querySubscriptions.has(queryKey)) {
      this.querySubscriptions.set(queryKey, new Set());
    }
    this.querySubscriptions.get(queryKey)!.add(callback);

    // Return unsubscribe function
    return () => {
      this.querySubscriptions.get(queryKey)?.delete(callback);
    };
  }

  private notifySubscribers(queryKey: string) {
    const subscribers = this.querySubscriptions.get(queryKey);
    if (subscribers) {
      subscribers.forEach(callback => callback());
    }
  }
}
```

## React Integration

### useCachedQuery Hook

```typescript
/**
 * React hook for cached queries with automatic updates
 */
export function useCachedQuery<T>(
  method: string,
  input: any,
  options?: { enabled?: boolean }
): { data: T[] | null; loading: boolean; error: Error | null } {
  const cache = useContext(CacheContext);
  const [data, setData] = useState<T[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    if (options?.enabled === false) return;

    const queryKey = cache.generateQueryKey(method, input);

    // Subscribe to cache changes
    const unsubscribe = cache.subscribe(queryKey, () => {
      // Query result changed - re-read from cache
      const result = cache.getQueryResult<T>(queryKey);
      setData(result);
    });

    // Initial fetch
    (async () => {
      try {
        const result = await cache.query<T>(method, input);
        setData(result);
      } catch (e) {
        setError(e as Error);
      } finally {
        setLoading(false);
      }
    })();

    return unsubscribe;
  }, [method, JSON.stringify(input), options?.enabled]);

  return { data, loading, error };
}

// Usage in component
function AlbumList() {
  const { data: albums, loading } = useCachedQuery<Album>('albums.list', {});

  if (loading) return <Spinner />;

  // When ResourceChanged event arrives for an album:
  // 1. Cache updates
  // 2. This component re-renders
  // 3. User sees new data instantly!
  return (
    <div>
      {albums?.map(album => <AlbumCard key={album.id} album={album} />)}
    </div>
  );
}
```

## SwiftUI Integration

### ObservableObject Pattern

```swift
/// Observable cache for SwiftUI
@MainActor
class CachedQueryClient: ObservableObject {
    private let cache: NormalizedCache
    @Published private var queryResults: [String: Any] = [:]

    init(cache: NormalizedCache) {
        self.cache = cache

        // Subscribe to cache changes
        Task {
            for await notification in cache.changeStream {
                // Update published results
                queryResults[notification.queryKey] = notification.newValue
            }
        }
    }

    func query<T: Identifiable>(_ method: String, input: Encodable) async throws -> [T] {
        let results = try await cache.query(method, input: input)

        // Store in published results for observation
        let queryKey = cache.generateQueryKey(method, input)
        queryResults[queryKey] = results

        return results
    }

    func getQueryResult<T>(_ queryKey: String) -> [T]? {
        queryResults[queryKey] as? [T]
    }
}

// Usage in SwiftUI view
struct AlbumListView: View {
    @ObservedObject var client: CachedQueryClient
    @State private var albums: [Album] = []

    var body: some View {
        List(albums, id: \.id) { album in
            Text(album.name)
        }
        .task {
            albums = try await client.query("albums.list", input: EmptyInput())
        }
        // When ResourceChanged event arrives:
        // 1. Cache updates
        // 2. client publishes change
        // 3. SwiftUI re-renders
        // 4. User sees update instantly!
    }
}
```

## Memory Management

### LRU Eviction

```swift
actor NormalizedCache {
    private let maxEntities: Int = 10_000
    private var accessOrder: [String] = [] // LRU tracking

    func updateEntity<T: Identifiable>(_ resource: T) {
        let cacheKey = "\(T.resourceType):\(resource.id.uuidString)"

        // Update store
        entityStore[cacheKey] = resource

        // Update access order (LRU)
        if let index = accessOrder.firstIndex(of: cacheKey) {
            accessOrder.remove(at: index)
        }
        accessOrder.append(cacheKey)

        // Evict if over limit
        if entityStore.count > maxEntities {
            evictLRU()
        }
    }

    private func evictLRU() {
        // Evict oldest unreferenced entities
        let referencedKeys = Set(queryIndex.values.flatMap { $0.entityKeys })

        for key in accessOrder {
            if !referencedKeys.contains(key) {
                // Not in any active query - safe to evict
                entityStore.removeValue(forKey: key)
                accessOrder.removeAll { $0 == key }

                if entityStore.count <= maxEntities * 9 / 10 {
                    break // Evicted 10% - done
                }
            }
        }
    }
}
```

### TTL (Time-To-Live)

```swift
struct QueryCacheEntry {
    var entityKeys: Set<String>
    let fetchedAt: Date
    let ttl: TimeInterval = 300 // 5 minutes default

    var isExpired: Bool {
        Date().timeIntervalSince(fetchedAt) > ttl
    }
}

// Different TTLs per query type
func getTTL(for method: String) -> TimeInterval {
    switch method {
    case "files.search": return 60      // 1 minute (changes frequently)
    case "albums.list": return 300      // 5 minutes (changes rarely)
    case "core.status": return 10       // 10 seconds (real-time)
    default: return 300
    }
}
```

### Reference Counting

```swift
// Track which queries reference each entity
private var entityRefCounts: [String: Int] = [:]

func removeQuery(_ queryKey: String) {
    guard let entry = queryIndex[queryKey] else { return }

    // Decrement ref counts
    for entityKey in entry.entityKeys {
        entityRefCounts[entityKey, default: 0] -= 1

        // If no longer referenced, can evict
        if entityRefCounts[entityKey] == 0 {
            entityStore.removeValue(forKey: entityKey)
            entityRefCounts.removeValue(forKey: entityKey)
        }
    }

    queryIndex.removeValue(forKey: queryKey)
}
```

## Event-Driven Updates

### Integration with Event System

```swift
actor EventCacheUpdater {
    let cache: NormalizedCache

    func start(eventStream: AsyncStream<Event>) async {
        for await event in eventStream {
            await handleEvent(event)
        }
    }

    func handleEvent(_ event: Event) async {
        switch event.kind {
        case .ResourceChanged(let resourceType, let resourceJSON):
            // Decode resource
            guard let resource = try? ResourceTypeRegistry.decode(
                resourceType: resourceType,
                from: resourceJSON
            ) else {
                print("Failed to decode \(resourceType)")
                return
            }

            // Update cache (triggers UI updates)
            await cache.updateEntity(resource)

        case .ResourceBatchChanged(let resourceType, let resourcesJSON, _):
            // Batch update
            for json in resourcesJSON {
                if let resource = try? ResourceTypeRegistry.decode(resourceType: resourceType, from: json) {
                    await cache.updateEntity(resource)
                }
            }

        case .ResourceDeleted(let resourceType, let resourceId):
            // Remove from cache
            await cache.deleteEntity(resourceType: resourceType, id: resourceId)

        case .BulkOperationCompleted(let resourceType, _, _, let hints):
            // Invalidate affected queries
            await cache.invalidateQueriesMatching { queryKey in
                // Match by location_id or other hints
                if let locationId = hints["location_id"] as? String {
                    return queryKey.contains(locationId)
                }
                return queryKey.contains(resourceType)
            }

        default:
            break
        }
    }
}
```

### Gap Detection

When events have sequence numbers, detect gaps caused by network issues:

```swift
actor NormalizedCache {
    private var lastEventSequence: [UUID: UInt64] = [:] // library_id → sequence

    func processEvent(_ event: Event) async {
        guard let libraryId = event.envelope.library_id,
              let sequence = event.envelope.sequence else {
            return
        }

        let lastSeq = lastEventSequence[libraryId] ?? 0

        if sequence > lastSeq + 1 {
            // Gap detected! Missed events
            print("️ Gap detected: expected \(lastSeq + 1), got \(sequence)")
            await reconcileState(libraryId: libraryId, fromSequence: lastSeq + 1)
        }

        // Update sequence tracker
        lastEventSequence[libraryId] = sequence

        // Process event normally
        await handleEvent(event)
    }

    /// Reconcile state after detecting missed events
    func reconcileState(libraryId: UUID, fromSequence: UInt64) async {
        print("Reconciling state from sequence \(fromSequence)")

        // Option 1: Fetch missed events
        if let missedEvents = try? await client.query(
            "events.since.v1",
            input: ["library_id": libraryId, "sequence": fromSequence]
        ) {
            for event in missedEvents {
                await processEvent(event)
            }
        }

        // Option 2: Full cache invalidation (fallback)
        invalidateLibrary(libraryId)
    }
}
```

## Cache Persistence (Offline Support)

### SQLite Storage

```swift
import SQLite

actor NormalizedCache {
    private let db: Connection

    init() {
        // SQLite database for cache persistence
        let path = FileManager.default
            .urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("spacedrive_cache.db")

        db = try! Connection(path.path)
        createTables()
    }

    func createTables() {
        try! db.run("""
            CREATE TABLE IF NOT EXISTS entities (
                cache_key TEXT PRIMARY KEY,
                resource_type TEXT NOT NULL,
                resource_data TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )
        """)

        try! db.run("""
            CREATE TABLE IF NOT EXISTS queries (
                query_key TEXT PRIMARY KEY,
                entity_keys TEXT NOT NULL,
                fetched_at INTEGER NOT NULL,
                ttl INTEGER NOT NULL
            )
        """)
    }

    func updateEntity<T: Identifiable>(_ resource: T) async {
        let cacheKey = "\(T.resourceType):\(resource.id.uuidString)"
        let json = try! JSONEncoder().encode(resource)

        // Update memory
        entityStore[cacheKey] = resource

        // Persist to disk
        let stmt = try! db.prepare("""
            INSERT OR REPLACE INTO entities (cache_key, resource_type, resource_data, updated_at)
            VALUES (?, ?, ?, ?)
        """)
        try! stmt.run(cacheKey, T.resourceType, String(data: json, encoding: .utf8)!, Date().timeIntervalSince1970)
    }

    /// Load cache from disk on startup
    func loadFromDisk() async {
        let stmt = try! db.prepare("SELECT cache_key, resource_data FROM entities")

        for row in stmt {
            let cacheKey = row[0] as! String
            let jsonString = row[1] as! String

            // Deserialize using type registry
            let parts = cacheKey.split(separator: ":")
            let resourceType = String(parts[0])

            if let data = jsonString.data(using: .utf8),
               let resource = try? ResourceTypeRegistry.decode(resourceType: resourceType, from: data) {
                entityStore[cacheKey] = resource
            }
        }

        print("Loaded \(entityStore.count) entities from disk cache")
    }
}
```

## Optimistic Updates

```swift
actor NormalizedCache {
    private var optimisticUpdates: [UUID: any Identifiable] = [:] // pending_id → resource

    /// Apply optimistic update immediately
    func updateOptimistically<T: Identifiable>(pendingId: UUID, resource: T) {
        let cacheKey = "\(T.resourceType):\(resource.id.uuidString)"

        // Store in both places
        entityStore[cacheKey] = resource
        optimisticUpdates[pendingId] = resource

        // Notify observers (UI updates instantly!)
        notifyAffectedQueries(cacheKey)
    }

    /// Commit optimistic update when server confirms
    func commitOptimisticUpdate(pendingId: UUID, confirmedResource: any Identifiable) {
        optimisticUpdates.removeValue(forKey: pendingId)
        updateEntity(confirmedResource) // Final update
    }

    /// Rollback optimistic update on error
    func rollbackOptimisticUpdate(pendingId: UUID) {
        guard let resource = optimisticUpdates.removeValue(forKey: pendingId) else {
            return
        }

        let cacheKey = "\(type(of: resource).resourceType):\(resource.id.uuidString)"
        entityStore.removeValue(forKey: cacheKey)
        notifyAffectedQueries(cacheKey)
    }
}

// Usage example
func renameAlbum(id: UUID, newName: String) async throws {
    let pendingId = UUID()

    // 1. Optimistic update (instant UI)
    let optimisticAlbum = Album(id: id, name: newName, cover: nil)
    await cache.updateOptimistically(pendingId: pendingId, resource: optimisticAlbum)

    do {
        // 2. Send action to server
        let confirmed = try await client.action("albums.rename.v1", input: ["id": id, "name": newName])

        // 3. Commit (replace optimistic with confirmed)
        await cache.commitOptimisticUpdate(pendingId: pendingId, confirmedResource: confirmed)
    } catch {
        // 4. Rollback on error
        await cache.rollbackOptimisticUpdate(pendingId: pendingId)
        throw error
    }
}
```

## Query Invalidation

### Manual Invalidation

```swift
// After bulk operations
cache.invalidateQuery("files.search", input: searchParams)

// After mutations
cache.invalidateQueriesMatching { queryKey in
    queryKey.contains("albums.list")
}

// Clear entire library
cache.invalidateLibrary(libraryId)
```

### Automatic Invalidation

```swift
// ResourceBatchChanged with hints
case .ResourceBatchChanged(_, _, let operation):
    switch operation {
    case .Index:
        // Invalidate directory listings
        cache.invalidateQueriesMatching { $0.contains("directory:") }
    case .WatcherBatch:
        // Keep cache (events contain full data)
        break
    }
```

## Memory Budget

```swift
struct CacheConfig {
    // Entity store limits
    let maxEntities: Int = 10_000           // ~10MB at 1KB/entity
    let evictionThreshold: Int = 9_000      // Start evicting at 90%

    // Query limits
    let maxQueries: Int = 100
    let defaultTTL: TimeInterval = 300      // 5 minutes

    // Persistence
    let persistToDisk: Bool = true
    let maxDiskSize: Int64 = 50_000_000     // 50MB
}
```

## Testing

### Unit Tests

```swift
func testCacheUpdate() async {
    let cache = NormalizedCache()

    // Store entity
    let album = Album(id: UUID(), name: "Test", cover: nil)
    await cache.updateEntity(album)

    // Verify stored
    let retrieved = await cache.getEntity(Album.self, id: album.id)
    XCTAssertEqual(retrieved?.name, "Test")
}

func testQueryInvalidation() async {
    let cache = NormalizedCache()

    // Query and cache
    let albums = try await cache.query("albums.list", input: EmptyInput())
    XCTAssertEqual(albums.count, 5)

    // Invalidate
    await cache.invalidateQuery("albums.list", input: EmptyInput())

    // Verify cache miss
    let cached = await cache.getQueryResult("albums.list", input: EmptyInput())
    XCTAssertNil(cached)
}
```

### Integration Tests

1. **Real-time update**: Create album on Device A → Event → Device B cache updates
2. **Offline resilience**: Disconnect → Queue writes → Reconnect → Sync
3. **Memory limits**: Load 20K entities → Verify LRU eviction
4. **Gap detection**: Miss events → Detect gap → Reconcile

## Performance Metrics

### Cache Hit Rates (Target)
- File queries: >90% hit rate
- Album/Tag queries: >95% hit rate
- Search queries: >70% hit rate (more volatile)

### Memory Usage (Typical)
- Entity store: 5-10MB (5K-10K entities)
- Query index: 1-2MB (100 queries)
- Total: <15MB

### Update Latency
- Event received → Cache updated: <1ms
- Cache updated → UI re-renders: <16ms (1 frame)
- Total: <20ms from server to UI

## Implementation Checklist

### Swift
- [ ] Create `NormalizedCache` actor
- [ ] Implement entity store + query index
- [ ] Implement `EventCacheUpdater`
- [ ] Create `ResourceTypeRegistry`
- [ ] Add LRU eviction
- [ ] Add SQLite persistence
- [ ] Create `CachedQueryClient` (ObservableObject)
- [ ] Create SwiftUI view integration
- [ ] Unit tests
- [ ] Integration tests

### TypeScript/React
- [ ] Create `NormalizedCache` class
- [ ] Implement entity store + query index
- [ ] Create `EventCacheUpdater`
- [ ] Create `ResourceTypeRegistry`
- [ ] Add LRU eviction
- [ ] Add IndexedDB persistence
- [ ] Create `useCachedQuery` hook
- [ ] Create React integration examples
- [ ] Unit tests
- [ ] Integration tests

## Migration Strategy

### Phase 1: Parallel Systems
- New cache runs alongside existing query system
- No breaking changes
- Opt-in per view/component

### Phase 2: Gradual Adoption
- Migrate high-traffic views first (file browser, search)
- Measure: Cache hit rate, UI responsiveness
- Iterate on memory management

### Phase 3: Full Migration
- All queries use cache
- Remove old query caching logic
- Cleanup legacy code

## Edge Cases

### Circular References

```swift
// File references Album, Album references Files (cover)
// Solution: Store by ID, resolve lazily

struct Album {
    let id: UUID
    let name: String
    let coverFileId: UUID? // Just ID, not full File object
}

// UI resolves when needed:
let coverFile = cache.getEntity(File.self, id: album.coverFileId)
```

### Large Resources

```swift
// File with 1000 tags (rare but possible)
// Solution: Paginate relationships or use lazy loading

struct File {
    let id: UUID
    let name: String
    let tagIds: [UUID]  // Just IDs
    // NOT: tags: [Tag]  // Would explode memory
}

// Load tags on demand:
let tags = album.tagIds.compactMap { cache.getEntity(Tag.self, id: $0) }
```

## References

- **Sync System**: `docs/core/sync.md`
- **Event System**: `docs/core/events.md`
- **Design Details**: `docs/core/design/sync/NORMALIZED_CACHE_DESIGN.md` (2674 lines, comprehensive)
- **Client Architecture**: `docs/core/design/sync/SYNC_TX_CACHE_MINI_SPEC.md`
