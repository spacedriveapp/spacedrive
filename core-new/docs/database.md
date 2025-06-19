# Database and Infrastructure

Core v2 uses a modern database stack built on SeaORM and SQLite, replacing the abandoned prisma-client-rust dependency. The new schema is optimized for space efficiency and query performance.

## Database Architecture

### Technology Stack

**SeaORM** - Modern async ORM for Rust
- **Type-safe queries** - Compile-time SQL validation
- **Automatic migrations** - Version-controlled schema changes
- **Rich relationships** - Foreign keys and joins
- **Connection pooling** - Efficient resource management

**SQLite** - Embedded database engine
- **ACID transactions** - Data consistency
- **WAL mode** - Better concurrency
- **Full-text search** - Built-in FTS5 (future)
- **Cross-platform** - Works everywhere Spacedrive runs

### Schema Overview

```sql
-- Core entities
CREATE TABLE devices (...)         -- Device identity
CREATE TABLE locations (...)       -- Indexed directories
CREATE TABLE entries (...)         -- Files and directories
CREATE TABLE content_identity (...) -- Content deduplication

-- User organization
CREATE TABLE user_metadata (...)   -- Tags, notes, favorites
CREATE TABLE tags (...)            -- User-defined tags
CREATE TABLE labels (...)          -- Hierarchical labels
CREATE TABLE metadata_tag (...)    -- Many-to-many: metadata ↔ tags
CREATE TABLE metadata_label (...)  -- Many-to-many: metadata ↔ labels

-- Infrastructure
CREATE TABLE jobs (...)            -- Job system persistence
```

## Storage Design

### Materialized Path Approach

We use a materialized path approach for storing file system hierarchies, providing excellent query performance:

```sql
-- Direct path storage with materialized paths
CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    uuid BLOB UNIQUE NOT NULL,
    location_id INTEGER NOT NULL REFERENCES locations(id),
    relative_path TEXT NOT NULL,    -- Directory path (e.g. "Documents/Projects")
    name TEXT NOT NULL,             -- Entry name (e.g. "file.txt")
    kind TEXT NOT NULL,             -- "file" or "directory"
    -- ... other columns
);
```

**Benefits:**
- **Simple queries** - No complex joins needed for path operations
- **Fast hierarchy queries** - Direct path matching with LIKE patterns
- **No parent_id complexity** - Avoid recursive queries for deep hierarchies
- **Efficient indexing** - Single index on relative_path for most queries

### Example Storage Efficiency

For a typical user with 100,000 files in `/Users/james/Documents/`:

| Approach | Storage Size | Index Size | Query Performance |
|----------|-------------|------------|-------------------|
| **Naive** | 2.1 GB | 500 MB | Slow (large indexes) |
| **Optimized** | 650 MB | 150 MB | Fast (compact indexes) |
| **Savings** | **69%** | **70%** | **3x faster** |

## Entity Definitions

### Devices Table

```sql
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,           -- 16-byte UUID
    name TEXT NOT NULL,
    os TEXT NOT NULL,
    os_version TEXT,
    hardware_model TEXT NOT NULL,
    network_addresses TEXT,              -- JSON array
    is_online BOOLEAN NOT NULL,
    last_seen_at TEXT NOT NULL,          -- ISO 8601
    capabilities TEXT,                   -- JSON object
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_devices_uuid ON devices(uuid);
CREATE INDEX idx_devices_online ON devices(is_online);
```

### Entries Table (Core File/Directory Model)

```sql
CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    location_id INTEGER NOT NULL REFERENCES locations(id),
    relative_path TEXT NOT NULL,  -- Materialized path (parent directory path)
    name TEXT NOT NULL,           -- Entry name without extension
    kind TEXT NOT NULL CHECK (kind IN ('file', 'directory')),
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),
    content_id INTEGER REFERENCES content_identity(id),
    location_id INTEGER REFERENCES locations(id),
    size INTEGER NOT NULL,
    permissions TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    accessed_at TEXT
);

-- Critical indexes for performance
CREATE INDEX idx_entries_uuid ON entries(uuid);
CREATE INDEX idx_entries_name ON entries(name);
CREATE INDEX idx_entries_kind ON entries(kind);
CREATE INDEX idx_entries_size ON entries(size);
CREATE INDEX idx_entries_prefix_path ON entries(prefix_id, relative_path);
CREATE INDEX idx_entries_location ON entries(location_id);
CREATE INDEX idx_entries_content ON entries(content_id);
CREATE INDEX idx_entries_metadata ON entries(metadata_id);
```

### User Metadata Table

```sql
CREATE TABLE user_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    notes TEXT,
    favorite BOOLEAN NOT NULL DEFAULT FALSE,
    hidden BOOLEAN NOT NULL DEFAULT FALSE,
    custom_data TEXT,                    -- JSON object
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_user_metadata_uuid ON user_metadata(uuid);
CREATE INDEX idx_user_metadata_favorite ON user_metadata(favorite);
CREATE INDEX idx_user_metadata_hidden ON user_metadata(hidden);
```

