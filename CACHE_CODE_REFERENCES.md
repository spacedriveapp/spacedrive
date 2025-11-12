# Spacedrive Normalized Cache - Detailed Code References

## Backend Implementation Details

### 1. Identifiable Trait Definition

**File:** `/core/src/domain/resource.rs` (lines 1-62)

```rust
pub trait Identifiable: Serialize + for<'de> Deserialize<'de> + Type {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str where Self: Sized;
    fn sync_dependencies() -> &'static [&'static str] where Self: Sized { &[] }
}

pub fn is_virtual_resource(resource_type: &str) -> bool {
    match resource_type {
        "file" => true,
        _ => false,
    }
}

pub async fn map_dependency_to_virtual_ids(
    db: &sea_orm::DatabaseConnection,
    dependency_type: &str,
    dependency_id: Uuid,
) -> crate::common::errors::Result<Vec<(&'static str, Vec<Uuid>)>> {
    // Maps dependency changes to virtual resource IDs
    // E.g., ContentIdentity change → which File IDs?
}
```

**Status:**
- Trait is generic and extensible
- sync_dependencies() exists but underutilized
- Hard-coded virtual resource detection
- Hard-coded dependency mapping for File only

### 2. File Model (Missing Identifiable)

**File:** `/core/src/domain/file.rs` (lines 1-150)

```rust
pub struct File {
    pub id: Uuid,                              // Entry UUID
    pub sd_path: SdPath,                       // VDFS path
    pub name: String,
    pub size: u64,
    pub content_identity: Option<ContentIdentity>,  // Dedup info
    pub alternate_paths: Vec<SdPath>,          // Other locations
    pub tags: Vec<Tag>,
    pub sidecars: Vec<Sidecar>,
    // ... timestamps ...
}

// NOTE: No Identifiable impl!
// Should be:
// impl Identifiable for File {
//     fn id(&self) -> Uuid { self.id }
//     fn resource_type() -> &'static str { "file" }
//     fn sync_dependencies() -> &'static [&'static str] {
//         &["entry", "content_identity", "sidecar"]
//     }
// }
```

**Key method:** `File::from_entry_uuids()` (lines 227-430)
- Batch loads entries, content identities, sidecars
- Reconstructs File instances from raw DB data
- Complex because File is virtual resource

**Status:**
- Method works, fully implemented
- Efficient batch queries
- Not formalized as Identifiable
- Called directly by ResourceManager, not via trait

### 3. Location Model (Correct Implementation)

**File:** `/core/src/domain/location.rs` (lines 1-200)

```rust
pub struct Location {
    pub id: Uuid,
    pub name: String,
    pub sd_path: SdPath,
    // ... fields ...
}

impl Identifiable for Location {
    fn id(&self) -> Uuid {
        self.id
    }

    fn resource_type() -> &'static str {
        "location"
    }
    // No sync_dependencies, Location is simple
}
```

**Status:** Perfect example of correct implementation

### 4. Space, SpaceGroup, SpaceItem (Also Correct)

**File:** `/core/src/domain/space.rs` (lines 67-249)

```rust
impl Identifiable for Space {
    fn id(&self) -> Uuid { self.id }
    fn resource_type() -> &'static str { "space" }
}

impl Identifiable for SpaceGroup {
    fn id(&self) -> Uuid { self.id }
    fn resource_type() -> &'static str { "space_group" }
}

impl Identifiable for SpaceItem {
    fn id(&self) -> Uuid { self.id }
    fn resource_type() -> &'static str { "space_item" }
}
```

**Status:** All three implement correctly

### 5. ResourceManager (The Glue)

**File:** `/core/src/domain/resource_manager.rs` (lines 1-160)

