# Extension Models Schema - Final Design

**Key Insight:** UserMetadata already supports `content_identity_uuid` scoping! Extension models should use the same pattern.

---

## Current Schema (What Exists)

### The Dual-Scoping System

```rust
// UserMetadata can be scoped to EITHER:
user_metadata {
    entry_uuid: Option<Uuid>,            // THIS file at THIS path (device-specific)
    content_identity_uuid: Option<Uuid>, // THIS content (device-independent)

    tags, notes, favorite, custom_data...
}
```

**Example:**
```
Device A: /Photos/vacation.jpg  → Entry UUID: aaa
Device B: /Pictures/vacation.jpg → Entry UUID: bbb
Both point to: ContentIdentity UUID: ccc (same BLAKE3 hash)

// Tag the CONTENT (applies to both)
UserMetadata { content_identity_uuid: ccc, tags: ["#vacation"] }

// Tag just Device A's copy
UserMetadata { entry_uuid: aaa, tags: ["#backup-needed"] }
```

---

## Proposed Schema: Models Table

```sql
CREATE TABLE models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,

    -- Model identity
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,

    -- Data
    data TEXT NOT NULL,  -- JSON

    -- Scoping (same pattern as UserMetadata!)
    -- Exactly ONE of these is set:
    entry_uuid BLOB,             -- Scoped to specific entry (device-specific)
    content_identity_uuid BLOB,  -- Scoped to content (device-independent)
    standalone BOOLEAN,          -- True if not scoped to entry/content

    -- Metadata (for tags, collections)
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),

    -- Sync
    sync_strategy INTEGER NOT NULL,  -- 0=DeviceOwned, 1=Shared
    hlc_timestamp TEXT,
    device_uuid BLOB,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    CHECK (
        (entry_uuid IS NOT NULL AND content_identity_uuid IS NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NOT NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NULL AND standalone = 1)
    )
);

CREATE INDEX idx_models_uuid ON models(uuid);
CREATE INDEX idx_models_extension ON models(extension_id, model_type);
CREATE INDEX idx_models_entry ON models(entry_uuid);
CREATE INDEX idx_models_content ON models(content_identity_uuid);
CREATE INDEX idx_models_metadata ON models(metadata_id);
```

---

## Three Model Types

### Type 1: Content-Scoped (Photos Extension)

**USE CASE:** Photo enrichment that follows the content, not the path.

```rust
#[model(version = "1.0.0")]
#[scope = "content"]  // Scoped to content_identity
#[sync_strategy = "shared"]
struct PhotoAnalysis {
    id: Uuid,

    // Stored in models.data
    detected_faces: Vec<FaceDetection>,
    scene_tags: Vec<String>,
    quality_score: f32,
    identified_people: Vec<Uuid>,  // Person model UUIDs

    // This model is SCOPED to a content_identity
    // Multiple entries (same photo, different paths) → one PhotoAnalysis
}
```

**Storage:**
```sql
INSERT INTO models (
    uuid,
    extension_id,
    model_type,
    content_identity_uuid,  -- ← Scoped to content!
    standalone,
    data,
    metadata_id,
    sync_strategy
) VALUES (
    '<model_uuid>',
    'photos',
    'PhotoAnalysis',
    '<content_uuid>',       -- The unique photo content
    0,
    '{"detected_faces":[...],"scene_tags":["beach"],...}',
    123,
    1  -- Shared
);
```

**Query pattern:**
```rust
// Get PhotoAnalysis for this content
let content_uuid = entry.content_uuid();  // From any entry pointing to this content
let analysis = ctx.vdfs()
    .get_model_by_content::<PhotoAnalysis>(content_uuid)
    .await?;

// Works regardless of which device or path you're looking at!
```

### Type 2: Standalone (Person, Album, Place)

**USE CASE:** Not tied to any file/content.

