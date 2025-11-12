# Spacedrive Normalized Cache System Analysis

## Executive Summary

Spacedrive has implemented a normalized cache system with an `Identifiable` trait pattern on the backend and a `useNormalizedCache` hook on the frontend. However, there's a significant gap between the **intended design** (generic, trait-based) and the **current implementation** (special-cased with hardcoded logic).

### Current Status
- **What exists:** Identifiable trait, ResourceManager, useNormalizedCache hook
- **What's broken:** File resource doesn't implement Identifiable, cache updates require special-case logic for sd_path and content_identity
- **Why:** The system was designed to be generic but evolved with real-world complexity (virtual resources, path handling, deduplication)

---

## Part 1: Current State of the System

### 1.1 Backend: Identifiable Trait (PARTIALLY IMPLEMENTED)

**Location:** `/core/src/domain/resource.rs`

```rust
pub trait Identifiable: Serialize + for<'de> Deserialize<'de> + Type {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str where Self: Sized;
    fn sync_dependencies() -> &'static [&'static str] where Self: Sized { &[] }
}
```

**Resources implementing Identifiable:**
- Location (file locations)
- Space (sidebar layouts)
- SpaceGroup (sidebar groups)
- SpaceItem (sidebar items)
- File (NOT IMPLEMENTED - should be!)
- Tag (NOT IMPLEMENTED)
- Device (NOT IMPLEMENTED)
- ContentIdentity (NOT IMPLEMENTED)

**Problem:** File is the most important resource type but doesn't implement Identifiable. This forces workarounds throughout the caching layer.

### 1.2 Resource Manager: Virtual Resource Mapping

**Location:** `/core/src/domain/resource_manager.rs`

The ResourceManager handles a complex mapping problem:

```
ContentIdentity changed → Which Files are affected?
  ↓
  Look up all Entries with this ContentIdentity.id
  ↓
  Emit File events for affected entries
  ↓
  Frontend cache merges updates
```

**The complexity:** Files are "virtual resources" computed from:
- Entry (filesystem entry)
- ContentIdentity (deduplication/content hash)
- Sidecar (thumbnails, metadata)
- Tags (applied to files)

When any dependency changes, the system must:
1. Map the dependency change to affected Files
2. Reconstruct the File domain model
3. Emit ResourceChanged events
4. Frontend merges into cache

### 1.3 Frontend: useNormalizedCache Hook

**Location:** `/packages/ts-client/src/hooks/useNormalizedCache.ts`

The hook wraps TanStack Query with event-driven cache updates:

```tsx
const { data: locations } = useNormalizedCache({
  wireMethod: 'query:locations.list',
  input: {},
  resourceType: 'location',
});
```

**What it does:**
1. Uses TanStack Query for normal query lifecycle
2. Listens for ResourceChanged/ResourceChangedBatch events
3. When event arrives, atomically updates TanStack Query cache with deepMerge
4. Component re-renders with new data instantly

### 1.4 The deepMerge Function: WHERE THE HACKS LIVE

**Location:** `/packages/ts-client/src/hooks/useNormalizedCache.ts:9-47`

```typescript
function deepMerge(existing: any, incoming: any): any {
  // If incoming is null/undefined, keep existing
  if (incoming === null || incoming === undefined) {
    return existing !== null && existing !== undefined ? existing : incoming;
  }

  // ... standard merge logic ...

  for (const key in existing) {
    // SPECIAL CASE #1: sd_path should never be merged
    if (key === 'sd_path') {
      continue; // Use incoming sd_path as-is
    }

    // ... rest of merge ...
  }

  return merged;
}
```

**Current special cases in useNormalizedCache:**

1. **sd_path special handling** (lines 25-28, 277, 393)
   - sd_path is never merged, always replaced
   - Prevents stale path data from surviving

2. **content_identity UUID matching** (lines 206-215, 277-294, 335-345, 392-408)
   - When matching by content UUID instead of file ID
   - Handles "multiple File entries with same content" case
   - Used for sidecar updates where single Entry produces multiple File resources

3. **Single object vs wrapped response detection** (lines 304-351)
   - Checks if response is `{ id, sd_path }` (single File) or `{ files: [...] }` (wrapped)
   - Adds complexity to cache update logic

4. **Resource filter and global list handling** (lines 159-188, 254-299, 372-413)
   - isGlobalList allows appending new items
   - resourceFilter checks if item belongs in this query scope

**Why these hacks exist:**
- File is a virtual resource with multiple possible identifiers (id, content UUID)
- Content-based paths can map to multiple Files
- Sidecars affect Files, not stored directly
- sd_path represents location in VDFS, not a stable ID

