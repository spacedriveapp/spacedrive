# Architecture Decision Records

## ADR-000: SdPath as Core Abstraction

**Status**: Accepted

**Context**:
- Spacedrive promises a "Virtual Distributed File System"
- Current implementation can't copy files between devices
- Users expect seamless cross-device operations
- Path representations are inconsistent

**Decision**: Every file operation uses `SdPath` - a path that includes device context

**Consequences**:
- Enables true cross-device operations
- Unified API for all file operations
- Makes VDFS promise real
- Natural routing of operations to correct device
- Future-proof for cloud storage integration
- Requires P2P infrastructure for remote operations
- More complex than simple PathBuf

**Example**:
```rust
// This just works across devices
let source = SdPath::new(macbook_id, "/Users/me/file.txt");
let dest = SdPath::new(iphone_id, "/Documents");
copy_files(core, source, dest).await?;
```

---

## ADR-001: Decoupled File Data Model

**Status**: Accepted

**Context**:
- Current model requires content indexing (cas_id) to enable tagging
- Non-indexed files cannot have user metadata
- Content changes can break object associations
- Tags are tied to Objects, not file paths

**Decision**: Separate user metadata from content identity

**Architecture**:
```
Entry (file/dir) → UserMetadata (always exists)
  ↓ (optional)
ContentIdentity (for deduplication)
```

**Consequences**:
- Any file can be tagged immediately
- Metadata persists through content changes
- Progressive enhancement (index when needed)
- Works with ephemeral/non-indexed files
- Cleaner separation of concerns
- More complex data model
- Migration required from v1

---

## ADR-002: SeaORM Instead of Prisma

**Status**: Accepted

**Context**: 
- Prisma's Rust client is abandoned by the Spacedrive team
- The fork is locked to Prisma 4.x while current is 6.x
- Prisma is moving away from Rust support
- Custom sync attributes created tight coupling

**Decision**: Use SeaORM for database access

**Consequences**:
- Active maintenance and community
- Native Rust, no Node.js dependency
- Better async support
- Cleaner migration system
- Need to rewrite all database queries
- Lose Prisma's schema DSL

---

## ADR-002: Unified File Operations

**Status**: Accepted

**Context**:
- Current system has separate implementations for indexed vs ephemeral files
- Users can't perform basic operations across boundaries
- Code duplication for every file operation
- Confusing UX

**Decision**: Single implementation that handles both cases transparently

**Consequences**:
- Consistent user experience
- Half the code to maintain
- Easier to add new operations
- More complex implementation
- Need to handle both cases in one code path

---

## ADR-003: Event-Driven Architecture

**Status**: Accepted

**Context**:
- Current invalidate_query! macro couples backend to frontend
- String-based query keys are error-prone
- Backend shouldn't know about frontend caching

**Decision**: Backend emits domain events, frontend decides what to invalidate

**Consequences**:
- Clean separation of concerns
- Frontend can optimize invalidation
- Type-safe events
- Enables plugin system
- Frontend needs more logic
- Potential for missed invalidations

---

## ADR-004: Pragmatic Monolith

**Status**: Accepted

**Context**:
- Previous attempts to split into crates created "cyclic dependency hell"
- Current crate names (heavy-lifting) are non-descriptive
- Important business logic is hidden

**Decision**: Keep core as monolith with clear module organization

**Consequences**:
- No cyclic dependency issues
- Easier refactoring
- Clear where functionality lives
- Better incremental compilation
- Larger compilation unit
- Can't publish modules separately

---

## ADR-005: GraphQL API with async-graphql

**Status**: Accepted

**Context**:
- rspc was created and abandoned by the Spacedrive team
- Need better API introspection and tooling
- Want to support subscriptions for real-time updates
- Require full type safety from backend to frontend

**Decision**: Use async-graphql for API layer

**Benefits**:
- **Full type safety**: Auto-generated TypeScript types from Rust structs
- **Excellent tooling**: GraphQL Playground, Apollo DevTools, VSCode extensions
- **Built-in subscriptions**: Real-time updates without custom WebSocket code
- **Active community**: Well-maintained with regular updates
- **Standard GraphQL**: Developers already know it
- **Flexible queries**: Clients request exactly what they need
- **Better caching**: Apollo Client handles caching automatically
- Different from current rspc (but better documented)
- Initial setup more complex (but better long-term)

**Type Safety Example**:
```rust
// Rust
#[derive(SimpleObject)]
struct Library {
    id: Uuid,
    name: String,
}
```

```typescript
// Auto-generated TypeScript
export interface Library {
  id: string;
  name: string;
}

// Full type safety in React
const { data } = useGetLibraryQuery({ variables: { id } });
console.log(data.library.name); // Typed!

---

## ADR-006: Single Device Identity

**Status**: Accepted

**Context**:
- Current system has Node, Device, and Instance
- Developers confused about which to use
- Complex identity mapping between systems

**Decision**: Merge into single Device concept

**Consequences**:
- Clear mental model
- Simplified P2P routing
- Easier multi-device features
- Need to migrate existing data
- Breaking change for sync protocol

---

## ADR-007: Third-Party Sync

**Status**: Proposed

**Context**:
- Custom CRDT implementation never shipped
- Mixed local/shared data created unsolvable problems
- Many SQLite sync solutions exist

**Decision**: Use existing sync solution (TBD: Turso, cr-sqlite, etc.)

**Consequences**:
- Proven technology
- Don't maintain sync ourselves
- Can focus on core features
- Less control over sync behavior
- Potential vendor lock-in

---

## ADR-008: Jobs as Simple Functions

**Status**: Proposed

**Context**:
- Current job system requires 500-1000 lines of boilerplate
- Complex trait implementations
- Manual registration in macros

**Decision**: Replace with simple async functions + optional progress reporting

**Consequences**:
- Dramatically less boilerplate
- Easier to understand
- Can use standard Rust patterns
- Lose automatic serialization/resume
- Need different approach for long-running tasks