```rust
#[model(version = "1.0.0")]
#[scope = "standalone"]
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
INSERT INTO models (
    uuid,
    extension_id,
    model_type,
    entry_uuid,
    content_identity_uuid,
    standalone,  -- ← TRUE
    data,
    metadata_id,
    sync_strategy
) VALUES (
    '<person_uuid>',
    'photos',
    'Person',
    NULL,
    NULL,
    1,  -- Standalone
    '{"name":"Alice","embeddings":[[...]],"photo_count":42}',
    456,
    1  -- Shared
);
```

### Type 3: Entry-Scoped (Rare, but Possible)

**USE CASE:** Data specific to THIS file at THIS path (e.g., local processing state).

```rust
#[model]
#[scope = "entry"]
#[sync_strategy = "device_owned"]
struct LocalPhotoCache {
    id: Uuid,
    processing_state: String,
    local_edits: Vec<Edit>,
}
```

**Storage:**
```sql
-- Scoped to specific entry (device-specific)
models.entry_uuid = '<entry_uuid>'
models.standalone = 0
```

---

## Photo Model: The Right Way

```rust
#[model(version = "1.0.0")]
#[scope = "content"]  // ← KEY: Scoped to content_identity!
#[sync_strategy = "shared"]
struct PhotoAnalysis {
    id: Uuid,

    // All enrichment data
    detected_faces: Vec<FaceDetection>,
    scene_tags: Vec<SceneTag>,
    quality_score: f32,
    identified_people: Vec<Uuid>,
    place_id: Option<Uuid>,
    moment_id: Option<Uuid>,

    // Can have tags!
    // Via models.metadata_id → user_metadata
}
```

**SDK Usage:**
```rust
// Create PhotoAnalysis for a content
let entry = ctx.vdfs().get_entry(uuid).await?;
let content_uuid = entry.content_uuid().ok_or("No content ID")?;

let analysis = PhotoAnalysis {
    id: Uuid::new_v4(),
    detected_faces: faces,
    scene_tags: scenes,
    // ...
};

// Attaches to content_identity, not entry
ctx.vdfs()
    .create_model_for_content(content_uuid, analysis)
    .await?;

// Later, from ANY entry pointing to this content:
let entry_on_device_b = ...;
let analysis = ctx.vdfs()
    .get_model_by_content::<PhotoAnalysis>(entry_on_device_b.content_uuid())
    .await?;
// ✅ Same analysis, regardless of device/path!
```

---

## Complete Storage Map

```
Device A:
  Entry { uuid: aaa, path: "/Photos/img.jpg", content_id: → ccc }

Device B:
  Entry { uuid: bbb, path: "/Pictures/img.jpg", content_id: → ccc }

Shared Content:
  ContentIdentity { uuid: ccc, content_hash: "blake3_abc123" }

Extension Models:
  models {
    uuid: model_1,
    extension_id: "photos",
    model_type: "PhotoAnalysis",
    content_identity_uuid: ccc,  ← Scoped to content!
    data: '{"detected_faces":[...],"identified_people":["person_1"]}'
    metadata_id: 111
  }

  models {
    uuid: person_1,
    extension_id: "photos",
    model_type: "Person",
    standalone: true,  ← Not tied to any file
    data: '{"name":"Alice","embeddings":[[...]]}'
    metadata_id: 222
  }

Tags (work for both):
  user_metadata { id: 111, content_identity_uuid: ccc }  ← Tags the content
  metadata_tag { metadata_id: 111, tag_id: tag_vacation }

  user_metadata { id: 222, NULL, NULL }  ← Tags the person model
  metadata_tag { metadata_id: 222, tag_id: tag_family }
```

---

## Query Patterns (Final)

### Get Photo Analysis for Content

```rust
// From any entry (any device, any path)
let entry = ctx.vdfs().get_entry(some_uuid).await?;
let content_uuid = entry.content_uuid()?;

// Get content-scoped model
let analysis = ctx.vdfs()
    .get_model_by_content::<PhotoAnalysis>(content_uuid)
    .await?;

// ✅ Works even if you're looking at a different copy of the photo
```

### Find Photos of Person