---

## Part 2: Original Intended Design

### 2.1 Design Goal: Generic Normalized Cache

Based on the trait design and comments, the intended system was:

```
Every Resource Type:
  ├─ Implements Identifiable trait
  ├─ Has stable UUID id
  ├─ Is serializable
  └─ Can emit ResourceChanged events

Frontend:
  ├─ Receives ResourceChanged(resourceType, resource)
  ├─ Uses resource.id as cache key
  ├─ Merges into TanStack Query cache
  └─ No special logic needed
```

**Key design principles:**
1. **Identity is the unique ID field** - All resources have stable Uuid
2. **One resource = one cache entry** - No multiple identifiers
3. **Update means replace** - Merge incoming over existing
4. **Type-driven** - resource_type determines handlers

### 2.2 How It Was Supposed to Work

**Backend:**
```rust
// 1. Resource changes in database
location.update(...)

// 2. Emit typed event
manager.emit_resource_events("location", vec![location_id]).await?

// 3. Event sent to frontend as JSON
Event::ResourceChanged {
  resource_type: "location",
  resource: serde_json::to_value(&location)? // Full Location struct
}
```

**Frontend:**
```typescript
// 1. Event arrives
ResourceChanged { resource_type: "location", resource: {...location data...} }

// 2. Update cache by ID (generic)
queryClient.setQueryData(queryKey, (old) => {
  const updated = old.map(item => 
    item.id === resource.id ? resource : item
  );
  return updated;
});

// 3. Done! No special cases needed
```

### 2.3 Sync Dependencies: Handling Virtual Resources

The Identifiable trait includes sync_dependencies for virtual resources:

```rust
pub fn sync_dependencies() -> &'static [&'static str] {
  // File depends on Entry, ContentIdentity, Sidecar
  // When any dependency changes, File resource is affected
}
```

This was supposed to encapsulate the mapping logic, but it's currently hardcoded in map_dependency_to_virtual_ids.

---

## Part 3: Why Custom Logic Was Added

### 3.1 Problem 1: File is a Virtual Resource

**The issue:**
```
Database tables:          Frontend cache:
├─ entry                  ├─ location (simple)
├─ content_identity       ├─ tag (simple)
└─ sidecar                └─ file (virtual!)

File = Entry + ContentIdentity + Sidecar + ...
```

**The consequence:**
- File ID = Entry UUID
- But File data comes from multiple tables
- When ContentIdentity changes, which Files are affected?
  - Must query: which Entries have this content_id?
  - But Entry UUID ≠ File ID anymore

**The workaround:**
- Added map_dependency_to_virtual_ids() to handle mapping
- Added sync_dependencies() to Identifiable
- Added special logic in ResourceManager to reconstruct File

### 3.2 Problem 2: Multiple Paths to the Same Content

**The issue:**
```
File can be addressed multiple ways:
1. By Entry UUID: "file123"
2. By Content UUID: "content-abc123"  (deduplication)
3. By Physical path: "/user/files/..."
4. By Content path: "content://content-abc123"

Sidecar update:
  ├─ Affects ContentIdentity
  ├─ Could affect multiple Files (same content, different locations)
  └─ Frontend must match by content UUID, not file ID
```

**The workaround:**
- deepMerge checks both `item.id` and `item.content_identity.uuid`
- Lines 206-215, 335-345 do this matching
- Allows single sidecar event to update multiple File entries

### 3.3 Problem 3: sd_path is Not an ID

**The issue:**
```
sd_path represents: "where in VDFS is this file?"
  ├─ Physical { device: "macbook", path: "/Users/me/file.txt" }
  ├─ Content { content_id: UUID }
  ├─ Cloud { provider: "gdrive", path: "..." }
  └─ Catalog { ... }

It's contextual and can change:
  - When file moves: new path
  - When indexed differently: new path type
  - When accessed from different device: different device_slug

But if we used sd_path as the cache key:
  - Move file → new sd_path → seen as new resource
  - Cache has both old and new, never merged
  - Duplicates pile up
```

**The workaround:**
- deepMerge explicitly skips merging sd_path
- Always replaces it: `merged[sd_path] = incoming.sd_path`
- Prevents stale paths from being preserved

### 3.4 Problem 4: Missing Identifiable Implementations

**File:**
```rust
// Why File doesn't implement Identifiable:
// 1. It's virtual (no single DB table)
// 2. Needs custom from_entry_uuids() logic
// 3. Would require associated type for the ID source

// Current workaround: Hardcode in ResourceManager
```

