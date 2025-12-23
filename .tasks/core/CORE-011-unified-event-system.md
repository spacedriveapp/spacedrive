---
id: CORE-011
title: Unified Resource Event System
status: Done
parent: CORE-000
assignee: jamiepine
priority: High
tags: [core, events, architecture, refactor]
---

## Description

Refactor the event system from 40+ specialized event variants to a unified generic resource event architecture. This eliminates boilerplate and enables horizontal scaling - adding new resources requires zero event handling code changes.

## Current Problem

- 40+ event variants in `core/src/infra/event/mod.rs`
- Manual event emission scattered across codebase (easy to forget)
- Adding new resource = new event variant + client code changes
- No type safety between events and resources

## Solution

- Generic `ResourceChanged`, `ResourceBatchChanged`, `ResourceDeleted` events
- TransactionManager emits automatically (no manual emission)
- Client type registries handle deserialization generically
- Infrastructure events remain specific (CoreStarted, Job, etc.)

## Implementation Steps

1. Define new `Event` struct with `EventEnvelope` and `EventKind`
2. Add `ResourceChanged` and related variants to `EventKind`
3. Update TransactionManager to emit resource events automatically
4. Keep infrastructure events as specific variants
5. Mark old event variants as `#[deprecated]`
6. Migrate Albums/Tags/Locations to new events (parallel systems)
7. Remove old variants after full migration

## Acceptance Criteria

- [ ] New event structure defined
- [ ] EventEnvelope includes id, timestamp, library_id, sequence
- [ ] ResourceChanged auto-emitted by TransactionManager
- [ ] Infrastructure events (Job, CoreStarted) preserved
- [ ] No breaking changes (parallel systems initially)
- [ ] Documentation updated

## Migration Strategy

- Phase 1: Additive (both old and new events)
- Phase 2: Parallel (new resources use unified events only)
- Phase 3: Deprecation (mark old events deprecated)
- Phase 4: Cleanup (remove old events)

## References

- `docs/core/events.md` - Complete specification
- Current: `core/src/infra/event/mod.rs`