```rust
pub struct ResourceManager {
    db: Arc<DatabaseConnection>,
    events: Arc<EventBus>,
}

impl ResourceManager {
    pub async fn emit_resource_events(
        &self,
        resource_type: &str,
        resource_ids: Vec<Uuid>,
    ) -> Result<()> {
        // For simple resources: emit directly
        // For dependencies: map to virtual resources first

        let mut all_virtual_resources = Vec::new();
        for resource_id in resource_ids {
            // Lines 66-71: Call map_dependency_to_virtual_ids
            let virtual_mappings = 
                map_dependency_to_virtual_ids(&self.db, resource_type, resource_id).await?;
            all_virtual_resources.extend(virtual_mappings);
        }

        // Lines 92-120: Handle each virtual type with match statement
        for (virtual_type, virtual_ids) in grouped {
            match virtual_type {
                "file" => {
                    // Construct File instances
                    let files = File::from_entry_uuids(&self.db, &virtual_ids).await?;
                    // Emit ResourceChanged event
                    self.events.emit(Event::ResourceChangedBatch {...});
                }
                _ => { }
            }
        }
    }
}
```

**The problem area:** Lines 92-120
- Hard-coded match for "file"
- Would need match for each virtual resource type
- Better approach: trait-based registry

**Status:**
- Works correctly
- Not extensible
- Handles File specially, no other virtual types

### 6. Event Definition