**Tag:**
```rust
// Why Tag doesn't implement Identifiable:
// 1. Tags are complex (relationships, compositions)
// 2. Need to handle tag relationships
// 3. Didn't need caching yet

// Workaround: No tag cache events yet
```

**ContentIdentity:**
```rust
// Why ContentIdentity doesn't implement Identifiable:
// 1. Identified by UUID field, but also by content_hash
// 2. Multiple entries can share same content
// 3. More of a dependency than a user-facing resource

// Workaround: Mapped to File events instead
```

---

## Part 4: What Other Systems Do

### 4.1 Apollo Client

**Pattern:** __typename + id fields
```typescript
// Apollo cache key
"User:123"  // concatenation of __typename and id

// Update pattern
cache.modify({
  fields: {
    user(value, { DELETE }) {
      return DELETE;  // delete this user
    }
  }
})

// Or use writeQuery
cache.writeQuery({
  query: GET_USER,
  data: { user: newUserData }
})
```

**Key points:**
- Every type must have an id (configurable via keyFields)
- No special cases in merge logic
- Type information in every response (__typename)
- Automatic deduplication by type+id

**Similar to Spacedrive:**
- Uses __typename (equivalent to resource_type)
- Requires stable id field
- Normalizes by id

**Differences:**
- Apollo uses GraphQL introspection for type info
- Spacedrive uses Rust trait system
- Apollo has configurable key fields, Spacedrive assumes UUID

### 4.2 Relay Modern

**Pattern:** Recorded-based normalized store
```
Store = Map<data_id, Record>

Record {
  id: "User:123",
  __typename: "User",
  name: "Alice",
  friends: [__ref: "User:456"]
}

Update = setUpdateHandler to sync Record in Store
```

**Key points:**
- Uses __typename + id as data_id
- Stores references between records
- Update handlers for syncing
- Automatic deduplication across queries

**Similar to Spacedrive:**
- Uses __typename (resource_type)
- Normalizes by id
- Handles virtual resources

**Differences:**
- Relay uses references, Spacedrive embeds full resource
- Relay compiles queries ahead of time
- Spacedrive discovers types dynamically

### 4.3 SWR / React Query

**Pattern:** Query key-based, not normalized
```typescript
const { data } = useSWR('users/123', fetcher);
// Separate query per endpoint, no global normalization

// Manual merge on mutation
mutate('users/123', newData, false);
```

**Key points:**
- No built-in normalization
- Each query is independent
- Manual cache management
- Simpler but duplicates data

**Different from Spacedrive:**
- Spacedrive uses global normalization (better!)
- Spacedrive event-driven (better for real-time!)

---

## Part 5: What a Clean, Generic Solution Would Look Like

### 5.1 Phase 1: Make File Identifiable

```rust
// In core/src/domain/file.rs

impl Identifiable for File {
    fn id(&self) -> Uuid {
        self.id  // Entry UUID is File ID
    }

    fn resource_type() -> &'static str {
        "file"
    }

    fn sync_dependencies() -> &'static [&'static str] {
        // File depends on these - when they change, rebuild File
        &["entry", "content_identity", "sidecar"]
    }
}
```

### 5.2 Phase 2: Remove Special Cases from deepMerge

The deepMerge function becomes truly generic:

```typescript
function deepMerge(existing: any, incoming: any): any {
  if (incoming === null || incoming === undefined) {
    return existing !== null && existing !== undefined ? existing : incoming;
  }

  if (typeof existing !== 'object' || typeof incoming !== 'object' ||
      Array.isArray(existing) || Array.isArray(incoming)) {
    return incoming;
  }

  const merged: any = { ...incoming };
  for (const key in existing) {
    // NO SPECIAL CASES!
    if (!(key in incoming)) {
      merged[key] = existing[key];
    } else if (typeof existing[key] === 'object' && typeof incoming[key] === 'object' &&
               !Array.isArray(existing[key]) && !Array.isArray(incoming[key])) {
      merged[key] = deepMerge(existing[key], incoming[key]);
    }
  }

  return merged;
}
```

### 5.3 Phase 3: Update Cache by ID Only

```typescript
// Before: had to check both id and content_identity.uuid
const existing = array.find(item => item.id === resource.id);

// After: always by id, ResourceManager ensures correct ID
const existing = array.find(item => item.id === resource.id);
newArray[existingIndex] = deepMerge(existing, resource);
```

### 5.4 Phase 4: Handle Paths as Normal Fields

The VDFS path (sd_path) becomes just another field:

```typescript
// Before: special case to prevent path corruption
if (key === 'sd_path') continue;

// After: merges naturally like any field
merged[key] = deepMerge(existing[key], incoming[key]);

// Or if path should always be latest:
// Handled by ResourceManager ensuring full File is sent
```

