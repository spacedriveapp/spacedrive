# Identifiable Trait Genericization Design

**Date:** 2025-11-13
**Status:** Design Proposal
**Context:** Following implementation of Spaces resource events system

---

## Problem Statement

The `Identifiable` trait defines `sync_dependencies()` to declare what resources a virtual resource depends on, but this information is **duplicated and unused** in the actual routing logic.

### Current Architecture Issues

**1. Unused Trait Method**

`sync_dependencies()` is defined on the trait but never called:

```rust
// File implements this
impl Identifiable for File {
    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar", "image_media_data", ...]
    }
}

// But we duplicate it here!
pub fn get_dependencies(resource_type: &str) -> &'static [&'static str] {
    match resource_type {
        "file" => &["entry", "content_identity", "sidecar"],  // DUPLICATE!
        _ => &[],
    }
}
```

**2. Manual Registration in 4 Locations**

Adding a new virtual resource requires updates to:

1. `resource.rs:79` - `is_virtual_resource()` match statement
2. `resource.rs:87` - `get_dependencies()` match statement (duplicates trait)
3. `resource.rs:101` - `map_dependency_to_virtual_ids()` routing logic (300+ lines)
4. `resource_manager.rs:94` - `emit_resource_events()` constructor dispatch

**3. Complex Routing Logic Scattered Across Files**

The knowledge of "how to map ContentIdentity changes to affected File IDs" lives in `resource.rs`, not with the `File` domain model where it belongs.

---

## Root Cause Analysis

### Why Does Custom Logic Exist?

Virtual resources have **three fundamentally different routing patterns**:

#### Pattern 1: Direct ID Mapping
```rust
"entry" => {
    // File ID = Entry UUID (1:1)
    results.push(("file", vec![dependency_id]));
}
```
- No database query needed
- Simple identity mapping

#### Pattern 2: Content-based Fan-Out
```rust
"content_identity" => {
    // 1 ContentIdentity → Many Files (1:N)
    let ci = load_content_identity(db, dependency_id).await?;
    let entries = find_entries_with_content(db, ci.id).await?;
    results.push(("file", entry_uuids));
}
```
- Requires database queries
- One change affects multiple virtual resources

#### Pattern 3: Hierarchical Rollup
```rust
"space_group" => {
    // Navigate hierarchy: SpaceGroup → Space → SpaceLayout
    let group = load_group(db, dependency_id).await?;
    let space = load_parent_space(db, group.space_id).await?;
    results.push(("space_layout", vec![space.uuid]));
}
```
- Requires navigation through relationships
- Multiple dependency types route to same virtual resource

### Why `sync_dependencies()` Isn't Enough

The trait tells us **what** depends on **what**, but not **how to route** changes. Each pattern needs different logic:

- Pattern 1: Use dependency ID directly
- Pattern 2: Query database to find affected IDs
- Pattern 3: Navigate hierarchy to find parent ID

---

## Proposed Solution

### Design Goals

1. **Single source of truth** - Register virtual resources in one place
2. **Co-locate logic** - Routing logic lives with domain model, not in global match statements
3. **Type safety** - Leverage Rust's type system to prevent registration errors
4. **Maintain performance** - No runtime overhead vs current approach
5. **Preserve clarity** - Complex routing logic should be explicit, not hidden

### Architecture: Trait Extension + Static Registry

#### 1. Extend Identifiable Trait

```rust
pub trait Identifiable: Serialize + for<'de> Deserialize<'de> + Type {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str where Self: Sized;

    // Existing: What this virtual resource depends on
    fn sync_dependencies() -> &'static [&'static str]
    where Self: Sized
    {
        &[]
    }

    // NEW: How to route dependency changes to virtual resource IDs
    async fn route_from_dependency(
        db: &DatabaseConnection,
        dependency_type: &str,
        dependency_id: Uuid,
    ) -> Result<Vec<Uuid>>
    where Self: Sized
    {
        Ok(vec![]) // Default: not a virtual resource
    }

    // NEW: Constructor for virtual resources
    async fn from_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
    ) -> Result<Vec<Self>>
    where Self: Sized
    {
        Err(CoreError::NotImplemented(
            "from_ids not implemented for simple resource".to_string()
        ))
    }

    // Existing methods
    fn alternate_ids(&self) -> Vec<Uuid> { vec![] }
    fn no_merge_fields() -> &'static [&'static str] where Self: Sized { &[] }
}
```

