---
id: CORE-015
title: Normalized Client Cache (Swift)
status: To Do
assignee: james
priority: High
tags: [client, swift, cache, performance]
depends_on: [CORE-012]
---

## Description

Implement the normalized client cache for iOS/macOS apps. Provides instant UI updates, offline support, and massive bandwidth savings by normalizing all resources by ID and updating atomically when events arrive.

## Implementation Steps

1. Create `NormalizedCache` actor with two-level structure:
   - Level 1: Entity store (normalized by ID)
   - Level 2: Query index (maps queries to entity IDs)
2. Implement `updateEntity<T>()` - updates entity and notifies observers
3. Implement `query<T>()` - caches queries and results
4. Implement `deleteEntity()` - removes entity and updates indices
5. Implement `invalidateQueriesForResource()` - bulk operation handling
6. Add LRU eviction (max 10K entities)
7. Add SQLite persistence for offline support
8. Create `EventCacheUpdater` for event integration

## Cache Architecture

```
┌─────────────────────────────────────────┐
│ Entity Store (Level 1)                  │
│  "file:uuid-1" → File { ... }           │
│  "album:uuid-2" → Album { ... }         │
└─────────────────────────────────────────┘
                ↑
                │ Atomic updates
                │
┌─────────────────────────────────────────┐
│ Query Index (Level 2)                   │
│  "search:photos" → ["file:uuid-1", ...] │
│  "albums.list" → ["album:uuid-2"]       │
└─────────────────────────────────────────┘
```

## Technical Details

- Location: `packages/client-swift/Sources/SpacedriveCore/Cache/NormalizedCache.swift`
- Actor for thread-safety
- Max entities: 10,000 (configurable)
- TTL: 5 minutes default (query-specific)
- Persistence: SQLite in app cache directory

## Acceptance Criteria

- [ ] NormalizedCache actor implemented
- [ ] Entity store with LRU eviction
- [ ] Query index with TTL
- [ ] SQLite persistence
- [ ] EventCacheUpdater integration
- [ ] ObservableObject wrapper for SwiftUI
- [ ] Memory stays under 15MB with 10K entities
- [ ] Unit tests for cache operations
- [ ] Integration tests with events

## References

- `docs/core/normalized_cache.md` - Complete specification