**File:** `/core/src/infra/event/mod.rs` (lines 162-182)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum Event {
    // ...
    
    ResourceChanged {
        resource_type: String,          // e.g., "location", "file"
        resource: serde_json::Value,   // Full resource as JSON
    },
    ResourceChangedBatch {
        resource_type: String,
        resources: serde_json::Value,   // Array of resources
    },
    ResourceDeleted {
        resource_type: String,
        resource_id: Uuid,
    },
}
```

**Status:** Design is correct and generic

---

## Frontend Implementation Details

### 1. useNormalizedCache Hook

**File:** `/packages/ts-client/src/hooks/useNormalizedCache.ts` (466 lines)

```typescript
export function useNormalizedCache<I, O>({
  wireMethod,
  input,
  resourceType,
  enabled = true,
  isGlobalList = false,
  resourceFilter,
}: UseNormalizedCacheOptions<I>) {
    const client = useSpacedriveClient();
    const queryClient = useQueryClient();

    // Line 107: Query key is method + input
    const queryKey = [wireMethod, libraryId, input];

    // Lines 110-118: Use TanStack Query normally
    const query = useQuery<O>({
        queryKey,
        queryFn: async () => {
            return await client.execute<I, O>(wireMethod, input);
        },
        enabled: enabled && !!libraryId,
    });

    // Lines 121-462: Listen for events and update cache
    useEffect(() => {
        const handleEvent = (event: any) => {
            // Lines 129-220: Handle ResourceChanged
            if ("ResourceChanged" in event) {
                const { resource_type, resource } = event.ResourceChanged;
                if (resource_type === resourceType) {
                    // Update cache with deepMerge
                }
            }
            // Lines 221-421: Handle ResourceChangedBatch
            // Lines 422-452: Handle ResourceDeleted
        };
        
        const unsubscribe = client.on("spacedrive-event", handleEvent);
        return () => client.off("spacedrive-event", handleEvent);
    }, [resourceType, queryKey, queryClient]);

    return query;
}
```

**Status:** Hook structure is good, too many special cases in event handler

### 2. deepMerge Function

**File:** `/packages/ts-client/src/hooks/useNormalizedCache.ts` (lines 9-47)

```typescript
function deepMerge(existing: any, incoming: any): any {
    // Lines 11-12: Preserve existing if incoming is null
    if (incoming === null || incoming === undefined) {
        return existing !== null && existing !== undefined ? existing : incoming;
    }

    // Lines 15-19: Types must match, arrays handled differently
    if (typeof existing !== 'object' || typeof incoming !== 'object' ||
        Array.isArray(existing) || Array.isArray(incoming)) {
        return incoming;
    }

    // Lines 21-22: Start with incoming object
    const merged: any = { ...incoming };

    // Lines 24-44: Merge in existing fields
    for (const key in existing) {
        // HACK #1: sd_path special case
        if (key === 'sd_path') {
            continue;  // Use incoming sd_path as-is, never merge
        }

        if (!(key in incoming)) {
            // Preserve field from existing
            merged[key] = existing[key];
        } else if (incoming[key] === null || incoming[key] === undefined) {
            // Preserve non-null from existing
            if (existing[key] !== null && existing[key] !== undefined) {
                merged[key] = existing[key];
            }
        } else if (typeof existing[key] === 'object' && typeof incoming[key] === 'object' &&
                   !Array.isArray(existing[key]) && !Array.isArray(incoming[key])) {
            // Recurse on objects
            merged[key] = deepMerge(existing[key], incoming[key]);
        }
    }

    return merged;
}
```

**Status:**
- Generic structure
- Has special case for sd_path

### 3. ResourceChanged Event Handling

**File:** `/packages/ts-client/src/hooks/useNormalizedCache.ts` (lines 129-220)

```typescript
if ("ResourceChanged" in event) {
    const { resource_type, resource } = event.ResourceChanged;

    if (resource_type === resourceType) {
        queryClient.setQueryData<O>(queryKey, (oldData) => {
            if (!oldData) return oldData;

            // Lines 146-164: Handle direct array response
            if (Array.isArray(oldData)) {
                const resourceId = resource.id;
                const existingIndex = oldData.findIndex(
                    (item: any) => item.id === resourceId,
                );

                if (existingIndex >= 0) {
                    const newData = [...oldData];
                    newData[existingIndex] = deepMerge(oldData[existingIndex], resource);
                    return newData as O;
                }

                // Lines 159-162: Append if global list or passes filter
                if (isGlobalList || (resourceFilter && resourceFilter(resource))) {
                    return [...oldData, resource] as O;
                }

                return oldData;
            } else if (oldData && typeof oldData === "object") {
                // Lines 165-215: Handle wrapped response like {locations: [...]}
                const arrayField = Object.keys(oldData).find((key) =>
                    Array.isArray((oldData as any)[key]),
                );

                if (arrayField) {
                    // ... similar logic for wrapped arrays ...
                }

                // Lines 193-215: Handle single object response
                // Check if oldData is a single File being displayed
                if ((oldData as any).id === resource.id) {
                    return deepMerge(oldData, resource) as O;
                }

                // HACK #2: Also check by content UUID
                if (
                    (oldData as any).content_identity?.uuid &&
                    (oldData as any).content_identity.uuid === resource.content_identity?.uuid
                ) {
                    return deepMerge(oldData, resource) as O;
                }
            }

            return oldData;
        });
    }
}
```

**Status:**
- Handles arrays and wrapped responses
- Lines 206-215: content_identity UUID matching is hack
- Multiple code paths for same operation

### 4. ResourceChangedBatch Handling

**File:** `/packages/ts-client/src/hooks/useNormalizedCache.ts` (lines 221-421)

```typescript
else if ("ResourceChangedBatch" in event) {
    const { resource_type, resources } = event.ResourceChangedBatch;

    if (resource_type === resourceType && Array.isArray(resources)) {
        queryClient.setQueryData<O>(queryKey, (oldData) => {
            if (!oldData) return oldData;

            // Lines 237: Map resources by ID for lookup
            const resourceMap = new Map(resources.map((r: any) => [r.id, r]));

            if (Array.isArray(oldData)) {
                // Lines 239-252: Update existing items by ID
                const newData = [...oldData];
                const seenIds = new Set();

                for (let i = 0; i < newData.length; i++) {
                    const item: any = newData[i];
                    if (resourceMap.has(item.id)) {
                        const incomingResource = resourceMap.get(item.id);
                        newData[i] = deepMerge(item, incomingResource);
                        seenIds.add(item.id);
                    }
                }

                // Lines 254-299: Append new items (with filter)
                if (isGlobalList) {
                    // Append all new items
                } else if (resourceFilter) {
                    // HACK #3: content_identity UUID matching for Content paths
                    if (resource.sd_path?.Content && resource.content_identity?.uuid) {
                        const contentId = resource.content_identity.uuid;
                        
                        // Lines 281-293: Find existing by content UUID
                        const existingIndex = newData.findIndex(
                            (item: any) => item.content_identity?.uuid === contentId
                        );

                        if (existingIndex >= 0) {
                            // Update existing, skip append
                            continue;
                        }
                    }
                    // Append new item
                    newData.push(resource);
                }

                return newData as O;
            }
            
            // Lines 302-351: Similar logic for wrapped/single responses
            // HACK #4: Single resource detection and matching
            const isSingleResource = !!(oldData as any).id && !!(oldData as any).sd_path;
            
            if (isSingleResource) {
                // Lines 325-346: Check each resource
                for (const resource of resources) {
                    if ((oldData as any).id === resource.id) {
                        return deepMerge(oldData, resource) as O;
                    }
                    if (
                        (oldData as any).content_identity?.uuid &&
                        (oldData as any).content_identity.uuid === resource.content_identity?.uuid
                    ) {
                        return deepMerge(oldData, resource) as O;
                    }
                }
            }
            
            // ... wrapped array handling ...
        });
    }
}
```

**Status:**
- Batch logic is well-structured
- Duplicates hack #2 in multiple places (lines 277-294, 335-345, 392-408)
- Hack #4: isSingleResource detection is fragile (checks for id + sd_path)

---

## Why These Hacks Were Added

### ContentIdentity UUID Matching (Hack #2)

**Scenario:**
```
File 1: id=uuid123, content_id=content-abc, path=/user/file1.jpg
File 2: id=uuid456, content_id=content-abc, path=/user/file2.jpg (same file, different location)

