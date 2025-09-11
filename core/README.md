# Spacedrive Core v2: Clean Architecture for the VDFS Future

A complete reimplementation of Spacedrive's core with modern Rust patterns, unified file operations, and a foundation built for the Virtual Distributed File System vision.

## The Foundation We Built

This core rewrite addresses fundamental architectural issues from the original implementation while establishing clean patterns for future development:

### 1. **Unified File System Architecture**

Single abstraction for all file operations:

```rust
// No more dual systems - everything works through SdPath
pub struct SdPath {
    device_id: Uuid,    // Which device
    path: PathBuf,      // Path on that device
}

// Same operations work everywhere
copy_files(sources: Vec<SdPath>, destination: SdPath)
```

### 2. **Modern Database Foundation**

Built on SeaORM with SQLite:

- No abandoned dependencies
- Proper migration system
- Type-safe queries
- Clean entity relationships

### 3. **Clean Job System**

Background processing with proper patterns:

```rust
// Standardized job configuration
pub struct IndexerJobConfig {
    location_id: Option<Uuid>,
    path: SdPath,
    mode: IndexMode,
    scope: IndexScope,        // Current vs Recursive
    persistence: IndexPersistence, // Database vs Ephemeral
}
```

### 4. **Event-Driven Architecture**

Replace invalidation anti-patterns:

```rust
pub enum CoreEvent {
    FileCreated { path: SdPath },
    IndexingProgress { location_id: Uuid, progress: f64 },
    DeviceConnected { device_id: Uuid },
}
```

### 5. **Domain-Driven Organization**

Clear separation of concerns:

```
src/
├── domain/           # Core business entities
├── operations/       # What users do (copy, index, search)
├── infrastructure/   # External interfaces (CLI, API, database)
└── shared/          # Common types and utilities
```

## Core Features

### 1. **SdPath: Cross-Device File Operations**

The foundation for virtual distributed file systems:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SdPath {
    device_id: Uuid,            // Which device
    path: PathBuf,              // Path on that device
}

impl SdPath {
    pub fn new(device_id: Uuid, path: impl Into<PathBuf>) -> Self;
    pub fn local(path: impl Into<PathBuf>) -> Self;
    pub fn is_local(&self) -> bool;
    pub fn display(&self) -> String;
}
```

**Design benefits:**

- Unified abstraction for all file locations
- Prepares foundation for cross-device operations
- Clean API that works locally today, networked tomorrow

### 2. **Enhanced Indexing System**

Flexible indexing with scope and persistence control:

```rust
pub enum IndexScope {
    Current,    // Single directory level (<500ms target)
    Recursive,  // Full tree traversal
}

pub enum IndexPersistence {
    Persistent,  // Database storage
    Ephemeral,   // Memory-only browsing
}

pub struct IndexerJobConfig {
    location_id: Option<Uuid>,
    path: SdPath,
    mode: IndexMode,
    scope: IndexScope,
    persistence: IndexPersistence,
}
```

**Key capabilities:**

- **Current scope**: Fast directory browsing for UI navigation
- **Ephemeral mode**: Browse external paths without database pollution
- **Smart constructors**: Pre-configured patterns for common use cases

### 3. **Modern Database Layer**

Built on SeaORM with SQLite for reliability:

```rust
// Entry - universal file/directory model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub location_id: i32,
    pub relative_path: String,      // Materialized paths for efficiency
    pub name: String,
    pub kind: String,               // "file" or "directory"
    pub metadata_id: i32,           // Always present - immediate metadata
    pub content_id: Option<i32>,    // Optional content addressing
    pub size: u64,
    // ... timestamps and metadata
}
```

**Architecture benefits:**

- Modern async ORM with active maintenance
- Materialized path storage for query performance
- Type-safe database operations
- Proper migration system

### 4. **Working CLI Interface**

Proof-of-concept CLI demonstrating the architecture:

```bash
# Library management
spacedrive library create "My Files"
spacedrive library list
spacedrive library open ~/Documents/My\ Files.sdlibrary

# Location management
spacedrive location add ~/Documents --name "Documents"
spacedrive location list

