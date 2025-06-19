# Architecture Overview

## High-Level Design

Core v2 follows a clean layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────┐
│                 Core API                    │  ← Future: GraphQL/REST
├─────────────────────────────────────────────┤
│              Core Manager                   │  ← Main entry point
├─────────────────────────────────────────────┤
│         Domain & Operations Layer           │
├─────────────────────────────────────────────┤
│           Infrastructure Layer              │
└─────────────────────────────────────────────┘
```

## Core Components

### Core Manager (`Core`)

The main orchestrator that manages all subsystems:

```rust
pub struct Core {
    config: Arc<RwLock<AppConfig>>,     // Application configuration
    device: Arc<DeviceManager>,         // Device identity
    libraries: Arc<LibraryManager>,     // Library management
    events: Arc<EventBus>,              // Event communication
    services: Services,                 // Background services
}
```

**Responsibilities:**
- Initialize and coordinate all subsystems
- Manage application lifecycle
- Provide unified access to capabilities
- Handle graceful shutdown

### Library Management

Libraries are the core organizational unit in Spacedrive:

```rust
pub struct Library {
    id: Uuid,
    config: LibraryConfig,
    database: Database,
    thumbnail_manager: ThumbnailManager,
    // ... other components
}
```

**Key Features:**
- **File-based storage** - Each library is a `.sdlibrary` directory
- **SQLite database** - Optimized schema with SeaORM
- **Thumbnail management** - Efficient storage and retrieval
- **Atomic operations** - Consistent state management
- **Locking mechanism** - Prevents concurrent access conflicts

### Device Management  

Unified device identity (solving the Node/Device/Instance confusion):

```rust
pub struct Device {
    pub id: Uuid,           // Unique device identifier
    pub name: String,       // Human-readable name
    pub os: OperatingSystem,
    pub hardware_model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Design Principles:**
- **One identity per installation** - No more multiple overlapping concepts
- **Persistent across restarts** - Stable device identity
- **OS integration** - Automatic hardware detection

## Domain Layer

### Entry-Centric Model

Everything is an `Entry` - files and directories are treated uniformly:

```rust
pub struct Entry {
    pub id: i32,                    // Database ID
    pub uuid: Uuid,                 // Global identifier
    pub name: String,               // Display name
    pub kind: EntryKind,            // File or Directory
    pub size: u64,                  // Size in bytes
    pub metadata_id: i32,           // Always present
    pub content_id: Option<i32>,    // Optional for deduplication
    // ... timestamps and paths
}
```

**Benefits:**
- **Immediate metadata** - Every entry can be tagged/organized from creation
- **Unified operations** - Same APIs work for files and directories
- **Flexible relationships** - Content identity separate from metadata

### Optimized Storage Schema

Path compression reduces storage requirements by 70%+ for large collections:

```rust
// Instead of storing full paths repeatedly:
// /Users/james/Documents/file1.txt
// /Users/james/Documents/file2.txt
// /Users/james/Documents/subdir/file3.txt

// We store materialized paths directly:
Entry { relative_path: "", name: "file1.txt", location_id: 1 }
Entry { relative_path: "", name: "file2.txt", location_id: 1 }  
Entry { relative_path: "subdir", name: "file3.txt", location_id: 1 }
```

## Infrastructure Layer

### Event Bus

Decoupled communication using a type-safe event system:

```rust
pub enum Event {
    CoreStarted,
    CoreShutdown,
    LibraryCreated { id: Uuid, name: String },
    LibraryOpened { id: Uuid },
    LibraryClosed { id: Uuid },
    EntryCreated { library_id: Uuid, entry_id: Uuid },
    EntryModified { library_id: Uuid, entry_id: Uuid },
    // ... more events
}
```

**Replaces the problematic `invalidate_query!` pattern** with proper event-driven architecture.

### Job System

Minimal boilerplate job processing:

```rust
#[derive(Serialize, Deserialize)]
pub struct MyJob {
    // Job fields
}

impl Job for MyJob {
    const NAME: &'static str = "my_job";
    const RESUMABLE: bool = true;
}

#[async_trait]
impl JobHandler for MyJob {
    type Output = MyOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Job implementation
    }
}
```

**Key improvements over original:**
- **50 lines vs 500+** to implement a new job
- **Automatic serialization** with MessagePack
- **Database persistence** with checkpointing
- **Type-safe progress** reporting

### Database Layer

Modern ORM with proper migrations:

```rust
// SeaORM entities with proper relationships
#[derive(DeriveEntityModel)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub name: String,
    // ... other fields
}

// Automatic relationship handling
impl Related<super::user_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMetadata.def()
    }
}
```

## Data Flow

### Library Operations

```
User Request → Core → LibraryManager → Library → Database
                ↓
           EventBus ← Event Emission
```

### File Operations  

```
Operation Request → JobManager → Job Execution → Database Update
                                      ↓
                              Progress Updates → EventBus
```

### Event Propagation

```
Domain Change → Event Creation → EventBus → Subscribers
                                    ↓
                              Frontend Updates (Future)
```

## Design Principles

### 1. Event-Driven Architecture
- **Loose coupling** - Components communicate via events
- **Extensibility** - Easy to add new event handlers
- **Debuggability** - Clear audit trail of system changes

### 2. Domain-First Design
- **Business logic in domain layer** - Clear separation from infrastructure
- **Rich domain models** - Behavior lives with data
- **Ubiquitous language** - Code reflects business concepts

### 3. Pragmatic Choices
- **Monolith over microservices** - Simpler deployment and development
- **SQLite over complex databases** - Perfect for client-side storage
- **Standard libraries over custom** - Leverage existing, well-tested solutions

### 4. Performance Optimization
- **Optimized schemas** - Significant space savings
- **Efficient queries** - Proper indexing and relationships  
- **Async everywhere** - Non-blocking operations
- **Resource pooling** - Shared connections and managers

## Error Handling

Consistent error handling across all layers:

```rust
// Domain errors
#[derive(thiserror::Error, Debug)]
pub enum LibraryError {
    #[error("Library not found: {0}")]
    NotFound(Uuid),
    #[error("Library already open: {0}")]
    AlreadyOpen(PathBuf),
    // ... other variants
}

// Result types for operations
type LibraryResult<T> = Result<T, LibraryError>;
```

## Testing Strategy

- **Unit tests** - Domain logic and utilities
- **Integration tests** - Full system workflows  
- **Property tests** - Database consistency
- **Example tests** - Documentation as runnable code

## Future Extensions

The architecture supports planned features:

- **API Layer** - GraphQL/REST endpoints
- **Sync System** - Third-party database sync
- **Plugin System** - Dynamic job registration
- **Search Engine** - Full-text and semantic search
- **Real-time Updates** - WebSocket event streaming