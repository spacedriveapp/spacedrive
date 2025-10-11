# Metadata Blobs: Large Data Storage for VDFS

**Problem:** Storing large data (embeddings, large JSON) in `models.data` or `user_metadata.custom_data` bloats queries.

**Solution:** Separate `metadata_blobs` table with content-addressed storage.

---

## Schema Design

```sql
-- Main blob storage (content-addressed for deduplication)
CREATE TABLE metadata_blobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Content addressing (like content_identity)
    blob_hash TEXT UNIQUE NOT NULL,      -- BLAKE3 hash of blob content
    blob_data BLOB NOT NULL,             -- Actual binary/compressed data
    size_bytes INTEGER NOT NULL,

    -- Metadata
    blob_type TEXT NOT NULL,             -- "face_embeddings", "large_json", etc.
    compression TEXT,                    -- "zstd", "gzip", NULL

    -- Usage tracking
    reference_count INTEGER DEFAULT 1,

    -- Timestamps
    created_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_metadata_blobs_hash ON metadata_blobs(blob_hash);
CREATE INDEX idx_metadata_blobs_type ON metadata_blobs(blob_type);

-- References from user_metadata
CREATE TABLE user_metadata_blobs (
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),
    blob_key TEXT NOT NULL,              -- "embeddings", "large_data", etc.
    blob_id INTEGER NOT NULL REFERENCES metadata_blobs(id),

    PRIMARY KEY (metadata_id, blob_key)
);

CREATE INDEX idx_user_metadata_blobs_metadata ON user_metadata_blobs(metadata_id);
CREATE INDEX idx_user_metadata_blobs_blob ON user_metadata_blobs(blob_id);

-- References from models
CREATE TABLE model_blobs (
    model_uuid BLOB NOT NULL REFERENCES models(uuid),
    blob_key TEXT NOT NULL,
    blob_id INTEGER NOT NULL REFERENCES metadata_blobs(id),

    PRIMARY KEY (model_uuid, blob_key)
);

CREATE INDEX idx_model_blobs_model ON model_blobs(model_uuid);
CREATE INDEX idx_model_blobs_blob ON model_blobs(blob_id);
```

---

## How It Works

### Storage Pattern

```rust
// Extension stores large data
#[model]
struct Person {
    id: Uuid,
    name: Option<String>,  // → models.data JSON (lightweight)

    #[blob_data]
    embeddings: Vec<Vec<f32>>,  // → metadata_blobs table (heavy)
}
```

**What happens:**

```sql
-- 1. Hash the embeddings
blob_hash = blake3(serialize(embeddings))

-- 2. Check if blob exists (deduplication!)
SELECT id FROM metadata_blobs WHERE blob_hash = '<hash>'

-- 3a. If exists, increment reference_count
UPDATE metadata_blobs SET reference_count = reference_count + 1

-- 3b. If not, insert new blob
INSERT INTO metadata_blobs (blob_hash, blob_data, blob_type, size_bytes)
VALUES ('<hash>', <compressed_embeddings>, 'face_embeddings', 204800)

-- 4. Link to model
INSERT INTO model_blobs (model_uuid, blob_key, blob_id)
VALUES ('<person_uuid>', 'embeddings', <blob_id>)

-- 5. Model data stays lightweight
INSERT INTO models (uuid, data, ...) VALUES (
    '<person_uuid>',
    '{"name":"Alice","photo_count":42}',  -- No embeddings here!
    ...
)
```

### Retrieval Pattern

```rust
// Extension reads Person model
let person = ctx.vdfs().get_model::<Person>(person_uuid).await?;

// SDK automatically:
// 1. Reads models.data → lightweight fields
// 2. Checks model_blobs for this model_uuid
// 3. Reads metadata_blobs.blob_data for heavy fields
// 4. Deserializes and returns complete Person
```

---

## API Design

### Extension SDK

```rust
#[model]
struct Person {
    id: Uuid,
    name: Option<String>,  // Inline in models.data

    #[blob_data(compression = "zstd")]
    embeddings: Vec<Vec<f32>>,  // Stored separately, auto-loaded
}

// Usage is transparent
let person = ctx.vdfs().get_model::<Person>(uuid).await?;
// person.embeddings automatically loaded from blob
```

### Core Implementation