```rust
// Find all PhotoAnalysis models that reference this person
let photo_models = ctx.vdfs()
    .query_models::<PhotoAnalysis>()
    .where_json_field("identified_people", contains(person_uuid))
    .collect()
    .await?;

// Get the actual content UUIDs
let content_uuids: Vec<Uuid> = photo_models.iter()
    .map(|model| model.content_identity_uuid())
    .collect();

// Find entries (any device) for these contents
let entries = ctx.vdfs()
    .query_entries()
    .where_content_id_in(content_uuids)
    .on_this_device()  // Filter to accessible paths
    .collect()
    .await?;
```

### Tag Content-Level

```rust
// Tag applies to the CONTENT (all copies)
let content_uuid = entry.content_uuid()?;
ctx.vdfs()
    .add_tag_to_content(content_uuid, "#vacation")
    .await?;

// Under the hood:
// 1. Find or create user_metadata with content_identity_uuid = content_uuid
// 2. Add tag to that metadata
// 3. Syncs to all devices
// 4. All entries pointing to this content show the tag
```

---

## Collections: Reference Both

```sql
CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL,

    -- Can reference entry, content, or model
    entry_uuid BLOB,
    content_uuid BLOB,
    model_uuid BLOB,

    position INTEGER,

    CHECK (
        (entry_uuid IS NOT NULL AND content_uuid IS NULL AND model_uuid IS NULL) OR
        (entry_uuid IS NULL AND content_uuid IS NOT NULL AND model_uuid IS NULL) OR
        (entry_uuid IS NULL AND content_uuid IS NULL AND model_uuid IS NOT NULL)
    )
);
```

**Usage:**
```rust
Collection {
    items: vec![
        CollectionItem::Content(photo_content_uuid),  // The photo content
        CollectionItem::Model(album_uuid),            // An album
        CollectionItem::Model(person_uuid),           // A person
    ]
}
```

---

## Updated Photos Extension Models

```rust
// Content-scoped model (attaches to photos)
#[model(version = "1.0.0")]
#[scope = "content"]
#[sync_strategy = "shared"]
struct PhotoAnalysis {
    id: Uuid,
    detected_faces: Vec<FaceDetection>,
    scene_tags: Vec<SceneTag>,
    identified_people: Vec<Uuid>,  // References Person models
    place_id: Option<Uuid>,        // References Place model
}

// Standalone models
#[model]
#[scope = "standalone"]
#[sync_strategy = "shared"]
struct Person {
    id: Uuid,
    name: Option<String>,
    embeddings: Vec<Vec<f32>>,
}

#[model]
#[scope = "standalone"]
#[sync_strategy = "shared"]
struct Album {
    id: Uuid,
    name: String,
    content_ids: Vec<Uuid>,  // References content_identity UUIDs!
}
```

---

## Schema Changes Needed

```sql
-- NEW TABLE
CREATE TABLE models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    data TEXT NOT NULL,

    -- Scoping (one of three)
    entry_uuid BLOB REFERENCES entries(uuid),
    content_identity_uuid BLOB REFERENCES content_identities(uuid),
    standalone BOOLEAN DEFAULT 0,

    -- Metadata (for tags/collections)
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),

    -- Sync
    sync_strategy INTEGER NOT NULL,
    hlc_timestamp TEXT,
    device_uuid BLOB,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    CHECK (
        (entry_uuid IS NOT NULL AND content_identity_uuid IS NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NOT NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NULL AND standalone = 1)
    )
);

-- EXTEND (existing table)
ALTER TABLE collection_items ADD COLUMN content_uuid BLOB;
ALTER TABLE collection_items ADD COLUMN model_uuid BLOB;
ALTER TABLE collection_items ADD CONSTRAINT check_item
    CHECK (entry_uuid IS NOT NULL OR content_uuid IS NOT NULL OR model_uuid IS NOT NULL);
```

**This is bulletproof!** Content-scoped models survive device changes, standalone models are independent, and everything uses the same tag/collection infrastructure.

Want me to update the Photos extension with this correct architecture?