**Key insight:** If ResourceManager sends the complete File with latest sd_path, deepMerge doesn't need special cases.

### 5.5 Phase 5: Metadata-Driven Behavior (Future)

Allow resource types to customize merge behavior:

```rust
pub trait Identifiable {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str;
    fn sync_dependencies() -> &'static [&'static str] { &[] }
    
    // New: Define which fields should never be merged
    fn immutable_fields() -> &'static [&'static str] { &[] }
    
    // New: Custom merge logic per type
    fn merge_strategy() -> MergeStrategy { MergeStrategy::Replace }
}

pub enum MergeStrategy {
    Replace,        // Incoming replaces existing
    Merge,          // Deep merge
    Custom(fn)      // Type-specific logic
}
```

Then frontend uses this info:
```typescript
const strategy = resourceMetadata[resourceType].merge_strategy;
if (strategy === 'Replace') {
  newData[i] = resource;
} else if (strategy === 'Merge') {
  newData[i] = deepMerge(newData[i], resource);
}
```

---

## Part 6: Migration Path from Current to Ideal State

### Phase 1: Immediate Wins (Week 1)

**Goal:** Remove deepMerge special cases without breaking functionality

**Steps:**
1. Implement Identifiable for File
   ```rust
   impl Identifiable for File {
       fn id(&self) -> Uuid { self.id }
       fn resource_type() -> &'static str { "file" }
       fn sync_dependencies() -> &'static [&'static str] { &["entry", "content_identity", "sidecar"] }
   }
   ```

2. Update File::from_entry_uuids() to ensure correct ID
   - File ID MUST be Entry UUID
   - Use this ID consistently

3. Simplify deepMerge - remove sd_path special case
   - Ensure ResourceManager sends full File with current sd_path
   - No need for special handling

4. Remove content_identity UUID matching
   - File ID is the source of truth
   - Sidecar events update the correct File by ID

**Tests:**
- File cache updates correctly by ID
- Sidecar changes update the right File
- No stale paths in cache

### Phase 2: Consolidate Virtual Resource Logic (Week 2)

**Goal:** Move dependency mapping into trait system

**Steps:**
1. Create ResourceMapper trait
   ```rust
   pub trait ResourceMapper {
       async fn map_dependency(
           db: &DatabaseConnection,
           changed: (&str, Uuid),
       ) -> Result<Vec<(&str, Vec<Uuid>)>>;
   }
   ```

2. Implement for each virtual resource
   ```rust
   struct FileMapper;
   impl ResourceMapper for FileMapper {
       async fn map_dependency(...) {
           // Current map_dependency_to_virtual_ids logic
           // But organized by type
       }
   }
   ```

3. Register mappers in ResourceManager
   ```rust
   pub struct ResourceManager {
       mappers: HashMap<&str, Box<dyn ResourceMapper>>,
   }
   ```

4. Remove hardcoded match statements
   - ResourceManager queries registry instead

**Tests:**
- ContentIdentity → File mapping works
- Sidecar → File mapping works
- Multiple files with same content get updated

### Phase 3: Extend to All Resource Types (Week 3-4)

**Goal:** Full Identifiable implementations

**Steps:**
1. Implement Identifiable for Tag
   - Handle tag relationships
   - Resource type: "tag"

2. Implement Identifiable for Device
   - Handle device-specific caching
   - Resource type: "device"

3. Implement Identifiable for ContentIdentity
   - Or keep as dependency-only (no direct cache events)

4. Update frontend to use resource_type discovery
   ```typescript
   // Instead of hardcoding "location", "file", "tag"
   useNormalizedCache({
     wireMethod: 'query:resources.list',
     resourceType: resourceMetadata.getType(queryResult[0]), // Auto-detect
   })
   ```

**Tests:**
- All resource types emit events correctly
- Frontend caches update automatically
- No resource type-specific code in useNormalizedCache

### Phase 4: Clean Cache Merge Strategy (Week 5)

**Goal:** Metadata-driven merge behavior

**Steps:**
1. Add merge strategy to Identifiable
   ```rust
   fn merge_strategy() -> MergeStrategy { MergeStrategy::DeepMerge }
   fn immutable_fields() -> &'static [&'static str] { &[] }
   ```

2. Frontend checks metadata before merging
   ```typescript
   const metadata = await client.getResourceMetadata(resourceType);
   const merged = metadata.merge_strategy === 'Replace' 
       ? resource 
       : deepMerge(existing, resource);
   ```

3. Remove all hardcoded special cases
   - No more sd_path checks
   - No more content_identity UUID matching
   - No more type detection hacks

