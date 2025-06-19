# Spacedrive Core v2

A ground-up reimplementation of Spacedrive's core, addressing architectural issues and technical debt identified in the original implementation.

## Why a Rewrite?

The original core suffered from:
- **Dual systems**: Separate implementations for indexed vs ephemeral files
- **Prisma lock-in**: Heavily coupled to a deprecated Rust fork
- **Scattered logic**: Core functionality hidden in non-obvious places
- **Incomplete migrations**: Old and new systems running in parallel
- **Over-engineering**: Complex abstractions that hindered development

## Key Improvements

### 1. SdPath - The Foundation of VDFS
The most important innovation: `SdPath` enables true virtual distributed file system operations.

```rust
// Copy files from your MacBook to your iPhone
let source = SdPath::new(macbook_id, "/Users/me/photo.jpg");
let dest = SdPath::new(iphone_id, "/Documents");
copy_files(core, sources, dest).await?;

// It Just Works™ - P2P complexity is handled transparently
```

Every file operation uses `SdPath`, enabling:
- Cross-device copy/move/delete
- Unified API regardless of file location  
- True VDFS - files are just paths, device is a detail

### 2. SeaORM Instead of Prisma
- Modern, actively maintained Rust ORM
- No dependency on abandoned forks
- Better async support
- Cleaner migration system

### 3. Unified Architecture
- **Single file system**: One implementation handles both indexed and ephemeral files
- **Single identity**: No more node/device/instance confusion
- **Single job system**: If needed at all - consider simpler alternatives

### 4. Event-Driven Design
- No more `invalidate_query!` coupling frontend and backend
- Clean event bus for state changes
- Frontend decides what to invalidate

### 5. Modern GraphQL API
- **Full Type Safety**: From Rust structs to TypeScript interfaces
- **Better Developer Experience**: GraphQL Playground, auto-complete, tooling
- **Flexible Queries**: Clients request exactly what they need
- **Real-time Updates**: Built-in subscriptions for live data

### 6. Pragmatic Organization
```
src/
├── domain/           # Core business entities
├── operations/       # What Spacedrive actually does
├── infrastructure/   # External interfaces
└── shared/          # Common types and utilities (including SdPath!)
```

## Architecture Principles

### Domain Layer (`/domain`)
Core business entities that represent Spacedrive's model of the world:
- **Library**: A collection of locations across devices
- **Location**: A folder being tracked by Spacedrive
- **Object**: A unique file with metadata (the same file in multiple locations)
- **Device**: A machine running Spacedrive (unified concept)

### Operations Layer (`/operations`)
Business operations that users care about:
- **File Operations**: Copy, move, delete, rename - the core value proposition
- **Indexing**: Scanning and tracking files
- **Search**: Lightning-fast file finding (with proper FTS and future vector search)
- **Media Processing**: Thumbnail generation and metadata extraction
- **Sync**: Multi-device file metadata synchronization

### Infrastructure Layer (`/infrastructure`)
How the core interacts with the outside world:
- **API**: GraphQL with async-graphql for full type safety
- **Database**: SeaORM entities and migrations
- **Events**: Event bus for state changes
- **Jobs**: Simple task queue if needed (or just async tasks)

### Shared Layer (`/shared`)
Common code used across layers:
- **Errors**: Unified error handling
- **Types**: Shared type definitions
- **Utils**: Common utilities

## Implementation Strategy

### Phase 1: Foundation (Week 1-2)
- [ ] Set up SeaORM with initial schema
- [ ] Create domain entities
- [ ] Implement event bus
- [ ] Basic file operations (copy, move, delete)

### Phase 2: Core Features (Week 3-4)
- [ ] Unified file management (indexed + ephemeral)
- [ ] Location watching and indexing
- [ ] Basic search with SQLite FTS5
- [ ] Media processing pipeline

### Phase 3: Advanced Features (Week 5-6)
- [ ] Cloud sync abstraction (using third-party solution)
- [ ] P2P communication layer
- [ ] Advanced search features
- [ ] Performance optimizations

## Key Decisions

### 1. No More Dual Systems
Every file operation works the same way whether the file is indexed or not. The indexing status only affects what metadata we have available.

### 2. Database Schema Clarity
- Clear separation between local and syncable data
- No nullable fields that "aren't actually nullable"
- Proper use of SeaORM relations

### 3. Simple Job System
Instead of complex job traits with 500+ lines of boilerplate:
```rust
pub async fn copy_files(paths: Vec<PathBuf>, destination: PathBuf) -> Result<()> {
    // Just async functions with progress reporting
}
```

### 4. Search as a First-Class Citizen
- SQLite FTS5 from day one
- Prepared for vector search
- Unified search across all file types

## Migration Path

1. **Run in parallel**: `core-new` alongside existing core
2. **Port operations**: One at a time, starting with file operations
3. **Switch frontends**: Update UI to use new API gradually
4. **Deprecate old core**: Once feature parity achieved

## Development Guidelines

1. **Simplicity first**: Can it be a simple function instead of a trait?
2. **User-focused**: Directory structure reflects user operations
3. **Type safety**: Leverage Rust's type system, but pragmatically
4. **Progressive enhancement**: Start simple, add complexity only when needed

## Getting Started

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Start the CLI daemon
./target/release/spacedrive start

# Create your first library and location
./target/release/spacedrive library create "My Library"
./target/release/spacedrive location add ~/Documents --name "Documents"

# Monitor indexing progress
./target/release/spacedrive job monitor
```

For detailed CLI usage, see [CLI Documentation](./docs/cli.md).

## Architecture Decisions Log

### Why SeaORM?
- Active development and community
- Excellent async support
- Clean migration system
- No need for custom forks

### Why unified file operations?
- Users don't care about our indexing implementation
- Reduces code duplication
- Simplifies mental model

### Why event-driven?
- Decouples backend from frontend
- Enables future plugin system
- Standard pattern in modern applications