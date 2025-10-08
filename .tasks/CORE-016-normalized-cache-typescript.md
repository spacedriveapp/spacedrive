---
id: CORE-016
title: Normalized Client Cache (TypeScript)
status: To Do
assignee: unassigned
priority: High
tags: [client, typescript, react, cache, performance]
depends_on: [CORE-013]
---

## Description

Implement the normalized client cache for web/desktop (Electron) apps. Same architecture as Swift version but with React integration via hooks.

## Implementation Steps

1. Create `NormalizedCache` class with entity store + query index
2. Implement `updateEntity()` with subscription notifications
3. Implement `query()` with caching
4. Implement `deleteEntity()` and query invalidation
5. Add LRU eviction
6. Add IndexedDB persistence for offline support
7. Create `useCachedQuery` React hook
8. Create `EventCacheUpdater` for event integration

## React Integration

```typescript
function useCachedQuery<T>(
  method: string,
  input: any,
): { data: T[] | null; loading: boolean; error: Error | null } {
  const cache = useContext(CacheContext);
  const [data, setData] = useState<T[] | null>(null);

  useEffect(() => {
    const queryKey = cache.generateQueryKey(method, input);

    // Subscribe to cache changes
    const unsubscribe = cache.subscribe(queryKey, () => {
      const result = cache.getQueryResult<T>(queryKey);
      setData(result);
    });

    // Initial fetch
    cache.query<T>(method, input).then(setData);

    return unsubscribe;
  }, [method, JSON.stringify(input)]);

  return { data, loading: data === null, error: null };
}
```

## Technical Details

- Location: `packages/client/src/core/NormalizedCache.ts`
- React hook: `packages/client/src/hooks/useCachedQuery.ts`
- Max entities: 10,000
- TTL: 5 minutes default
- Persistence: IndexedDB

## Acceptance Criteria

- [ ] NormalizedCache class implemented
- [ ] Entity store with LRU eviction
- [ ] Query index with TTL
- [ ] IndexedDB persistence
- [ ] useCachedQuery hook
- [ ] EventCacheUpdater integration
- [ ] Memory stays under 15MB
- [ ] Unit tests for cache operations
- [ ] Integration tests with React components

## References

- `docs/core/normalized_cache.md` lines 188-279