```rust
// In core/src/infra/db/models.rs

impl ModelStore {
    pub async fn create<T: ExtensionModel>(
        &self,
        extension_id: &str,
        model: &T,
    ) -> Result<()> {
        // Split model into inline and blob fields
        let (inline_data, blob_fields) = model.split_for_storage();

        // Store lightweight data
        let model_data = serde_json::to_string(&inline_data)?;
        let model_uuid = model.uuid();

        // Store heavy blobs separately
        for (blob_key, blob_value) in blob_fields {
            let blob_id = self.store_blob(
                &blob_value,
                blob_type = format!("{}_{}", T::MODEL_TYPE, blob_key),
                compression = Some("zstd"),
            ).await?;

            // Link to model
            self.db.execute(
                "INSERT INTO model_blobs (model_uuid, blob_key, blob_id) VALUES (?, ?, ?)",
                params![model_uuid, blob_key, blob_id]
            ).await?;
        }

        // Insert model record
        self.db.insert_model(model_uuid, extension_id, model_data).await?;

        Ok(())
    }

    async fn store_blob(
        &self,
        data: &[u8],
        blob_type: String,
        compression: Option<&str>,
    ) -> Result<i32> {
        // Compress if requested
        let (final_data, compression_used) = match compression {
            Some("zstd") => (zstd::encode_all(data, 3)?, "zstd"),
            _ => (data.to_vec(), "none"),
        };

        // Hash for deduplication
        let blob_hash = blake3::hash(&final_data).to_hex();

        // Check if exists
        if let Some(existing) = self.db.query_one(
            "SELECT id FROM metadata_blobs WHERE blob_hash = ?",
            params![blob_hash]
        ).await? {
            // Increment reference count
            self.db.execute(
                "UPDATE metadata_blobs SET reference_count = reference_count + 1",
                params![existing.id]
            ).await?;

            return Ok(existing.id);
        }

        // Insert new blob
        let blob_id = self.db.execute(
            "INSERT INTO metadata_blobs (blob_hash, blob_data, blob_type, size_bytes, compression)
             VALUES (?, ?, ?, ?, ?)",
            params![blob_hash, final_data, blob_type, final_data.len(), compression_used]
        ).await?;

        Ok(blob_id)
    }
}
```

---

## Benefits

### 1. **Deduplication**

```rust
// Two people with identical embedding (twins?)
Person { name: "Alice", embeddings: [...] }
Person { name: "Bob", embeddings: [...] }  // Same embeddings!

// Stored as:
metadata_blobs { id: 1, blob_hash: "abc123", blob_data: <embeddings> }
model_blobs { model_uuid: alice_uuid, blob_key: "embeddings", blob_id: 1 }
model_blobs { model_uuid: bob_uuid, blob_key: "embeddings", blob_id: 1 }

// Only one copy of embeddings stored!
```

### 2. **Fast Queries**

```sql
-- Query models without loading heavy blobs
SELECT uuid, data FROM models
WHERE extension_id = 'photos' AND model_type = 'Person'
-- ✅ Fast! data column only has {"name":"Alice","photo_count":42}
-- ✅ Embeddings not loaded until accessed
```

### 3. **Lazy Loading**

```rust
// SDK can lazy-load blobs
let person = ctx.vdfs().get_model::<Person>(uuid).await?;
// ✅ name and photo_count loaded
// ✅ embeddings NOT loaded yet

// Only load when accessed
let embedding = person.embeddings().await?;
// ← Triggers blob fetch from metadata_blobs
```

### 4. **Compression**

```rust
#[blob_data(compression = "zstd", lazy = true)]
embeddings: Vec<Vec<f32>>,

// 512 floats × 100 faces = 204KB uncompressed
// With zstd: ~50KB (4x compression)
```

---

## When to Use Blobs vs Inline JSON

### Inline in `models.data` (Good for)
- ✅ Small data (< 10KB)
- ✅ Frequently queried fields
- ✅ Simple types (strings, numbers, small arrays)
- **Examples:** name, photo_count, dates, UUIDs

### Blob storage (Good for)
- ✅ Large data (> 10KB)
- ✅ Rarely accessed
- ✅ Binary data (embeddings, images, audio)
- ✅ Compressible data
- **Examples:** Face embeddings (200KB), vector indices, cached AI results

---

## Updated Photos Extension

```rust
#[model]
#[scope = "content"]
struct PhotoAnalysis {
    id: Uuid,

    // Inline (frequently used, small)
    scene_tags: Vec<SceneTag>,      // ~1KB
    quality_score: f32,
    identified_people: Vec<Uuid>,

    // Blob storage (large, rarely accessed)
    #[blob_data(compression = "zstd", lazy = true)]
    detected_faces: Vec<FaceDetection>,  // ~50KB (with bbox + embeddings)
}

#[model]
#[scope = "standalone"]
struct Person {
    id: Uuid,

    // Inline
    name: Option<String>,
    photo_count: usize,
    thumbnail_content_id: Option<Uuid>,

    // Blob storage (100+ embeddings = 200KB)
    #[blob_data(compression = "zstd")]
    embeddings: Vec<Vec<f32>>,
}
```