4. deepMerge becomes truly generic
   - Works for all types
   - No special knowledge of fields

**Tests:**
- Cache behavior matches metadata
- Different resource types merge correctly
- New resource types work automatically

### Phase 5: Optimize Frontend (Week 6)

**Goal:** More efficient cache updates

**Steps:**
1. Use ID as cache key in TanStack Query
   ```typescript
   const queryKey = ['resources', resourceType, resourceId];
   // Instead of ['locations.list'] with array inside
   ```

2. Implement proper cache invalidation
   ```typescript
   queryClient.invalidateQueries({
     queryKey: ['resources', 'file'],
   });
   ```

3. Add cache persistence
   ```typescript
   // Store resourceType/id cache to localStorage
   // Rehydrate on app start
   ```

4. Add resource relationship traversal
   ```typescript
   // When File updates, auto-update containing Location's file_count
   ```

**Tests:**
- Cache size doesn't grow unbounded
- Updates propagate to dependent resources
- App remains responsive with large datasets

---

## Part 7: Current Blockers and Technical Debt

### 7.1 File as Virtual Resource

**Issue:** File doesn't have a single DB table source
**Impact:** Can't implement Identifiable easily
**Solution:** 
- Define File ID as Entry UUID
- Reconstruct File from Entry + dependencies when needed
- Keep this logic in from_entry_uuids()

### 7.2 Multiple Identifiers for Same Resource

**Issue:** File can be looked up by:
- Entry UUID (file123)
- Content UUID (content-abc)
- Physical path (/Users/me/file)
- Content path (content://abc)

**Current Impact:** deepMerge must check multiple fields
**Solution:**
- Use Entry UUID as primary ID
- Store content_uuid as a field, not an ID
- Paths are fields, not IDs
- ResourceManager resolves all paths to primary IDs before emitting

### 7.3 Sidecar Updates → File Updates

**Issue:** Sidecar table has no direct File reference
**Impact:** Must query: which Entries have this ContentIdentity?
**Solution:** Keep current mapping in ResourceManager, just organize it better
- Map[ContentIdentity.uuid] → Entry.uuid → File (File ID)
- Emit File event with correct ID

### 7.4 No Metadata Exchange

**Issue:** Frontend doesn't know about resource merge strategies
**Impact:** Hardcoded logic in useNormalizedCache
**Solution:** 
- Add `core:resources.metadata` query
- Returns merge strategies, immutable fields, etc. for each type
- Frontend uses this to drive cache behavior

### 7.5 Event Overhead

**Issue:** ResourceChangedBatch uses array of full resources
**Impact:** Large JSON for events (could be optimized)
**Solution:** (Future optimization)
- Use patch format: { id, fields_changed: {...} }
- Only send changed fields, not full resource
- Requires merge strategy changes

---

## Part 8: Summary Table

| Aspect | Current | Intended | Gap |
|--------|---------|----------|-----|
| Identifiable trait | 4 types | All types | File, Tag, Device missing |
| File ID | Entry UUID | Entry UUID | ✓ Aligned |
| Virtual resource handling | Hardcoded in ResourceManager | Trait system | Needs registry |
| deepMerge | Full resource, 4 special cases | No special cases | Needs cleanup |
| Frontend cache key | id + content_uuid | id only | Needs simplification |
| Path handling | Special case in merge | Normal field | Needs ResourceManager fix |
| Resource type discovery | Hardcoded | Trait system | Future |
| Merge strategy | Hardcoded (deep merge) | Configurable | Future |
| Test coverage | Minimal | Comprehensive | Needs writing |

---

## Part 9: Recommended Starting Point

**START HERE:** Implement File::impl Identifiable and remove deepMerge special cases

**Why:**
1. File is the most important resource type
2. Special case logic is clearly visible and testable
3. Will unblock Tag and Device implementations
4. Frontend code becomes simpler immediately

**What to test:**
```rust
#[test]
async fn file_event_updates_cache() {
    // 1. Create File in database
    // 2. Emit ResourceChanged event
    // 3. Frontend cache should update by ID
    // 4. No special matching logic needed
}

#[test]
async fn sidecar_maps_to_correct_file() {
    // 1. Create Sidecar (references content_uuid)
    // 2. ResourceManager maps to File ID
    // 3. Emit File ResourceChanged
    // 4. Frontend cache updates single entry by ID
}

#[test]
async fn multiple_files_same_content() {
    // 1. Two entries, same content_identity
    // 2. Content identity updated (e.g., verified)
    // 3. Both File events emitted (correct IDs)
    // 4. Both cache entries updated independently
}
```

**Timeline:** 2-3 weeks to production-ready

