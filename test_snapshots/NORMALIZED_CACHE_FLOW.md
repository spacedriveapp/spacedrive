# Normalized Cache Flow Analysis

## Current Implementation Flow

### 1. Initial Query Load
```typescript
// User navigates to /Downloads
const directoryQuery = useNormalizedCache({
  wireMethod: "query:files.directory_listing",
  input: { path: "/Downloads", ... },
  resourceType: "file",
  resourceFilter: (file) => { /* content_id matching logic */ }
})
```

**Result:** Query returns 30 files with:
- `id` = entry.uuid (e.g., `"a72dfd9e-1908-..."`)
- `sd_path` = Physical path
- `content_identity.uuid` = content UUID

**TanStack Query cache now contains:**
```json
{
  "files": [
    {
      "id": "a72dfd9e-1908-...",
      "name": "document.pdf",
      "content_identity": { "uuid": "xyz123..." }
    },
    // ... 29 more files
  ]
}
```

---

### 2. Indexing Starts - Events Arrive

**Event arrives:**
```json
ResourceChangedBatch {
  resource_type: "file",
  resources: [
    {
      "id": "different-uuid-not-in-downloads",
      "sd_path": { "Content": { "content_id": "abc456..." } },
      "content_identity": { "uuid": "abc456..." }
    },
    // ... 99 more files from various directories
  ]
}
```

---

### 3. Event Processing (lines 194-322)

```typescript
queryClient.setQueryData((oldData) => {
  // oldData = { files: [30 items from /Downloads] }
  
  const resourceMap = new Map(resources.map(r => [r.id, r]));
  // Map has 100 items with IDs from the event
  
  const array = [...oldData.files]; // 30 items
  
  // STEP A: Try to update existing items (lines 275-296)
  for (let i = 0; i < 30; i++) {
    const item = array[i]; // File from /Downloads
    
    if (resourceMap.has(item.id)) {
      // Does event batch contain THIS file's ID?
      // Usually NO - event has files from other directories
      // So this rarely executes
    }
  }
  
  // Result: updateCount = 0 (no matches)
  
  // STEP B: Try to append new items (lines 309-315)
  if (resourceFilter) {
    for (const resource of resources) { // 100 event files
      if (!seenIds.has(resource.id) && resourceFilter(resource)) {
        // resourceFilter checks: is this file's content_id in oldData?
        // Problem: resourceFilter accesses directoryQuery.data
        // But we're INSIDE the setQueryData callback!
        // directoryQuery.data might be stale!
        array.push(resource);
      }
    }
  }
  
  return { files: array };
});
```

---

## THE BUG

**Line 116 in context.tsx:**
```typescript
const currentFiles = directoryQuery.data?.files || [];
```

This creates a **closure problem**:
1. resourceFilter is defined with `directoryQuery.data` reference
2. When event arrives, resourceFilter runs INSIDE `setQueryData`
3. But `directoryQuery.data` still has the OLD data at this point
4. React Query hasn't updated the hook's `data` property yet
5. So resourceFilter is comparing against stale data!

**Even worse:** The resourceFilter runs for EVERY batch (100 files × many batches).
Each time it checks if 100 files match the stale `directoryQuery.data`.

---

## Why Content Identity Disappears

Looking at your logs:
- "Updated 100 items" sometimes
- "Updated 0 items" other times

When "Updated 100 items":
- Those 100 event files happened to have IDs matching files in /Downloads
- `mergeWithoutNulls()` runs
- BUT: `mergeWithoutNulls()` does shallow merge!
- Line 14: `const merged = { ...incoming }`
- This REPLACES the entire object with incoming data
- Only top-level null fields get preserved

**The Problem:**
If incoming file has `content_identity: { uuid: "xyz", content_hash: "abc" }`
And existing has `content_identity: { uuid: "xyz", content_hash: "abc", extra_field: "value" }`

The merge does:
```javascript
merged = { ...incoming } // Start with incoming
// Then fix top-level nulls
if (incoming.content_identity === null && existing.content_identity !== null) {
  merged.content_identity = existing.content_identity
}
```

But if BOTH have content_identity (not null), no preservation happens!
The incoming object REPLACES the existing one entirely.

---

## Summary

1. **resourceFilter** has stale data closure issue
2. **mergeWithoutNulls** only preserves null→value, not value→different-value
3. **No filtering works** because paths don't match (Content vs Physical)
4. Every batch processes all 100 files, checking stale data
5. When IDs match by chance, merge doesn't preserve nested fields properly

