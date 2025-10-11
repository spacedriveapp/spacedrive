# Extension Models: Final Architecture

**Design:** Clean separation with unified tag/collection support.

---

## Core Schema Changes

### 1. New `models` Table

```sql
CREATE TABLE models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,

    -- Model identity
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,          -- "Person", "Album", "Email", etc.

    -- Data storage
    data TEXT NOT NULL,                -- JSON serialized model

    -- Metadata (same as entries)
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),

    -- Sync
    sync_strategy INTEGER NOT NULL,    -- 0=DeviceOwned, 1=Shared
    hlc_timestamp TEXT,                -- For shared sync
    device_uuid BLOB,

    -- Timestamps
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    UNIQUE(extension_id, model_type, uuid)
);

CREATE INDEX idx_models_uuid ON models(uuid);
CREATE INDEX idx_models_extension ON models(extension_id, model_type);
CREATE INDEX idx_models_metadata ON models(metadata_id);
CREATE INDEX idx_models_sync ON models(sync_strategy, hlc_timestamp);

-- FTS5 for search
CREATE VIRTUAL TABLE models_fts USING fts5(
    uuid UNINDEXED,
    extension_id UNINDEXED,
    model_type UNINDEXED,
    data,
    content='models',
    content_rowid='id'
);
```

### 2. Junction Table: `model_entry`

```sql
-- Links models to entries (optional, many-to-many)
CREATE TABLE model_entry (
    model_uuid BLOB NOT NULL REFERENCES models(uuid),
    entry_uuid BLOB NOT NULL REFERENCES entries(uuid),

    -- Optional relationship metadata
    relationship_type TEXT,  -- "contains", "depicts", "created_by", etc.

    PRIMARY KEY (model_uuid, entry_uuid)
);

CREATE INDEX idx_model_entry_model ON model_entry(model_uuid);
CREATE INDEX idx_model_entry_entry ON model_entry(entry_uuid);
```

### 3. Extend Collections to Support Models

```sql
-- Current collections only reference entries
CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL,
    entry_id INTEGER REFERENCES entries(id),

    -- NEW: Also support models
    model_id INTEGER REFERENCES models(id),

    position INTEGER,

    CHECK (entry_id IS NOT NULL OR model_id IS NOT NULL),
    PRIMARY KEY (collection_id, entry_id, model_id)
);
```

### 4. Tags Already Work!

```sql
-- No changes needed! Tags reference user_metadata
-- Both entries and models have metadata_id
-- Tags work automatically for both
```

---

## How Extension Models Map

### Photo Model (Entry-Backed)

```rust
#[model(version = "1.0.0")]
struct Photo {
    #[entry(filter = "*.{jpg,png}")]
    file: Entry,

    // Stored in user_metadata.custom_data.photos
    #[custom_field]
    identified_people: Vec<Uuid>,  // References Person.uuid

    #[custom_field]
    place_id: Option<Uuid>,
}
```

**Storage:**
```sql
-- No model record! Photo is just an augmented Entry
-- The #[entry] attribute means Photo wraps an existing entry

-- Entry (kind=File)
entries { uuid: photo_uuid, name: "img.jpg", metadata_id: 123 }

-- Custom fields stored in metadata
user_metadata.custom_data = {
  "photos": {
    "identified_people": ["person_uuid_1"],
    "place_id": "place_uuid_1"
  }
}

-- No row in models table - Photo is entry + metadata augmentation
```

### Person Model (Standalone Virtual)

```rust
#[model(version = "1.0.0")]
#[sync_strategy = "shared"]
struct Person {
    id: Uuid,
    name: Option<String>,
    embeddings: Vec<Vec<f32>>,
    photo_count: usize,
}
```

**Storage:**
```sql
-- Model record
INSERT INTO models (
    uuid,
    extension_id,
    model_type,
    data,
    metadata_id,
    sync_strategy
) VALUES (
    '<person_uuid>',
    'photos',
    'Person',
    '{"id":"<uuid>","name":"Alice","embeddings":[[...]],"photo_count":42}',
    456,  -- Has user_metadata for tags
    1     -- Shared sync
);

-- Can be tagged
INSERT INTO metadata_tag VALUES (456, tag_id);
-- Tag: "#family"

-- Can be in collections
INSERT INTO collection_items VALUES (collection_id, NULL, model_id);
```

### Album Model (References Other Entities)

