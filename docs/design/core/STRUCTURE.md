# core Structure

```
core/
├── Cargo.toml                    # Dependencies (SeaORM, axum, etc.)
├── README.md                     # Overview and strategy
├── MIGRATION.md                  # How to migrate from old core
├── ARCHITECTURE_DECISIONS.md     # ADRs documenting choices
├── STRUCTURE.md                  # This file
│
└── src/
    ├── lib.rs                    # Main Core struct and initialization
    │
    ├── domain/                   # Core business entities
    │   ├── mod.rs
    │   ├── device.rs            # Unified device (no more node/instance)
    │   ├── library.rs           # Library management
    │   ├── location.rs          # Folder tracking
    │   └── object.rs            # Unique files with metadata
    │
    ├── operations/              # Business operations (what users care about)
    │   ├── mod.rs
    │   ├── file_ops/           # THE IMPORTANT STUFF
    │   │   ├── mod.rs          # Common types and utils
    │   │   └── copy.rs         # Example: unified copy operation
    │   ├── indexing.rs         # File scanning
    │   ├── media_processing.rs # Thumbnails and metadata
    │   ├── search.rs           # Proper search implementation
    │   └── sync.rs             # Multi-device sync
    │
    ├── infrastructure/         # External interfaces
    │   ├── mod.rs
    │   ├── api.rs             # GraphQL API example
    │   ├── database.rs        # SeaORM setup
    │   ├── events.rs          # Event bus (replaces invalidate_query!)
    │   └── jobs.rs            # Simple job system (if needed)
    │
    └── shared/                # Common code
        ├── mod.rs
        ├── errors.rs          # Unified error types
        ├── types.rs           # Shared type definitions
        └── utils.rs           # Common utilities
```

## Key Improvements

1. **Clear Organization**: You can immediately see where file operations live
2. **No Dual Systems**: One implementation for all files
3. **No invalidate_query!**: Clean event-driven architecture
4. **No Prisma**: Using SeaORM for maintainability
5. **Unified Identity**: Just "Device", not node/device/instance
6. **Pragmatic Monolith**: No cyclic dependency hell

## Next Steps

1. Start implementing domain models with SeaORM
2. Port file operations one at a time
3. Build GraphQL API incrementally
4. Create integration tests for each operation
5. Develop migration tooling
