# Spacedrive Core v2 Documentation

**A unified, simplified architecture for cross-platform file management.**

## Overview

Core v2 is a complete rewrite of Spacedrive's core system, designed to address the architectural issues identified in the original codebase. It implements a clean, event-driven architecture with unified file management and a dramatically simplified job system.

## Key Improvements

### ✅ Unified File System
- **Single API** for all file operations (no more dual indexed/ephemeral systems)
- **Consistent behavior** across all file management scenarios
- **Bridge operations** between different storage modes

### ✅ Event-Driven Architecture
- **Replaced query invalidation** with proper event bus
- **Type-safe events** for state changes
- **Decoupled frontend/backend** communication

### ✅ Modern Database Layer
- **SeaORM** instead of abandoned prisma-client-rust
- **Optimized storage** with 70%+ space savings for large file collections
- **Proper migrations** and database versioning

### ✅ Simplified Job System
- **50 lines** vs 500+ lines to create new jobs
- **Automatic serialization** with MessagePack
- **Type-safe progress** reporting
- **Database persistence** with resume capabilities

### ✅ Clean Domain Models
- **Entry-centric design** where every file/folder has metadata by default
- **Optional content identity** for deduplication
- **Unified device management** (no more Node/Device/Instance confusion)

## What's Complete

- [x] **Core initialization and lifecycle**
- [x] **Library management** (create, open, close, discovery)
- [x] **Device management** with persistent identity
- [x] **Domain models** (Entry, Location, Device, UserMetadata, ContentIdentity)
- [x] **Database layer** with SeaORM and migrations
- [x] **Job system infrastructure** with example jobs
- [x] **Event bus** for decoupled communication
- [x] **File operations** foundation (copy jobs)
- [x] **Indexing operations** foundation
- [x] **Comprehensive tests** and working examples

## Architecture Documents

- **[Architecture Overview](architecture.md)** - High-level system design
- **[Domain Models](domain-models.md)** - Core business entities and their relationships
- **[Job System](job-system.md)** - Background task processing and job management
- **[Database](database.md)** - Data persistence and storage optimization
- **[Examples](examples.md)** - Working code examples and usage patterns

## Quick Start

```rust
use sd_core::Core;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize core
    let core = Core::new().await?;

    // Create a library
    let library = core.libraries
        .create_library("My Library", None)
        .await?;

    println!("Library created: {}", library.name().await);
    println!("Path: {}", library.path().display());

    // Core automatically handles cleanup on drop
    Ok(())
}
```

## Running Examples

```bash
# Library management demo
cargo run --example library_demo

# Job system demo
cargo run --example job_demo

# File type system demo
cargo run --example file_type_demo
```

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test library_test
cargo test job_system_test
cargo test indexer_job_test
```

## Project Status

Core v2 provides a solid foundation for Spacedrive's file management capabilities. The architecture is designed to be:

- **Simple** - Fewer abstractions, clearer responsibilities
- **Maintainable** - Modern Rust patterns, comprehensive tests
- **Extensible** - Event-driven design, pluggable job system
- **Performant** - Optimized database schema, efficient operations

## Next Steps

1. **API Layer** - GraphQL/REST API implementation
2. **Advanced Search** - Full-text search with SQLite FTS5
3. **Sync System** - Cloud/P2P synchronization using third-party solutions
4. **Media Processing** - Thumbnail generation and metadata extraction
5. **File Watching** - Real-time filesystem monitoring

## Contributing

See the [examples](examples.md) for detailed usage patterns and the architecture docs for implementation guidance.