```rust
#[model(version = "1.0.0")]
struct Album {
    id: Uuid,
    name: String,
    photo_ids: Vec<Uuid>,  // Entry UUIDs
}
```

**Storage:**
```sql
-- Album model
INSERT INTO models (...) VALUES (
    '<album_uuid>',
    'photos',
    'Album',
    '{"id":"<uuid>","name":"Vacation","photo_ids":["photo1","photo2"]}',
    ...
);

-- Optional: Explicit relationships via model_entry
INSERT INTO model_entry VALUES ('<album_uuid>', 'photo1', 'contains');
INSERT INTO model_entry VALUES ('<album_uuid>', 'photo2', 'contains');

-- Allows reverse queries: "Which albums contain this photo?"
```

---

## The Model Type System

### SDK Syntax

```rust
// Entry-backed model (augments existing entry)
#[model]
struct Photo {
    #[entry] file: Entry,  // Wraps entry, no model record
    #[custom_field] place_id: Option<Uuid>,
}

// Standalone model (creates model record)
#[model]
#[sync_strategy = "shared"]
struct Person {
    id: Uuid,
    name: Option<String>,
    // All fields in models.data JSON
}

// Model that references entries (hybrid)
#[model]
struct Album {
    id: Uuid,
    name: String,

    #[references(Entry)]
    photo_ids: Vec<Uuid>,  // Creates model_entry records
}
```

### Macro Behavior

```rust
// Photo macro generates:
impl Photo {
    // Queries entries, augments with custom_data
    async fn from_entry(entry: Entry) -> Self { ... }
}

// Person macro generates:
impl Person {
    // Creates record in models table
    async fn create(ctx: &VdfsContext) -> Self { ... }

    // Queries models table
    async fn find(uuid: Uuid) -> Option<Self> { ... }
}
```

---

## Query Patterns

### Query Entry-Backed Models

```rust
// Photos in a location
let photos = ctx.vdfs()
    .query_entries()
    .in_location("/My Photos")
    .of_type::<Image>()
    .map(Photo::from_entry)  // Augment with custom_data
    .collect()
    .await?;
```

### Query Standalone Models

```rust
// All people
let people = ctx.vdfs()
    .query_models::<Person>()
    .collect()
    .await?;

// Under the hood:
// SELECT * FROM models
// WHERE extension_id = 'photos' AND model_type = 'Person'
```

### Query with Tags (Works for Both!)

```rust
// Tagged photos
let vacation_photos = ctx.vdfs()
    .query_entries()
    .with_tag("#vacation")
    .collect()
    .await?;

// Tagged people
let family_people = ctx.vdfs()
    .query_models::<Person>()
    .with_tag("#family")  // Joins models → metadata_id → tags
    .collect()
    .await?;
```

### Reverse Queries (via model_entry)

```rust
// Photos of Alice
let alice_uuid = ...;
let photos = ctx.vdfs()
    .query_entries()
    .referenced_by_model(alice_uuid, relationship = "depicts")
    .collect()
    .await?;

// Under the hood:
// SELECT e.* FROM entries e
// JOIN model_entry me ON e.uuid = me.entry_uuid
// WHERE me.model_uuid = '<alice_uuid>'
//   AND me.relationship_type = 'depicts'
```

---

## Where Data Actually Lives

### Photo Model

```
Photo {
    file: Entry                  → entries table (existing)
    exif: ExifData              → content_identity.media_data JSON (existing)
    detected_faces: Vec<Face>   → .sdlibrary/sidecars/content/{uuid}/extensions/photos/faces.json
    tags: Vec<Tag>              → tags table + metadata_tag junction
    identified_people: Vec<Uuid> → user_metadata.custom_data.photos.identified_people JSON
    place_id: Option<Uuid>      → user_metadata.custom_data.photos.place_id JSON
}

NO RECORD IN MODELS TABLE
Photo is just a typed wrapper around Entry + metadata + sidecars
```

### Person Model

```
Person {
    id: Uuid                    → models.uuid
    name: Option<String>        → models.data JSON
    embeddings: Vec<Vec<f32>>   → models.data JSON (or separate blob table if huge)
    photo_count: usize          → models.data JSON
    tags: Vec<Tag>              → tags table via models.metadata_id
}

CREATES RECORD IN MODELS TABLE
Full standalone entity
```

### Album Model