#### 2. Implement on Virtual Resources

**Example: File**

```rust
impl Identifiable for File {
    fn id(&self) -> Uuid {
        self.id
    }

    fn resource_type() -> &'static str {
        "file"
    }

    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar",
          "image_media_data", "video_media_data", "audio_media_data"]
    }

    // NEW: Routing logic co-located with domain model
    async fn route_from_dependency(
        db: &DatabaseConnection,
        dependency_type: &str,
        dependency_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        use crate::infra::db::entities::{content_identity, entry, sidecar};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        match dependency_type {
            // Pattern 1: Direct mapping
            "entry" => Ok(vec![dependency_id]),

            // Pattern 2: Fan-out via content
            "content_identity" => {
                let ci = content_identity::Entity::find()
                    .filter(content_identity::Column::Uuid.eq(dependency_id))
                    .one(db)
                    .await?
                    .ok_or_else(|| CoreError::NotFound(
                        format!("ContentIdentity {} not found", dependency_id)
                    ))?;

                let entries = entry::Entity::find()
                    .filter(entry::Column::ContentId.eq(ci.id))
                    .all(db)
                    .await?;

                Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
            }

            // Pattern 2: Fan-out via sidecar
            "sidecar" => {
                let sc = sidecar::Entity::find()
                    .filter(sidecar::Column::Uuid.eq(dependency_id))
                    .one(db)
                    .await?
                    .ok_or_else(|| CoreError::NotFound(
                        format!("Sidecar {} not found", dependency_id)
                    ))?;

                let ci_opt = content_identity::Entity::find()
                    .filter(content_identity::Column::Uuid.eq(sc.content_uuid))
                    .one(db)
                    .await?;

                if let Some(ci) = ci_opt {
                    let entries = entry::Entity::find()
                        .filter(entry::Column::ContentId.eq(ci.id))
                        .all(db)
                        .await?;

                    Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
                } else {
                    Ok(vec![])
                }
            }

            // Media data routes same as content
            "image_media_data" | "video_media_data" | "audio_media_data" => {
                // Similar fan-out logic
                Ok(vec![])
            }

            _ => Ok(vec![])
        }
    }

    // NEW: Constructor already exists, just expose via trait
    async fn from_ids(db: &DatabaseConnection, ids: &[Uuid]) -> Result<Vec<Self>> {
        File::from_entry_uuids(db, ids).await
    }

    fn alternate_ids(&self) -> Vec<Uuid> {
        if let Some(content) = &self.content_identity {
            vec![content.uuid]
        } else {
            vec![]
        }
    }

    fn no_merge_fields() -> &'static [&'static str] {
        &["sd_path"]
    }
}
```

**Example: SpaceLayout**

