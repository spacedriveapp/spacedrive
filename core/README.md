# Spacedrive Core

Rust library implementing the Virtual Distributed File System (VDFS) architecture for local-first, AI-native file management.

## Architecture

### Structure

```
src/
├── domain/        # Core data models (Entry, Library, Device)
├── ops/           # Operations (actions and queries, CQRS pattern)
├── infra/         # Infrastructure (database, events, wire protocol)
├── service/       # High-level services (network, jobs, sessions)
├── location/      # Location management and indexing
├── library/       # Library lifecycle and operations
├── device/        # Device identity and management
├── volume/        # Volume detection and fingerprinting
├── config/        # Application configuration
├── crypto/        # Cryptographic primitives
└── bin/           # Binaries (cli, daemon)
```

### Core Components

**Core Manager** (`Core` struct in `lib.rs`)

- Coordinates all subsystems
- Manages application lifecycle
- Provides unified access to capabilities

**Library** (`library/`)

- File-based storage (`.sdlibrary` directories)
- SQLite database with SeaORM
- Job management and thumbnail generation
- Device registration and sync coordination

**Entry-Centric Model** (`domain/entry.rs`)

- Unified representation for files and directories
- Conditional UUIDs (directories immediate, files after content ID)
- On-demand UserMetadata creation
- Relative paths from location roots

**Device Management** (`device/`)

- Single device identity per installation
- Persistent across restarts (`device.json`)
- Sync leadership model per library
- Network address tracking for P2P

**Operations Layer** (`ops/`)

- CQRS pattern: Actions (mutations) and Queries (reads)
- Wire protocol with automatic type generation
- Registry system using `inventory` crate
- Shared across daemon RPC, iOS FFI, and WASM extensions

**Infrastructure** (`infra/`)

- `api/`: Wire protocol dispatcher and RPC server
- `event/`: Event bus for state changes
- `action/`: Transactional action system with preview-commit-verify
- `query/`: Read-optimized query handlers

**Networking** (`service/network/`)

- Iroh-based P2P networking
- Device pairing protocol
- File transfer (Spacedrop)
- mDNS local discovery

**Jobs** (`service/jobs/`)

- Durable, resumable operations
- MessagePack serialization for state
- Progress reporting and cancellation
- Per-library job managers

**Indexing** (`location/indexer/`)

- Five-phase pipeline (discover, classify, extract, thumbnail, cleanup)
- File system watcher integration
- Rules engine with glob patterns
- Resumable with checkpoints

**Volume Management** (`volume/`)

- Cross-platform volume detection
- Fingerprinting for identity
- Mount point tracking

## Communication Patterns

### Daemon-Client (Desktop/CLI)

- Unix socket JSON-RPC 2.0
- Wire method strings (e.g., `query:vdfs.list_entries.v1`)
- Operations auto-register at compile time

### Embedded FFI (iOS/Mobile)

- Direct Rust library integration
- Same JSON-RPC protocol over FFI
- Swift client with Specta-generated types

### Extensions (WASM)

- Sandboxed WASM modules
- Minimal host functions (log, register_job)
- SDK with procedural macros (`crates/sdk/`)
- Models, jobs, actions, agents, UI manifests

## Key Technologies

- **Async runtime**: tokio
- **Database**: SQLite via SeaORM
- **Serialization**: serde, Specta for type generation
- **Networking**: Iroh (P2P), mdns-sd (local discovery)
- **WASM**: wasmer runtime, spacedrive-sdk
- **Jobs**: inventory for registration, rmp-serde for state
- **Crypto**: blake3 (content addressing), ed25519 (signing)
- **Indexing**: notify (fs watcher), globset (rules)

## Building

```bash
# Full build
cargo build --release

# With optional features
cargo build --features ffmpeg,ai,heif

# Specific binary
cargo build --bin spacedrive
cargo build --bin daemon

# Run CLI
cargo run --bin spacedrive -- --help
```

## Binaries

- `spacedrive`: CLI interface
- `daemon`: Background daemon process

## Development

- Uses CQRS and DDD patterns
- Operations register via `inventory` crate macros
- Resumable jobs with MessagePack state serialization
- Type-safe Wire protocol with Specta generation
- Event-driven architecture with EventBus
- No layered architecture (direct Rust patterns)

## Testing

```bash
# All tests
cargo test

# Specific module
cargo test --lib location::indexer

# Integration tests
cargo test --test indexer_test
```

See `/docs/core/` for detailed architecture documentation.