Sidecar created for content-abc (e.g., thumbnail):
  → ResourceChanged { id=content-abc, sidecars: [...] }
  
Frontend needs to:
  → Update BOTH File 1 and File 2
  → But event only has content ID, not file IDs!
  → Must match by content_identity.uuid
```

**Problem:** ResourceManager should emit separate events for each File:
```
ResourceChangedBatch {
  resources: [
    { id=uuid123, content_id=content-abc, sidecars: [...] },
    { id=uuid456, content_id=content-abc, sidecars: [...] }
  ]
}
```

But currently might emit single event with content ID.

### sd_path Special Case (Hack #1)

**Scenario:**
```
File: id=uuid123, sd_path=Physical{path="/users/file.txt"}, sidecars=[...]

Later: File moved or re-indexed
  → New event has sd_path=Content{id=content-abc}

If we merge:
  → existing.sd_path = Physical{...}
  → incoming.sd_path = Content{...}
  → merged.sd_path = Physical{...} (old path stays!)
  
Result: UI shows wrong location
```

**Better fix:** ResourceManager ensures File always has current sd_path before emitting
- File::from_entry_uuids() must always return current sd_path
- deepMerge doesn't need special handling

### Single Resource Detection (Hack #4)

**Scenario:**
```
Query returns single File:   vs   Query returns wrapped array:
{ id, name, sd_path, ... }       { files: [{ id, name, ... }] }

useNormalizedCache must detect which format to use different merge logic
```

**Better fix:** Hook should know response type from wireMethod
- Different queries have different shapes
- Should be normalized before reaching deepMerge
- Or hook should validate response schema

---

## Summary

| Code Location | What | Status | Why |
|---|---|---|---|
| resource.rs:18-46 | Identifiable trait | Good design | Well-designed, just incomplete |
| file.rs:43-79 | File struct | Complete | Has all fields needed |
| file.rs:missing | File::Identifiable | Missing | Virtual resource complexity |
| resource_manager.rs:92-120 | Hard-coded match | ️ Works but fragile | No trait registry |
| useNormalizedCache.ts:9-47 | deepMerge | ️ Has 1 hack | sd_path special case |
| useNormalizedCache.ts:206-215 | content_uuid matching | Hack | Should be in ResourceManager |
| useNormalizedCache.ts:277-294 | Batch content matching | Hack | Duplicated code |
| useNormalizedCache.ts:304-351 | Single object detection | ️ Works but fragile | Should use response type |

