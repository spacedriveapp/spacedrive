---
id: CORE-017
title: Optimistic Updates for Client Cache
status: To Do
assignee: unassigned
parent: CORE-015
priority: Medium
tags: [client, cache, ux, optimistic]
depends_on: [CORE-015, CORE-016]
---

## Description

Implement optimistic updates in the normalized cache, allowing instant UI feedback before server confirmation. If the action fails, the update is rolled back automatically.

## Implementation Steps

1. Add `optimisticUpdates` map to cache (pending_id → resource)
2. Implement `updateOptimistically()` - applies change immediately
3. Implement `commitOptimisticUpdate()` - replaces with confirmed data
4. Implement `rollbackOptimisticUpdate()` - reverts on error
5. Integrate with action execution flow
6. Add visual indicators for pending changes (optional)

## Flow Example

```typescript
// 1. Optimistic update (instant UI)
const pendingId = uuid();
await cache.updateOptimistically(pendingId, {
  id: albumId,
  name: newName,
  ...optimisticAlbum
});

try {
  // 2. Send action to server
  const confirmed = await client.action('albums.rename.v1', { id: albumId, name: newName });

  // 3. Commit (replace optimistic with confirmed)
  await cache.commitOptimisticUpdate(pendingId, confirmed);
} catch (error) {
  // 4. Rollback on error
  await cache.rollbackOptimisticUpdate(pendingId);
  throw error;
}
```

## Technical Details

- Optimistic updates stored separately from confirmed entities
- UI sees merged view (optimistic + confirmed)
- Pending changes visually indicated (future)
- Automatic rollback on action failure

## Acceptance Criteria

- [ ] Optimistic update API implemented
- [ ] UI updates instantly before server response
- [ ] Rollback works on errors
- [ ] No flickering during commit
- [ ] Unit tests for optimistic flow
- [ ] Integration tests validate error scenarios

## References

- `docs/core/normalized_cache.md` lines 685-741
