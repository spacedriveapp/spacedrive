# Spacedrive Normalized Cache Investigation - Summary

## What I Found

A comprehensive analysis of Spacedrive's normalized cache system revealing the gap between intended design and current implementation.

## Key Findings

### 1. **The Identifiable Pattern Exists But Is Incomplete**

- Trait is well-designed: `fn id()`, `fn resource_type()`, `fn sync_dependencies()`
- Location, Space, SpaceGroup, SpaceItem implement it correctly
- **File doesn't implement it** (the most important resource type!)
- Tag, Device, ContentIdentity also missing
- **Impact:** Forces workarounds throughout the caching layer

### 2. **Virtual Resource Problem**

File is computed from multiple tables (Entry + ContentIdentity + Sidecar), creating complexity:

```
Database:                  Frontend needs to show:
entry + content_identity   → File (with both pieces merged)
entry + sidecar            → File (with sidecar data)
content_identity change    → Which Files are affected?
```

Current solution: Hardcoded mapping in `map_dependency_to_virtual_ids()`
Better solution: Trait-based registry with `ResourceMapper` types

### 3. **The deepMerge Function Has 4 Major Hacks**

```typescript
// Hack #1: sd_path special case (lines 25-28)
if (key === 'sd_path') continue;  // Never merge, always replace

// Hack #2: content_identity UUID matching (lines 206-215, 335-345)
if (oldData.content_identity?.uuid === resource.content_identity?.uuid) {
  // Match by content, not file ID - this is a smell!
}

// Hack #3: Single object detection (lines 304-351)
const isSingleResource = !!(oldData).id && !!(oldData).sd_path;
// Detects if response is a File vs wrapped array

// Hack #4: Resource filter logic (lines 159-188, 254-299)
// Determines which items belong in this query scope
```

**Why these exist:**
- File has multiple identifiers (id + content_uuid)
- sd_path changes without changing File ID
- Sidecar events affect Files, not stored directly
- Need to filter items by query context

**What it reveals:**
- Original design assumed simple resources with stable IDs
- File violates these assumptions (virtual, multi-identifier)
- Frontend has to know about File's internal structure

### 4. **Multiple Paths to Same Resource**

File can be identified by:
1. Entry UUID: "file123"
2. Content UUID: "content-abc" (for deduplication)
3. Physical path: "/Users/me/file.txt"
4. Content path: "content://content-abc"

Cache update logic must handle this complexity:
- Line 277-294: Update by content UUID when sd_path is Content-based
- Line 335-345: Same logic for batch updates
- Current workaround: Check multiple fields

### 5. **How Apollo & Relay Handle This**

**Apollo Client:**
- Uses `__typename + id` as cache key
- Configurable via `keyFields`
- Automatic deduplication
- No special case logic in merge

**Relay Modern:**
- Uses `__typename + id` as record ID
- Stores references between records
- Update handlers for custom logic
- Also no special case in core merge

**Spacedrive's approach:**
- Very similar architecture!
- Has the trait pattern
- But incomplete implementation created special cases

### 6. **Why It Evolved This Way**

1. **Original vision:** Generic normalized cache, all types implement Identifiable
2. **Reality hit:** File is more complex than expected (virtual resource)
3. **Pragmatic fix:** Hardcode File handling in ResourceManager
4. **Side effect:** Frontend deepMerge gets special cases for File's quirks
5. **Technical debt:** System works but isn't generic anymore

## What the Ideal State Would Be

```
Clean tier:
├─ All resources implement Identifiable
├─ sync_dependencies tells system what affects what
├─ ResourceManager emits events with correct ID
├─ Frontend uses generic deepMerge
└─ No special cases anywhere

Key insight:
If File always sends full, correct data (all fields, latest sd_path),
deepMerge doesn't need to know it's special
```

## The 5-Phase Fix

1. **Week 1:** Implement File::Identifiable, remove sd_path special case
2. **Week 2:** Extract virtual resource logic into ResourceMapper trait
3. **Week 3-4:** Implement Tag, Device, ContentIdentity
4. **Week 5:** Add merge strategy metadata (replace/merge/custom)
5. **Week 6:** Optimize cache structure (ID-keyed instead of array-in-array)

## Technical Debt Summary

| Issue | Current | Ideal | Effort |
|-------|---------|-------|--------|
| File missing Identifiable | 4 hacks in deepMerge | 1 trait impl | 1 day |
| Virtual resource mapping | Hardcoded match | Trait registry | 2 days |
| Multiple identifiers | Check id + content_uuid | Use Entry UUID only | 1 day |
| sd_path handling | Special case skip | Normal field merge | 1 day |
| Missing trait impls | 4 types | 8 types | 3 days |
| Merge strategy | Hardcoded deep merge | Metadata-driven | 2 days |
| **Total** | | | **2 weeks** |

## Files to Read

**Backend:**
- `/core/src/domain/resource.rs` - Identifiable trait definition
- `/core/src/domain/resource_manager.rs` - Virtual resource mapping
- `/core/src/domain/file.rs` - File model (needs Identifiable impl)
- `/core/src/infra/event/mod.rs` - Event definitions

**Frontend:**
- `/packages/ts-client/src/hooks/useNormalizedCache.ts` - Cache sync hook (deepMerge is here)
- `/packages/interface/src/LocationCacheDemo.tsx` - Example usage

## Key Insights

1. **The system was well-designed** - Trait pattern is solid, just incomplete
2. **Special cases leak down** - Backend virtual resource complexity → frontend hacks
3. **File needs to change** - It's the root cause (most important + most complex)
4. **The merge strategy is right** - Just needs to be metadata-driven
5. **Other systems faced this** - Apollo/Relay solved it with configurable key fields

## Recommendation

Start with **File::impl Identifiable** + **remove deepMerge sd_path hack**

Why:
- Unblocks everything else
- File is highest-value target
- Simplest special case to remove
- Most important resource type
- Will expose what else needs fixing

This is a well-intentioned system that hit complexity head-on. The fix is systematic, not architectural.