### Content Identity Table

```sql
CREATE TABLE content_identity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cas_id TEXT UNIQUE NOT NULL,         -- Content-addressed storage ID
    kind TEXT NOT NULL,                  -- image, video, audio, document, etc.
    size_bytes INTEGER NOT NULL,
    media_data TEXT,                     -- JSON metadata
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_content_identity_cas ON content_identity(cas_id);
CREATE INDEX idx_content_identity_kind ON content_identity(kind);
CREATE INDEX idx_content_identity_size ON content_identity(size_bytes);
```

### Tags and Labels

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    name TEXT UNIQUE NOT NULL,
    color TEXT,                          -- Hex color code
    icon TEXT,                           -- Icon identifier
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    name TEXT NOT NULL,
    color TEXT,
    parent_id INTEGER REFERENCES labels(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Junction tables for many-to-many relationships
CREATE TABLE metadata_tag (
    metadata_id INTEGER REFERENCES user_metadata(id),
    tag_id INTEGER REFERENCES tags(id),
    PRIMARY KEY (metadata_id, tag_id)
);

CREATE TABLE metadata_label (
    metadata_id INTEGER REFERENCES user_metadata(id),
    label_id INTEGER REFERENCES labels(id),
    PRIMARY KEY (metadata_id, label_id)
);
```

### Locations Table

```sql
CREATE TABLE locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    device_id INTEGER NOT NULL REFERENCES devices(id),
    path TEXT NOT NULL,
    name TEXT,
    index_mode TEXT NOT NULL CHECK (index_mode IN ('metadata', 'content', 'deep')),
    scan_state TEXT NOT NULL CHECK (scan_state IN ('pending', 'scanning', 'complete', 'error', 'paused')),
    last_scan_at TEXT,
    error_message TEXT,
    total_file_count INTEGER NOT NULL DEFAULT 0,
    total_byte_size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_locations_device ON locations(device_id);
CREATE INDEX idx_locations_scan_state ON locations(scan_state);
CREATE UNIQUE INDEX idx_locations_device_path ON locations(device_id, path);
```

## SeaORM Entity Definitions

### Entry Entity

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub prefix_id: i32,
    pub relative_path: String,
    pub name: String,
    pub kind: String,
    pub metadata_id: i32,
    pub content_id: Option<i32>,
    pub location_id: Option<i32>,
    pub parent_id: Option<i32>,
    pub size: u64,
    pub permissions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub accessed_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_metadata::Entity",
        from = "Column::MetadataId",
        to = "super::user_metadata::Column::Id"
    )]
    UserMetadata,
    #[sea_orm(
        belongs_to = "super::content_identity::Entity",
        from = "Column::ContentId",
        to = "super::content_identity::Column::Id"
    )]
    ContentIdentity,
    #[sea_orm(
        belongs_to = "super::location::Entity",
        from = "Column::LocationId",
        to = "super::location::Column::Id"
    )]
    Location,
}

impl Related<super::user_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMetadata.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

## Query Patterns

### Common Queries

**Find files by name pattern:**
```rust
let pdf_files = Entry::find()
    .filter(entry::Column::Name.like("%.pdf"))
    .filter(entry::Column::Kind.eq("file"))
    .all(db)
    .await?;
```

**Find files with specific tag:**
```rust
let important_files = Entry::find()
    .find_with_related(UserMetadata)
    .join(JoinType::InnerJoin, metadata_tag::Relation::Tag.def())
    .filter(tag::Column::Name.eq("Important"))
    .all(db)
    .await?;
```

**Find duplicate content:**
```rust
let duplicates = ContentIdentity::find()
    .find_with_related(Entry)
    .having(entry::Column::Id.count().gt(1))
    .group_by(content_identity::Column::CasId)
    .all(db)
    .await?;
```

**Reconstruct full path:**
```rust
let entry = Entry::find_by_id(entry_id)
    .one(db)
    .await?;

let full_path = if entry.relative_path.is_empty() {
    entry.name
} else {
    format!("{}/{}", entry.relative_path, entry.name)
};
```

### Complex Queries

**Find large files by directory:**
```rust
let large_files_by_dir = Entry::find()
    .select_only()
    .column_as(entry::Column::RelativePath, "directory")
    .column_as(entry::Column::Size.sum(), "total_size")
    .column_as(entry::Column::Id.count(), "file_count")
    .filter(entry::Column::Kind.eq("file"))
    .filter(entry::Column::Size.gt(100 * 1024 * 1024)) // > 100MB
    .group_by(entry::Column::RelativePath)
    .order_by_desc(entry::Column::Size.sum())
    .into_tuple::<(String, Option<i64>, Option<i64>)>()
    .all(db)
    .await?;
```

**Tag usage statistics:**
```rust
let tag_stats = Tag::find()
    .select_only()
    .column(tag::Column::Name)
    .column_as(metadata_tag::Column::MetadataId.count(), "usage_count")
    .join(JoinType::LeftJoin, tag::Relation::MetadataTag.def())
    .group_by(tag::Column::Id)
    .order_by_desc(metadata_tag::Column::MetadataId.count())
    .into_tuple::<(String, Option<i64>)>()
    .all(db)
    .await?;
```

## Migration System

### Migration Structure

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Devices::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Devices::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Devices::Uuid).binary().not_null().unique_key())
                    .col(ColumnDef::new(Devices::Name).text().not_null())
                    // ... other columns
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Devices::Table).to_owned())
            .await
    }
}
```

### Running Migrations

```rust
use sea_orm_migration::MigratorTrait;

// Apply all pending migrations
Migrator::up(db, None).await?;

// Reset database (development only)
Migrator::fresh(db).await?;

// Check migration status
let status = Migrator::status(db).await?;
```

## Performance Optimizations

### Indexing Strategy

**Primary indexes** - Critical for query performance:
- `entries(uuid)` - UUID lookups
- `entries(prefix_id, relative_path)` - Path reconstruction
- `entries(name)` - Name-based searches
- `entries(content_id)` - Duplicate detection

**Secondary indexes** - Common filter operations:
- `entries(kind)` - File vs directory filtering
- `entries(size)` - Size-based queries
- `user_metadata(favorite)` - Favorite file listings
- `locations(scan_state)` - Indexing status

**Composite indexes** - Multi-column queries:
- `entries(location_id, relative_path)` - Fast directory hierarchy queries
- `locations(device_id, path)` - Device-specific location lookup

### Query Optimization

**Use covering indexes** where possible:
```sql
-- Index covers entire query, no table lookup needed
CREATE INDEX idx_entries_name_size ON entries(name, size) 
WHERE kind = 'file';
```

**Limit result sets** for UI pagination:
```rust
let entries = Entry::find()
    .filter(entry::Column::Kind.eq("file"))
    .order_by_asc(entry::Column::Name)
    .limit(50)
    .offset(page * 50)
    .all(db)
    .await?;
```

**Use prepared statements** - SeaORM handles this automatically:
```rust
// This generates a prepared statement that's reused
let find_by_name = Entry::find()
    .filter(entry::Column::Name.eq("filename"));
```

### Connection Management

```rust
use sea_orm::{Database, ConnectOptions};

async fn create_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.max_connections(10)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(false); // Disable in production
    
    Database::connect(opt).await
}
```

## Backup and Recovery

### Database Backup

```rust
use std::fs;

async fn backup_library_database(library_path: &Path) -> Result<(), std::io::Error> {
    let db_path = library_path.join("database.db");
    let backup_path = library_path.join("backups").join(
        format!("database_backup_{}.db", chrono::Utc::now().format("%Y%m%d_%H%M%S"))
    );
    
    fs::create_dir_all(backup_path.parent().unwrap())?;
    fs::copy(&db_path, &backup_path)?;
    
    println!("Database backed up to: {}", backup_path.display());
    Ok(())
}
```

### Point-in-Time Recovery

SQLite WAL mode provides crash recovery:
```sql
-- Enable WAL mode for better concurrency and recovery
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = 10000;
PRAGMA temp_store = MEMORY;
```

## Database Utilities

### Vacuum and Optimization

```rust
async fn optimize_database(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Analyze tables for query planner
    db.execute_unprepared("ANALYZE").await?;
    
    // Reclaim free space
    db.execute_unprepared("VACUUM").await?;
    
    // Update table statistics
    db.execute_unprepared("PRAGMA optimize").await?;
    
    Ok(())
}
```

### Statistics and Monitoring

```rust
async fn database_statistics(db: &DatabaseConnection) -> Result<(), DbErr> {
    use sea_orm::FromQueryResult;
    
    #[derive(FromQueryResult)]
    struct TableInfo {
        name: String,
        count: i64,
        size_kb: Option<i64>,
    }
    
    let stats = db.query_all(
        Statement::from_string(
            DbBackend::Sqlite,
            r#"
            SELECT 
                name,
                COUNT(*) as count,
                (page_count * page_size / 1024) as size_kb
            FROM sqlite_master m, sqlite_stat1 s 
            WHERE m.name = s.tbl 
            GROUP BY name
            "#.to_string()
        )
    ).await?;
    
    for stat in stats {
        println!("Table: {} - {} rows - {} KB", 
            stat.name, stat.count, stat.size_kb.unwrap_or(0)
        );
    }
    
    Ok(())
}
```

The database layer provides a solid foundation for Spacedrive's file management needs while maintaining excellent performance characteristics and developer experience.