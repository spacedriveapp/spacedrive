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
    /// Application configuration
    pub config: Arc<RwLock<AppConfig>>,
    
    /// Device manager
    pub device: Arc<DeviceManager>,
    
    /// Library manager
    pub libraries: Arc<LibraryManager>,
    
    /// Volume manager
    pub volumes: Arc<VolumeManager>,
    
    /// Event bus for state changes
    pub events: Arc<EventBus>,
    
    /// Container for high-level services
    pub services: Services,
    
    /// Shared context for core components
    pub context: Arc<CoreContext>,
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
    /// Root directory of the library (the .sdlibrary folder)
    path: PathBuf,
    
    /// Library configuration
    config: RwLock<LibraryConfig>,
    
    /// Database connection
    db: Arc<Database>,
    
    /// Job manager for this library
    jobs: Arc<JobManager>,
    
    /// Lock preventing concurrent access
    _lock: LibraryLock,
}
```

**Key Features:**
- **File-based storage** - Each library is a `.sdlibrary` directory
- **SQLite database** - Optimized schema with SeaORM
- **Integrated thumbnail management** - Built into Library methods (no separate ThumbnailManager)
- **Atomic operations** - Consistent state management
- **Locking mechanism** - Prevents concurrent access conflicts
- **Job integration** - Dedicated job manager per library
- **Library statistics** - Tracks files, size, locations, tags, thumbnails
- **Device registration** - Tracks devices accessing the library

### Device Management  

Unified device identity (solving the Node/Device/Instance confusion):

```rust
pub struct Device {
    pub id: Uuid,                              // Unique device identifier
    pub name: String,                          // Human-readable name
    pub os: OperatingSystem,
    pub hardware_model: Option<String>,        // Optional hardware model
    pub network_addresses: Vec<String>,        // For P2P connections
    pub is_online: bool,                       // Device online status
    pub sync_leadership: HashMap<Uuid, SyncRole>, // Sync roles per library
    pub last_seen_at: DateTime<Utc>,           // Last time device was seen
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Design Principles:**
- **One identity per installation** - No more multiple overlapping concepts
- **Persistent across restarts** - Stable device identity stored in device.json
- **OS integration** - Automatic OS detection, partial hardware detection (macOS only)
- **Sync coordination** - Built-in sync leadership model per library

## Domain Layer

### Entry-Centric Model

Everything is an `Entry` - files and directories are treated uniformly:

```rust
pub struct Entry {
    pub id: i32,                    // Database ID
    pub uuid: Option<Uuid>,         // Global identifier (conditional)
    pub name: String,               // Display name
    pub kind: i32,                  // File(0), Directory(1), or Symlink(2)
    pub size: i64,                  // Size in bytes
    pub relative_path: String,      // Path relative to location
    pub location_id: i32,           // Reference to location
    pub metadata_id: Option<i32>,   // UserMetadata (created on demand)
    pub content_id: Option<i32>,    // Optional for deduplication
    // ... timestamps
}
```

**Benefits:**
- **On-demand metadata** - UserMetadata created only when user adds tags/notes
- **Unified operations** - Same APIs work for files and directories
- **Flexible relationships** - Content identity separate from metadata
- **Conditional UUIDs** - Assigned to directories immediately, files after content identification

### Storage Schema

Paths are stored as relative paths from location root:

```rust
// Paths stored relative to location:
Entry { relative_path: "", name: "file1.txt", location_id: 1 }
Entry { relative_path: "", name: "file2.txt", location_id: 1 }  
Entry { relative_path: "subdir", name: "file3.txt", location_id: 1 }
```

**Note:** Path compression mentioned in design documents is not currently implemented.

## Infrastructure Layer

### Event Bus

Decoupled communication using a type-safe event system:

```rust
pub enum Event {
    // Core events
    CoreStarted,
    CoreShutdown,
    
    // Library events
    LibraryCreated { id: Uuid, name: String, path: PathBuf },
    LibraryOpened { id: Uuid, name: String, path: PathBuf },
    LibraryClosed { id: Uuid, name: String },
    LibraryDeleted { id: Uuid, name: String },
    
    // Entry events
    EntryCreated { library_id: Uuid, entry_id: Uuid },
    EntryModified { library_id: Uuid, entry_id: Uuid },
    EntryDeleted { library_id: Uuid, entry_id: Uuid },
    EntryMoved { library_id: Uuid, entry_id: Uuid, old_path: PathBuf, new_path: PathBuf },
    
    // Volume events
    VolumeAdded(Volume),
    VolumeRemoved(Uuid),
    VolumeUpdated(Volume),
    
    // Job events
    JobQueued { id: Uuid, name: String },
    JobStarted { id: Uuid },
    JobProgress { id: Uuid, progress: Progress },
    JobCompleted { id: Uuid },
    JobFailed { id: Uuid, error: String },
    
    // ... and more
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
- **50 lines vs 500-1000+** to implement a new job
- **Automatic serialization** with MessagePack
- **Database persistence** with checkpointing  
- **Type-safe progress** reporting
- **#[derive(Job)] macro** - Auto-generates registration and boilerplate
- **Lifecycle methods** - Optional on_pause, on_resume, on_cancel
- **Inventory-based registration** - Jobs auto-register at compile time

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
- **Sync System** - Third-party database sync (foundation in place with sync_leadership)
- **Plugin System** - Dynamic job registration
- **Search Engine** - Full-text and semantic search
- **Real-time Updates** - WebSocket event streaming
- **Path Compression** - Implement the designed path compression for space savings
- **Agent System** - Recently introduced agent manager design