# Enhanced indexing capabilities
spacedrive index quick-scan ~/Desktop --scope current
spacedrive index browse /tmp --ephemeral --scope recursive
spacedrive index location ~/Pictures --mode deep

# Job monitoring
spacedrive job list --status running
spacedrive job monitor
spacedrive status
```

### 5. **Self-Contained Libraries**

Libraries are portable directories:

- `library.json` - Configuration and device registry
- `database.db` - All metadata and search indices
- `thumbnails/` - Generated previews
- `.lock` - Concurrency protection

**Benefits:**

- **Backup** = copy the directory
- **Share** = send the directory
- **Migrate** = move the directory

## Technical Foundation

### 1. **Materialized Path Storage**

Efficient file hierarchy representation:

```rust
pub struct Entry {
    relative_path: String,  // Directory path: "Documents/Projects"
    name: String,          // File name: "README.md"
    // ... other fields
}
```

Benefits:

- Direct path queries without complex joins
- Efficient indexing for hierarchy operations
- Simple reconstruction of full paths

### 2. **Unified Metadata Model**

Every entry gets immediate metadata capabilities:

```rust
pub struct Entry {
    metadata_id: i32,           // Always present
    content_id: Option<i32>,    // Optional content addressing
    // ... other fields
}
```

Design benefits:

- Tag and organize files immediately
- No waiting for indexing to complete
- Progressive enhancement as analysis runs

### 3. **Multi-Phase Indexing**

Structured indexing pipeline:

- **Discovery**: File system traversal
- **Processing**: Database entry creation
- **Aggregation**: Directory statistics
- **Content Identification**: Hash generation (if enabled)

Features:

- Resumable operations with state checkpoints
- Graceful error handling for individual files
- Real-time progress reporting

## Development Status

Currently working features:

### ✅ Foundation

- **Library management**: Create, open, list libraries
- **Location management**: Add, list, and monitor indexed directories
- **Device identity**: Unified device tracking and capabilities
- **Database layer**: SeaORM entities with proper migrations

### ✅ Indexing System

- **Scope control**: Current directory vs recursive indexing
- **Persistence modes**: Database storage vs ephemeral browsing
- **Multi-phase pipeline**: Discovery, processing, aggregation
- **Progress tracking**: Real-time job monitoring

### ✅ CLI Interface

- Working command-line interface demonstrating all features
- Library and location management commands
- Enhanced indexing with scope and persistence options
- Job monitoring and system status

### 🚧 In Development

- **File operations**: Copy, move, delete jobs (infrastructure ready)
- **Search system**: SQLite FTS integration for content search
- **Event system**: Core event broadcasting for UI updates
- **Network layer**: P2P device communication

### 📋 Planned

- **Cross-device operations**: Copy/move files between devices
- **Advanced search**: Content indexing and semantic search
- **Desktop app integration**: Replace original core as backend
- **Cloud sync**: Optional cloud backup and synchronization

## Quick Start

```bash
# Clone and build
git clone https://github.com/spacedriveapp/spacedrive
cd spacedrive/core
cargo build --release

# Try the CLI
cargo run --bin spacedrive -- library create "Test Library"
cargo run --bin spacedrive -- location add ~/Documents
cargo run --bin spacedrive -- job monitor
```

## Architecture Principles

### Clean Separation

- **Domain**: Business logic and entities
- **Operations**: User-facing functionality
- **Infrastructure**: External interfaces and persistence
- **Shared**: Common types and utilities

### Modern Rust Patterns

- **Type safety**: Compile-time guarantees throughout
- **Async/await**: Non-blocking operations by default
- **Error handling**: Comprehensive `Result` types
- **Memory safety**: No unsafe code in core business logic

### Extensible Design

- **Plugin-ready**: Clear interfaces for future extensions
- **Event-driven**: Loose coupling between components
- **Configuration**: Flexible behavior through configuration
- **Testing**: Mockable interfaces for reliable testing

## Contributing

See individual module documentation:

- [`docs/`](./docs/) - Comprehensive architecture documentation
- [`examples/`](./examples/) - Working code examples
- [`src/`](./src/) - Well-documented source code
  The codebase prioritizes clarity and maintainability over clever solutions. We believe the best code is code that's easy to understand, modify, and extend.
