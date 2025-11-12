# Spacedrive Normalized Cache - Quick Reference

## The Problem in One Sentence

**File resource doesn't implement Identifiable trait, forcing 4 hacks in the frontend deepMerge function.**

## The Three Key Files

### Backend
- **`core/src/domain/resource.rs`** - Identifiable trait (good design, incomplete)
- **`core/src/domain/resource_manager.rs`** - Virtual resource mapping (hardcoded)
- **`core/src/domain/file.rs`** - File model (missing Identifiable impl)

### Frontend
- **`packages/ts-client/src/hooks/useNormalizedCache.ts`** - Cache sync hook (has 4 hacks)

## The 4 Hacks in deepMerge

1. **Line 25-28:** `if (key === 'sd_path') continue;` — Never merge sd_path
2. **Line 206-215:** Match by `content_identity.uuid` instead of id
3. **Line 277-294:** Repeat hack #2 for batch updates
4. **Line 304-351:** Detect single File vs wrapped array response

## Why They Exist

| Hack | Why | Real Cause |
|------|-----|-----------|
| #1 sd_path skip | Path changes without File ID changing | ResourceManager should send current path |
| #2 content UUID match | Multiple Files can share content | Should be File ID only |
| #3 batch duplicate | Hack #2 needed for batch too | Same root cause |
| #4 single detection | Hook doesn't know response format | API should normalize responses |

## The 2-Week Fix

**Phase 1 (1 day):** Implement `File::Identifiable`
```rust
impl Identifiable for File {
    fn id(&self) -> Uuid { self.id }
    fn resource_type() -> &'static str { "file" }
    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar"]
    }
}
```

**Phase 2 (1 day):** Remove deepMerge sd_path hack
```typescript
// Delete lines 25-28
// ResourceManager must ensure File has current sd_path
```

**Phase 3 (2 days):** Remove content UUID matching hacks
```typescript
// Delete lines 206-215, 277-294, 335-345, 392-408
// ResourceManager should emit per-File, not per-content
```

**Phase 4 (1 day):** Fix single resource detection
```typescript
// Delete lines 304-351
// Hook should know response type from metadata
```

**Phase 5 (1 week):** Implement remaining traits
- Tag (complex because of relationships)
- Device (simpler)
- ContentIdentity (or keep as dependency-only)

## Current Architecture vs Ideal

### Current (Broken)
```
Database change
  ↓
ResourceManager detects virtual resource mapping (hard-coded)
  ↓
Emits ResourceChanged event
  ↓
Frontend useNormalizedCache receives event
  ↓
deepMerge applies 4 special cases to merge data
  ↓
TanStack Query cache updated
  ↓
Component re-renders
```

### Ideal (Clean)
```
Database change
  ↓
ResourceManager detects via sync_dependencies() trait
  ↓
Emits ResourceChanged with correct ID and full data
  ↓
Frontend useNormalizedCache receives event
  ↓
deepMerge applies generic merge (no special cases)
  ↓
TanStack Query cache updated
  ↓
Component re-renders
```

## How Apollo/Relay Handle This

Both use **configured identity fields** per type:
- Apollo: `keyFields` option (configurable)
- Relay: `__typename + id` pattern (fixed)

Spacedrive uses:
- `resource_type` (equivalent to `__typename`)
- `id` field (equivalent to id)

But incomplete implementation in Rust caused special cases to leak to frontend.

## The Root Cause Analysis

1. **Intended design:** Generic trait-based, all types implement Identifiable
2. **Reality hit:** File is virtual (Entry + ContentIdentity + Sidecar)
3. **Pragmatic fix:** Hardcode File in ResourceManager
4. **Side effect:** Frontend doesn't know File is special, adds hacks
5. **Result:** System works but isn't generic anymore

## What to Do Now

### If you have 1 hour
Read `/Users/jamespine/Projects/spacedrive/CACHE_INVESTIGATION_SUMMARY.md`

### If you have 2 hours
Read `/Users/jamespine/Projects/spacedrive/NORMALIZED_CACHE_ANALYSIS.md`

### If you have 4 hours
Read all 3 documents + look at code locations in `CACHE_CODE_REFERENCES.md`

### If you're implementing the fix
1. Read NORMALIZED_CACHE_ANALYSIS.md Part 6 (migration path)
2. Start with Phase 1 (File::Identifiable)
3. Use CACHE_CODE_REFERENCES.md for line numbers
4. Run existing tests to verify nothing breaks
5. Add tests for each hack removal

## Key Insight

This isn't an architectural problem — it's an implementation problem.

The **architecture is sound** (trait-based normalized cache like Apollo/Relay).
The **implementation is incomplete** (File doesn't implement the trait).

Finishing the implementation removes all special cases and makes the system generic again.

## Quick Stats

- **Total lines of code:** ~466 in useNormalizedCache.ts
- **Lines with special cases:** ~100+ (hacks #1-4)
- **Complexity added by File:** ~40% of hook logic
- **Estimated cleanup effort:** 2 weeks (including tests)
- **Expected result:** 50% reduction in cache hook complexity

