# Sync System Design Documentation

This directory contains **detailed design documents** for Spacedrive's multi-device synchronization and client-side caching architecture.

## Implementation Guides (Start Here!)

For implementation, read these **root-level guides**:

1. **[../../sync.md](../../sync.md)** **Sync System Implementation Guide**
   - TransactionManager API and usage
   - Syncable trait specification
   - Leader election protocol
   - Sync service implementation
   - Production-ready reference

2. **[../../events.md](../../events.md)** **Unified Event System**
   - Generic resource events
   - Type registry pattern (zero switch statements!)
   - Client integration (Swift + TypeScript)
   - Migration strategy

3. **[../../normalized_cache.md](../../normalized_cache.md)** **Client-Side Normalized Cache**
   - Cache architecture and implementation
   - Memory management (LRU, TTL, ref counting)
   - React and SwiftUI integration
   - Optimistic updates and offline support

---

## Design Documents (Deep Dives)

The documents in this directory provide comprehensive design rationale and detailed exploration. Read these for context and decision history:

### 1. Foundation & Context
- **[SYNC_DESIGN.md](./SYNC_DESIGN.md)** - The original comprehensive sync architecture
  - Covers: Sync domains (Index, Metadata, Content, State), conflict resolution, leader election
  - Start here for foundational understanding

### 2. Core Implementation Specs
- **[SYNC_TX_CACHE_MINI_SPEC.md](./SYNC_TX_CACHE_MINI_SPEC.md)** **START HERE FOR IMPLEMENTATION**
  - Concise, actionable spec for `Syncable`/`Identifiable` traits
  - TransactionManager API and semantics
  - BulkChangeSet mechanism for efficient bulk operations
  - Albums example with minimal boilerplate
  - Raw SQL compatibility notes

- **[UNIFIED_RESOURCE_EVENTS.md](./UNIFIED_RESOURCE_EVENTS.md)** **CRITICAL FOR EVENT SYSTEM**
  - Generic resource event design (eliminates ~40 specialized event variants)
  - Type registry pattern for zero-friction horizontal scaling
  - Swift and TypeScript examples with auto-generation via specta
  - **Key insight**: Zero switch statements when adding new resources

### 3. Unified Architecture
- **[UNIFIED_TRANSACTIONAL_SYNC_AND_CACHE.md](./UNIFIED_TRANSACTIONAL_SYNC_AND_CACHE.md)**
  - Complete end-to-end architecture integrating sync + cache
  - Context-aware commits: `transactional` vs `bulk` vs `silent`
  - **Critical**: Bulk operations create ONE metadata sync entry (not millions)
  - Performance analysis and decision rationale
  - 2295 lines of comprehensive design (reference doc, not reading material)

### 4. Client-Side Caching
- **[NORMALIZED_CACHE_DESIGN.md](./NORMALIZED_CACHE_DESIGN.md)**
  - Client-side normalized entity cache (similar to Apollo Client)
  - Event-driven invalidation and atomic updates
  - Memory management (LRU, TTL, reference counting)
  - Swift and TypeScript implementation patterns
  - 2674 lines covering edge cases and advanced scenarios

### 5. Implementation Analysis
- **[TRANSACTION_MANAGER_COMPATIBILITY.md](./TRANSACTION_MANAGER_COMPATIBILITY.md)**
  - Compatibility analysis with existing codebase
  - Current write patterns (SeaORM, transactions, raw SQL)
  - Migration strategy with code examples
  - Risk analysis and mitigation
  - **Verdict**: Fully compatible, ready to implement 

### 6. Historical & Supplementary
- **[SYNC_DESIGN_2025_08_19.md](./SYNC_DESIGN_2025_08_19.md)** - Updated sync design iteration
- **[SYNC_FIRST_DRAFT_DESIGN.md](./SYNC_FIRST_DRAFT_DESIGN.md)** - Early draft (historical context)
- **[SYNC_INTEGRATION_NOTES.md](./SYNC_INTEGRATION_NOTES.md)** - Integration notes and considerations
- **[SYNC_CONDUIT_DESIGN.md](./SYNC_CONDUIT_DESIGN.md)** - Sync conduit specific design

---

## Quick Reference

### Key Concepts

**Syncable** (Rust persistence models)
```rust
pub trait Syncable {
    const SYNC_MODEL: &'static str;
    fn sync_id(&self) -> Uuid;
    fn version(&self) -> i64;
}
```

**Identifiable** (Client-facing resources)
```rust
pub trait Identifiable {
    type Id;
    fn resource_id(&self) -> Self::Id;
    fn resource_type() -> &'static str;
}
```

**TransactionManager** (Sole write gateway)
- `commit()` - Single resource, per-entry sync log
- `commit_batch()` - Micro-batch (10-1K), per-entry sync logs
- `commit_bulk()` - Bulk (1K+), ONE metadata sync entry

**Event System** (Generic, horizontally scalable)
- `ResourceChanged { resource_type, resource }`
- `ResourceBatchChanged { resource_type, resources }`
- `BulkOperationCompleted { resource_type, affected_count, hints }`

### Critical Design Decisions

1. **Indexing ≠ Sync**: Each device indexes its own filesystem. Bulk operations create metadata notifications, not individual entry replications.

2. **Leader Election**: One device per library assigns sync log sequence numbers. Prevents collisions.

3. **Zero Manual Sync Logging**: TransactionManager automatically creates sync logs. Application code never touches sync infrastructure.

4. **Type Registry Pattern**: Clients use type registries (auto-generated via specta) to handle all resource events generically. No switch statements per resource type.

5. **Client-Side Cache**: Normalized entity store + query index. Events trigger atomic updates. Cache persistence for offline mode.

---

## Implementation Status

- [x] Design documentation complete
- [ ] Phase 1: Core infrastructure (TM, traits, events)
- [ ] Phase 2: Client prototype (Swift cache + event handler)
- [ ] Phase 3: Expansion (migrate all ops to TM)
- [ ] Phase 4: TypeScript port + advanced features

---

## Related Documentation

**Implementation Guides** (Root Level):
- `../../sync.md` - Sync system implementation
- `../../events.md` - Unified event system
- `../../normalized_cache.md` - Client cache implementation
- `../../sync-setup.md` - Library sync setup (Phase 1)

**Infrastructure**:
- `../INFRA_LAYER_SEPARATION.md` - Infrastructure layer architecture
- `../JOB_SYSTEM_DESIGN.md` - Job system (indexing jobs integrate with TM)
- `../DEVICE_PAIRING_PROTOCOL.md` - Device pairing (prerequisite for sync)

---

## Documentation Philosophy

**Root-level docs** (`docs/core/*.md`):
- Implementation-ready guides
- Concise, actionable specifications
- Code examples and usage patterns
- Reference during development

**Design docs** (`docs/core/design/sync/*.md`):
- Comprehensive exploration
- Decision rationale and alternatives
- Edge cases and advanced scenarios
- Historical context

---

## Contributing

**Adding implementation guidance**: Update root-level docs (`sync.md`, `events.md`, `normalized_cache.md`)

**Adding design exploration**: Create new document in this directory:
1. Follow naming: `SYNC_<TOPIC>_DESIGN.md`
2. Update this README
3. Reference related documents
4. Include comprehensive examples