---

## Sync Behavior

**Blobs participate in sync:**

```sql
-- Blob syncs like content_identity (by hash)
metadata_blobs {
    blob_hash: "abc123",  -- Deterministic
    blob_data: <data>,
}

-- Reference syncs with model
model_blobs {
    model_uuid: <synced_person_uuid>,
    blob_key: "embeddings",
    blob_id: → metadata_blobs.blob_hash
}
```

**Process:**
1. Device A creates Person with embeddings
2. Syncs model record (lightweight JSON)
3. Syncs blob reference (blob_hash)
4. Device B receives model → sees blob_hash
5. Device B requests blob if not local
6. Device B stores in its metadata_blobs table

**Deduplication across devices:**
- Same blob_hash → request once, share across models
- Like content_identity for files!

---

## Garbage Collection

```rust
// When model deleted
impl ModelStore {
    pub async fn delete_model(&self, model_uuid: Uuid) -> Result<()> {
        // Delete model record
        self.db.execute("DELETE FROM models WHERE uuid = ?", params![model_uuid]).await?;

        // Decrement blob references
        let blob_ids: Vec<i32> = self.db.query(
            "SELECT blob_id FROM model_blobs WHERE model_uuid = ?",
            params![model_uuid]
        ).await?;

        for blob_id in blob_ids {
            self.db.execute(
                "UPDATE metadata_blobs SET reference_count = reference_count - 1
                 WHERE id = ?",
                params![blob_id]
            ).await?;

            // Delete blob if no more references
            self.db.execute(
                "DELETE FROM metadata_blobs WHERE id = ? AND reference_count = 0",
                params![blob_id]
            ).await?;
        }

        // Delete blob links
        self.db.execute("DELETE FROM model_blobs WHERE model_uuid = ?", params![model_uuid]).await?;

        Ok(())
    }
}
```

---

## Complete Schema (Final)

```sql
-- Extension models (lightweight)
CREATE TABLE models (
    id INTEGER PRIMARY KEY,
    uuid BLOB UNIQUE NOT NULL,
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    data TEXT NOT NULL,  -- Lightweight JSON only

    -- Scoping
    entry_uuid BLOB,
    content_identity_uuid BLOB,
    standalone BOOLEAN DEFAULT 0,

    -- Metadata
    metadata_id INTEGER NOT NULL,

    -- Sync
    sync_strategy INTEGER NOT NULL,
    hlc_timestamp TEXT,
    device_uuid BLOB,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    CHECK (...)
);

-- Large data storage (content-addressed)
CREATE TABLE metadata_blobs (
    id INTEGER PRIMARY KEY,
    blob_hash TEXT UNIQUE NOT NULL,      -- BLAKE3 for deduplication
    blob_data BLOB NOT NULL,
    blob_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    compression TEXT,
    reference_count INTEGER DEFAULT 1,
    created_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL
);

-- Links models to blobs
CREATE TABLE model_blobs (
    model_uuid BLOB NOT NULL,
    blob_key TEXT NOT NULL,              -- "embeddings", "cache", etc.
    blob_id INTEGER NOT NULL,
    PRIMARY KEY (model_uuid, blob_key)
);

-- Links user_metadata to blobs (for entries/content)
CREATE TABLE user_metadata_blobs (
    metadata_id INTEGER NOT NULL,
    blob_key TEXT NOT NULL,
    blob_id INTEGER NOT NULL,
    PRIMARY KEY (metadata_id, blob_key)
);
```

---

## Usage Examples

### Photos Extension: Face Embeddings

```rust
#[model]
#[scope = "standalone"]
struct Person {
    id: Uuid,
    name: Option<String>,        // → models.data (inline, ~50 bytes)
    photo_count: usize,          // → models.data

    #[blob_data(compression = "zstd")]
    embeddings: Vec<Vec<f32>>,   // → metadata_blobs (200KB compressed to 50KB)
}

// Create
let person = Person {
    name: Some("Alice"),
    photo_count: 42,
    embeddings: vec![embedding1, embedding2, ...],  // 100 × 512 floats
};

ctx.vdfs().create_model(person).await?;

// Storage:
// models.data = '{"name":"Alice","photo_count":42}' (fast queries)
// metadata_blobs.blob_data = <compressed_embeddings> (lazy loaded)
// model_blobs = link between them
```