```rust
impl Identifiable for SpaceLayout {
    fn id(&self) -> Uuid {
        self.id
    }

    fn resource_type() -> &'static str {
        "space_layout"
    }

    fn sync_dependencies() -> &'static [&'static str] {
        &["space", "space_group", "space_item"]
    }

    // NEW: Pattern 3 routing - hierarchical rollup
    async fn route_from_dependency(
        db: &DatabaseConnection,
        dependency_type: &str,
        dependency_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        use crate::infra::db::entities::{space, space_group, space_item};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let space_id = match dependency_type {
            "space" => dependency_id,

            "space_group" => {
                if let Some(group) = space_group::Entity::find()
                    .filter(space_group::Column::Uuid.eq(dependency_id))
                    .one(db)
                    .await?
                {
                    space::Entity::find_by_id(group.space_id)
                        .one(db)
                        .await?
                        .map(|s| s.uuid)
                        .unwrap_or(dependency_id)
                } else {
                    dependency_id
                }
            }

            "space_item" => {
                if let Some(item) = space_item::Entity::find()
                    .filter(space_item::Column::Uuid.eq(dependency_id))
                    .one(db)
                    .await?
                {
                    space::Entity::find_by_id(item.space_id)
                        .one(db)
                        .await?
                        .map(|s| s.uuid)
                        .unwrap_or(dependency_id)
                } else {
                    dependency_id
                }
            }

            _ => return Ok(vec![])
        };

        Ok(vec![space_id])
    }

    async fn from_ids(db: &DatabaseConnection, ids: &[Uuid]) -> Result<Vec<Self>> {
        SpaceLayout::from_space_ids(db, ids).await
    }
}
```

#### 3. Static Registry with Macro

```rust
// core/src/domain/resource_registry.rs

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub struct VirtualResourceInfo {
    pub resource_type: &'static str,
    pub dependencies: &'static [&'static str],
    pub router: fn(&DatabaseConnection, &str, Uuid)
        -> Pin<Box<dyn Future<Output = Result<Vec<Uuid>>>>>,
    pub constructor: fn(&DatabaseConnection, &[Uuid])
        -> Pin<Box<dyn Future<Output = Result<Vec<serde_json::Value>>>>>,
}

static VIRTUAL_RESOURCES: Lazy<Vec<VirtualResourceInfo>> = Lazy::new(|| {
    vec![]
});

// Macro for type-safe registration
#[macro_export]
macro_rules! register_virtual_resource {
    ($type:ty) => {
        inventory::submit! {
            VirtualResourceInfo {
                resource_type: <$type>::resource_type(),
                dependencies: <$type>::sync_dependencies(),
                router: |db, dep_type, dep_id| {
                    Box::pin(async move {
                        <$type>::route_from_dependency(db, dep_type, dep_id).await
                    })
                },
                constructor: |db, ids| {
                    Box::pin(async move {
                        let resources = <$type>::from_ids(db, ids).await?;
                        resources.into_iter()
                            .map(|r| serde_json::to_value(&r))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|e| CoreError::Other(anyhow::anyhow!(e)))
                    })
                },
            }
        }
    };
}

// Usage: Register once per virtual resource
register_virtual_resource!(File);
register_virtual_resource!(SpaceLayout);
```

#### 4. Generic Routing Function

```rust
// core/src/domain/resource.rs

pub async fn map_dependency_to_virtual_ids(
    db: &DatabaseConnection,
    dependency_type: &str,
    dependency_id: Uuid,
) -> Result<Vec<(&'static str, Vec<Uuid>)>> {
    let mut results = Vec::new();

    // Iterate over all registered virtual resources
    for info in inventory::iter::<VirtualResourceInfo> {
        // Check if this virtual resource depends on the changed type
        if info.dependencies.contains(&dependency_type) {
            // Call the resource's routing function
            let ids = (info.router)(db, dependency_type, dependency_id).await?;

            if !ids.is_empty() {
                results.push((info.resource_type, ids));
            }
        }
    }

    Ok(results)
}
```

**Before: 226 lines with hardcoded match statements**
**After: 15 lines, fully generic**

#### 5. Generic Resource Manager