```
Album {
    id: Uuid                    → models.uuid
    name: String                → models.data JSON
    photo_ids: Vec<Uuid>        → models.data JSON + model_entry junction
    tags: Vec<Tag>              → tags table via models.metadata_id
}

CREATES RECORD IN MODELS TABLE
Can be in collections
Can be tagged
```

---

## Custom Fields: Where They Go

**The Rule:**
- Entry-backed models → `user_metadata.custom_data.{extension_id}`
- Standalone models → `models.data` JSON directly

```rust
// Entry-backed (Photo)
#[model]
struct Photo {
    #[entry] file: Entry,

    #[custom_field]  // → user_metadata.custom_data.photos.place_id
    place_id: Option<Uuid>,
}

// Standalone (Person)
#[model]
struct Person {
    // All fields go in models.data - no "custom" vs "regular"
    name: Option<String>,      // → models.data.name
    photo_count: usize,        // → models.data.photo_count
}
```

---

## Sync Behavior

```rust
#[model]
#[sync_strategy = "shared"]  // Model-level default
struct Person {
    #[sync(shared, conflict = "last_writer_wins")]
    name: Option<String>,

    #[sync(device_owned)]  // Override: this field is device-local
    local_note: String,
}
```

**Stored as:**
```sql
-- Shared fields in models.data
data = '{"name":"Alice","photo_count":42}'  -- Syncs via HLC

-- Device-owned fields in separate column?
-- OR: Store all in data but track sync strategy per-field in model schema
```

**Simpler approach:**
```rust
// Just use model-level sync strategy
#[sync_strategy = "shared"]  // Whole model syncs as shared
struct Person { ... }

#[sync_strategy = "device_owned"]  // Whole model is device-local
struct LocalCache { ... }
```

Per-field sync adds complexity. Start with model-level.

---

## Proposed Final Schema

```sql
-- Standalone extension models
CREATE TABLE models (
    id INTEGER PRIMARY KEY,
    uuid BLOB UNIQUE NOT NULL,
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    data TEXT NOT NULL,                  -- JSON
    metadata_id INTEGER NOT NULL,        -- Tags, notes, etc.
    sync_strategy INTEGER NOT NULL,      -- Whole-model strategy
    hlc_timestamp TEXT,
    device_uuid BLOB,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Optional relationships between models and entries
CREATE TABLE model_entry (
    model_uuid BLOB NOT NULL,
    entry_uuid BLOB NOT NULL,
    relationship_type TEXT,
    PRIMARY KEY (model_uuid, entry_uuid)
);

-- Collections support both
CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL,
    entry_uuid BLOB,     -- Reference to entry
    model_uuid BLOB,     -- Reference to model
    position INTEGER,
    CHECK (entry_uuid IS NOT NULL OR model_uuid IS NOT NULL)
);

-- Tags already work (both have metadata_id)
-- No changes to tags/metadata_tag needed
```

---

## SDK Model Definition (Final)

```rust
// Entry-backed model
#[model]
struct Photo {
    #[entry(filter = "*.jpg")]
    file: Entry,  // Links to existing entry

    // Custom data in user_metadata.custom_data.photos
    #[custom_field]
    identified_people: Vec<Uuid>,
}
// NO RECORD IN MODELS TABLE

// Standalone model
#[model]
#[sync_strategy = "shared"]
struct Person {
    id: Uuid,
    name: Option<String>,
    embeddings: Vec<Vec<f32>>,

    // Optional: Link to entry (e.g., profile photo)
    #[linked_entry]
    profile_photo: Option<Uuid>,
}
// CREATES RECORD IN MODELS TABLE
// Can optionally link to entries via model_entry

// Hybrid model
#[model]
#[sync_strategy = "shared"]
struct Album {
    id: Uuid,
    name: String,

    // References entries, creates model_entry records
    #[references(Entry, relationship = "contains")]
    photo_ids: Vec<Uuid>,
}
// CREATES RECORD IN MODELS TABLE
// + model_entry records for each photo
```

---

## Tags & Collections: Universal

```rust
// Tag a photo (entry-backed model)
ctx.vdfs().add_tag(photo.file.metadata_id(), "#vacation");

// Tag a person (standalone model)
ctx.vdfs().add_tag(person.metadata_id(), "#family");

// Tag an album
ctx.vdfs().add_tag(album.metadata_id(), "#2025");

// Create collection with both
let collection = Collection {
    name: "Vacation 2025",
    items: vec![
        CollectionItem::Entry(photo1_uuid),
        CollectionItem::Entry(photo2_uuid),
        CollectionItem::Model(album_uuid),
        CollectionItem::Model(person_uuid),
    ],
};
```