### Content-Scoped: Large Analysis Data

```rust
#[model]
#[scope = "content"]
struct PhotoAnalysis {
    id: Uuid,
    scene_tags: Vec<String>,     // → models.data (small)

    #[blob_data(lazy = true)]
    detected_faces: Vec<FaceDetection>,  // → metadata_blobs (large)
}

// Query without loading faces
let analyses = ctx.vdfs()
    .query_models::<PhotoAnalysis>()
    .select_fields(&["scene_tags"])  // Only load inline data
    .collect()
    .await?;
// ✅ Fast! Blobs not loaded

// Load specific analysis with faces
let analysis = ctx.vdfs()
    .get_model_by_content::<PhotoAnalysis>(content_uuid)
    .await?;
let faces = analysis.detected_faces().await?;  // ← Lazy load blob
```

---

## Performance Benefits

### Before (all in JSON)

```sql
-- Query 10,000 people
SELECT data FROM models WHERE extension_id = 'photos' AND model_type = 'Person'
-- Returns: 10,000 × 200KB = 2GB of embeddings
-- ❌ Slow! Memory intensive!
```

### After (blob separation)

```sql
-- Query 10,000 people
SELECT data FROM models WHERE extension_id = 'photos' AND model_type = 'Person'
-- Returns: 10,000 × 50 bytes = 500KB
-- ✅ Fast! Only load embeddings when needed
```

---

## Deduplication Example

```
Extension creates 1000 PhotoAnalysis models
Each has detected_faces: Vec<FaceDetection>

Scenario: 500 photos have NO faces detected
Result: Vec<FaceDetection> = []

Without blobs:
  1000 × empty array in JSON = still stored 1000 times

With blobs:
  blob_hash("[]") = "hash_empty"
  metadata_blobs: ONE row for empty array
  model_blobs: 500 references to same blob

Storage savings: 500 × (small blob overhead) instead of 500 × full serialization
```

---

## Sync Considerations

**Blobs sync by hash (like content_identity):**

```
Device A creates Person with embeddings
  ↓
Computes blob_hash = "abc123"
  ↓
Syncs model record (lightweight)
  ↓
Syncs blob reference: { blob_key: "embeddings", blob_hash: "abc123" }
  ↓
Device B receives model
  ↓
Checks local metadata_blobs for blob_hash "abc123"
  ↓
If missing: Request blob from Device A (P2P transfer)
  ↓
Stores in local metadata_blobs
  ↓
Links via model_blobs
```

**Lazy sync option:**
- Sync model metadata immediately
- Sync blobs on-demand (when accessed)
- Good for large libraries

---

## Migration from Existing Code

```sql
-- Add blob tables
CREATE TABLE metadata_blobs (...);
CREATE TABLE model_blobs (...);
CREATE TABLE user_metadata_blobs (...);

-- No migration needed for existing data
-- New models use blobs automatically
-- Old data stays in JSON (works fine for small data)
```

---

## Complete Storage Map (Updated)

```
Person Model:
├── models table
│   ├── uuid: person_uuid
│   ├── data: '{"name":"Alice","photo_count":42}' (lightweight)
│   └── metadata_id: → user_metadata (for tags)
│
├── model_blobs table
│   └── (person_uuid, "embeddings") → blob_id
│
└── metadata_blobs table
    └── { id: blob_id, blob_hash: "abc123", blob_data: <compressed> }

PhotoAnalysis Model (content-scoped):
├── models table
│   ├── content_identity_uuid: photo_content_uuid
│   ├── data: '{"scene_tags":["beach"],"quality_score":0.9}'
│   └── metadata_id: → user_metadata
│
├── model_blobs table
│   └── (analysis_uuid, "detected_faces") → blob_id
│
└── metadata_blobs table
    └── { id: blob_id, blob_data: <faces_with_coords_and_embeddings> }
```

---

## Recommendation

**YES - Add this system!**

**Benefits:**
- ✅ Fast queries (don't load heavy data)
- ✅ Deduplication (by content hash)
- ✅ Compression (zstd for embeddings)
- ✅ Lazy loading (load on access)
- ✅ Clean separation (lightweight vs heavy)
- ✅ Works with sync (hash-based)

**Minimal complexity:**
- Just 3 new tables
- Content-addressed like existing content_identity
- Extensions use transparently via `#[blob_data]`

**This is the right design.** 🎯