```rust
// core/src/domain/resource_manager.rs

pub async fn emit_resource_events(
    &self,
    resource_type: &str,
    resource_ids: Vec<Uuid>,
) -> Result<()> {
    if resource_ids.is_empty() {
        return Ok(());
    }

    // Step 1: Emit direct events for simple resources
    self.emit_direct_events(resource_type, &resource_ids).await?;

    // Step 2: Map to affected virtual resources (now generic!)
    let mut all_virtual_resources = Vec::new();
    for resource_id in &resource_ids {
        let virtual_mappings = map_dependency_to_virtual_ids(
            &self.db,
            resource_type,
            *resource_id
        ).await?;

        all_virtual_resources.extend(virtual_mappings);
    }

    if all_virtual_resources.is_empty() {
        return Ok(());
    }

    // Step 3: Group by virtual resource type
    let mut grouped: HashMap<&str, Vec<Uuid>> = HashMap::new();
    for (vtype, vids) in all_virtual_resources {
        grouped.entry(vtype).or_default().extend(vids);
    }

    // Step 4: Build and emit virtual resources (now generic!)
    for (virtual_type, virtual_ids) in grouped {
        // Find the constructor for this virtual resource type
        let info = inventory::iter::<VirtualResourceInfo>()
            .find(|r| r.resource_type == virtual_type)
            .ok_or_else(|| CoreError::Other(anyhow::anyhow!(
                "Unknown virtual resource type: {}", virtual_type
            )))?;

        // Call the constructor
        let resources = (info.constructor)(&self.db, &virtual_ids).await?;

        if !resources.is_empty() {
            // Build metadata from the first resource's type
            let metadata = Some(ResourceMetadata {
                no_merge_fields: vec![], // TODO: Get from trait
                alternate_ids: vec![],    // TODO: Collect from resources
            });

            self.events.emit(Event::ResourceChangedBatch {
                resource_type: virtual_type.to_string(),
                resources: serde_json::Value::Array(resources),
                metadata,
            });
        }
    }

    Ok(())
}
```

**Before: Hardcoded match for each virtual resource type**
**After: Generic lookup via registry**

---

## Implementation Plan

### Phase 1: Add Trait Methods (Non-Breaking)

1. Add `route_from_dependency()` and `from_ids()` to `Identifiable` trait with default implementations
2. Implement for `File` and `SpaceLayout`
3. Test implementations match current behavior

**Files:**
- `core/src/domain/resource.rs` - Update trait
- `core/src/domain/file.rs` - Implement routing
- `core/src/domain/space.rs` - Implement routing

### Phase 2: Create Registry System

1. Create `core/src/domain/resource_registry.rs`
2. Define `VirtualResourceInfo` struct
3. Create `register_virtual_resource!` macro
4. Register existing virtual resources

**Files:**
- `core/src/domain/resource_registry.rs` - New file
- `core/src/domain/mod.rs` - Export registry

### Phase 3: Migrate Functions to Use Registry

1. Update `map_dependency_to_virtual_ids()` to iterate registry
2. Update `ResourceManager::emit_resource_events()` to use generic constructor dispatch
3. Add tests comparing old vs new behavior

**Files:**
- `core/src/domain/resource.rs` - Refactor routing
- `core/src/domain/resource_manager.rs` - Refactor emission

### Phase 4: Remove Old Code

1. Delete `is_virtual_resource()` function
2. Delete `get_dependencies()` function
3. Remove old match statements
4. Update documentation

**Files:**
- `core/src/domain/resource.rs` - Cleanup
- Documentation updates

---

## Trade-Offs Analysis

### Pros

**Single Registration Point**
- Add virtual resource once with `register_virtual_resource!(MyType)`
- Compiler enforces completeness

**Co-Located Logic**
- Routing logic lives with domain model
- Easier to understand and maintain

**Uses Existing Trait Data**
- `sync_dependencies()` actually gets used
- No more duplication

**Type Safety**
- Macro ensures trait is implemented
- Constructor signatures checked at compile time

**Extensibility**
- Adding new virtual resources is straightforward
- No need to update multiple match statements

### Cons

**Increased Complexity**
- Trait with async methods (requires boxing)
- Registry system adds indirection

**Runtime Dispatch**
- Function pointers instead of direct calls
- Small performance overhead (likely negligible)