---

## The Beautiful Part

**Everything participates in VDFS primitives:**

| Feature | Entry | Model | How It Works |
|---------|-------|-------|--------------|
| **Tags** | ✅ | ✅ | Both have `metadata_id` → `user_metadata` |
| **Collections** | ✅ | ✅ | `collection_items` references both |
| **Search** | ✅ FTS5 | ✅ FTS5 | Separate virtual tables |
| **Sync** | ✅ Device | ✅ Shared/Device | Different strategies |
| **Custom Data** | ✅ JSON | ✅ JSON | `user_metadata.custom_data` vs `models.data` |
| **Sidecars** | ✅ VSS | ❌ N/A | Sidecars for file derivatives only |

---

## Where Custom Fields Go (Answered)

### Entry-Backed Models (Photo)

```rust
#[model]
struct Photo {
    #[entry] file: Entry,

    #[custom_field]
    place_id: Option<Uuid>,  // → user_metadata.custom_data.photos.place_id
}
```

**Storage:** `user_metadata.custom_data` (existing, namespaced by extension)

### Standalone Models (Person)

```rust
#[model]
struct Person {
    name: Option<String>,  // → models.data.name
    photo_count: usize,    // → models.data.photo_count
}
```

**Storage:** `models.data` JSON directly (all fields, no "custom" distinction)

---

## Migration Path

### Database Migration

```sql
-- Add models table
CREATE TABLE models (...);
CREATE TABLE model_entry (...);

-- Extend collections
ALTER TABLE collection_items ADD COLUMN model_uuid BLOB REFERENCES models(uuid);

-- Add constraint
ALTER TABLE collection_items ADD CONSTRAINT check_item_type
    CHECK (entry_uuid IS NOT NULL OR model_uuid IS NOT NULL);
```

### Core Implementation

```rust
// core/src/infra/db/models.rs (NEW)
pub struct ModelStore {
    db: DatabaseConnection,
}

impl ModelStore {
    pub async fn create<T: ExtensionModel>(
        &self,
        extension_id: &str,
        model: &T,
    ) -> Result<()> {
        // Serialize to JSON
        let data = serde_json::to_string(model)?;

        // Create user_metadata first (for tags)
        let metadata_id = self.create_metadata().await?;

        // Insert model
        self.db.insert_model(
            model.uuid(),
            extension_id,
            T::MODEL_TYPE,
            data,
            metadata_id,
            T::SYNC_STRATEGY,
        ).await?;

        // Create model_entry links if needed
        if let Some(entries) = model.linked_entries() {
            for (entry_uuid, rel_type) in entries {
                self.link_to_entry(model.uuid(), entry_uuid, rel_type).await?;
            }
        }

        Ok(())
    }
}
```

---

## Final Answer to Your Questions

### 1. **Sidecars for virtual models?**
❌ No. Sidecars are for file derivatives (thumbnails, OCR). Virtual model data goes in `models.data` JSON.

If embeddings are huge, use separate blob table:
```sql
CREATE TABLE model_blobs (
    model_uuid BLOB PRIMARY KEY,
    blob_data BLOB,  -- Binary compressed
);
```

### 2. **Where is virtual entry data stored?**
In `models.data` JSON column. Not in UserMetadata - that's for tags/notes only.

### 3. **Sync strategy for virtual models?**
Model-level attribute: `#[sync_strategy = "shared"]` or `"device_owned"`.
Stored in `models.sync_strategy` column.
Uses existing HLC sync for shared, existing device-owned logic otherwise.

---

## Clean Architecture Summary

```
VDFS Entities:
├── Entry (files/directories)
│   ├── Has: location, path, content
│   ├── Has: metadata_id
│   ├── Has: sidecars (file derivatives)
│   └── Sync: Device-owned
│
└── Model (extension data)
    ├── Has: extension_id, model_type
    ├── Has: metadata_id
    ├── Has: data (JSON)
    ├── Optional: Links to entries via model_entry
    └── Sync: Configurable (shared or device-owned)

Universal Features:
├── Tags (via metadata_id)
├── Collections (via union table)
├── Search (FTS5 on both)
└── Sync (existing infrastructure)
```

**This is clean, extensible, and uses VDFS primitives for both files and virtual data.**

Thoughts?