**Less Explicit**
- Routing happens via registry iteration, not visible match
- Harder to trace in debugger

**Inventory Dependency**
- Relies on `inventory` crate for static registration
- Can be fragile with dead code elimination

### Alternative: Keep Current Approach

**When to keep current system:**
- Only 2-3 virtual resources total
- Routing logic rarely changes
- Explicit match statements preferred for clarity

**When to adopt new system:**
- 5+ virtual resources planned
- Frequent addition of new resource types
- Want enforcement of complete trait implementation

---

## Migration Safety

### Compatibility

The new system can coexist with the old during migration:

```rust
pub async fn map_dependency_to_virtual_ids(...) -> Result<...> {
    let mut results = Vec::new();

    // New: Check registry
    for info in inventory::iter::<VirtualResourceInfo>() {
        if info.dependencies.contains(&dependency_type) {
            let ids = (info.router)(db, dependency_type, dependency_id).await?;
            if !ids.is_empty() {
                results.push((info.resource_type, ids));
            }
        }
    }

    // Old: Fallback to match (deprecated)
    if results.is_empty() {
        match dependency_type {
            // Old hardcoded logic as fallback
            _ => {}
        }
    }

    Ok(results)
}
```

This allows incremental migration and A/B testing.

### Testing Strategy

1. **Unit Tests**: Compare outputs of old vs new routing for same inputs
2. **Integration Tests**: Verify end-to-end event emission
3. **Property Tests**: Ensure all registered resources have complete trait impls
4. **Performance Tests**: Measure overhead of registry dispatch

---

## Future Enhancements

### 1. Compile-Time Registry (if possible)

Explore const generics or proc macros to make registry fully compile-time:

```rust
const VIRTUAL_RESOURCES: &[VirtualResourceType] = &[
    VirtualResourceType::new::<File>(),
    VirtualResourceType::new::<SpaceLayout>(),
];
```

### 2. Metadata in Registry

Store `no_merge_fields` and other metadata in registry to avoid per-instance queries:

```rust
pub struct VirtualResourceInfo {
    pub no_merge_fields: &'static [&'static str],
    pub can_have_alternate_ids: bool,
    // ...
}
```

### 3. Dependency Graph Validation

At startup, validate dependency graph has no cycles:

```rust
pub fn validate_dependency_graph() -> Result<()> {
    for resource in inventory::iter::<VirtualResourceInfo>() {
        // Check dependencies don't form cycle
        // Ensure all dependencies are registered
    }
    Ok(())
}
```

### 4. Routing Optimization

Cache routing results for common patterns:

```rust
static ROUTING_CACHE: Lazy<Mutex<HashMap<(String, Uuid), Vec<Uuid>>>> = ...;
```

---

## Recommendation

**Adopt the generic system incrementally:**

1. **Phase 1** (1-2 days): Add trait methods, implement for existing resources
2. **Phase 2** (1 day): Create registry, test in parallel with old system
3. **Phase 3** (1 day): Migrate `map_dependency_to_virtual_ids()` and `ResourceManager`
4. **Phase 4** (1 day): Remove old code, update docs

**Total effort: ~1 week**

**Payoff:**
- Adding new virtual resources: 1 location instead of 4
- Routing logic co-located with domain models
- `sync_dependencies()` actually used
- Foundation for advanced features (graph validation, caching)

The current duplication and scattered logic will become more painful as more virtual resources are added. Better to genericize now while there are only 2 examples.

---

## Open Questions

1. **How to handle metadata extraction generically?**
   - `alternate_ids()` requires instance, not static method
   - Could collect during construction, store in registry

2. **Should simple resources also register?**
   - Currently only virtual resources need routing
   - But could unify interface for all resources

3. **Error handling for missing registrations?**
   - Compile error vs runtime error
   - Startup validation check?

4. **Performance benchmarks needed?**
   - What's acceptable overhead for registry dispatch?
   - Profile hot paths before/after

---

**End of Design Document**
