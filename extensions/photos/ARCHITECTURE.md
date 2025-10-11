# Photos Extension Architecture

## Overview

This extension demonstrates **every major SDK feature** through a real-world use case: intelligent photo management mirroring Apple Photos and Google Photos.

---

## Core Design Patterns

### Pattern 1: Core Does Generic, Extension Does Specialized

**Core extracts:**
- ✅ EXIF metadata (camera, GPS, date)
- ✅ Thumbnails (for quick preview)
- ✅ Basic embeddings (for semantic search)

**Photos extension adds:**
- 🎭 Face detection (RetinaFace model)
- 📍 Place clustering (from GPS + reverse geocoding)
- 🎬 Scene classification (ResNet50)
- 🌟 Aesthetic scoring
- 🧠 Person identification with clustering

### Pattern 2: On-Demand, User-Scoped Analysis

**Not automatic:**
```rust
// ❌ Photos does NOT analyze every screenshot automatically
// ✅ User enables Photos on specific locations:
//    [x] /My Photos
//    [x] /Family Vacation 2025
//    [ ] /Work Documents (not relevant)
```

**User triggers:**
1. Install Photos extension
2. Grant access to photo locations
3. Click "Analyze for Faces" button
4. Job processes photos in background
5. Results appear progressively

### Pattern 3: Sidecar → Tags → Search

**Step 1: Detailed sidecar**
```json
// .sdlibrary/sidecars/content/{uuid}/extensions/photos/faces.json
{
  "model_version": "retinaface_v1",
  "faces": [
    {
      "bbox": { "x": 0.2, "y": 0.3, "width": 0.1, "height": 0.15 },
      "confidence": 0.95,
      "embedding": [0.123, 0.456, ...], // 512 dims
      "identified_as": "person_uuid_123"
    }
  ]
}
```

**Step 2: Searchable tags**
```sql
-- Core tags table
INSERT INTO metadata_tag VALUES (photo_metadata_id, tag_id);
-- Tag: "#person:Alice"
```

**Step 3: User searches**
```
User types: "photos of alice"
  ↓
Query: tags LIKE '#person:alice'
  ↓
Results: All photos with Alice
```

### Pattern 4: Enum-Based Memory for Multi-Domain Knowledge

```rust
enum PhotoKnowledge {
    FaceCluster { person_id, embeddings, photos },
    PlaceCluster { place_id, center, photos },
    ScenePattern { scene_type, common_times, locations },
}

// Single AssociativeMemory stores all three types
// Enables queries like: "Find places where Alice appears often"
knowledge
    .query()
    .where_variant(PhotoKnowledge::FaceCluster)
    .where_field("person_id", equals(alice_id))
    .and_related_concepts(PhotoKnowledge::PlaceCluster)
    .collect()
```

---

## Data Flow

### User Adds 1000 Photos to Spacedrive

```
1. Core Indexer runs (5 phases)
   - Discovery: Finds 1000 JPGs
   - Processing: Creates Entry records
   - Aggregation: Updates directory stats
   - Content ID: Generates CAS IDs
   - Analysis Queueing: Extracts EXIF, generates thumbnails
   ↓
2. Core emits Event::EntryCreated × 1000
   ↓
3. Photos agent receives events
   - Checks if photos are in granted scope
   - Adds to analysis queue (batches of 50)
   ↓
4. Agent dispatches analyze_photos_batch job
   - detect_faces_in_photo task × 1000 (parallel: 4)
   - Saves faces.json sidecars to VSS
   ↓
5. cluster_faces_into_people task
   - DBSCANgroups similar face embeddings
   - Creates/updates Person models
   ↓
6. generate_face_tags task
   - Reads faces.json sidecars
   - Writes tags to core tag system
   ↓
7. User searches "photos of alice"
   - Core tag query: tags LIKE '#person:alice'
   - Instant results
```

---

## Memory System Usage

### Temporal Memory (Event Timeline)

```rust
history: TemporalMemory<PhotoEvent>

// Stores:
PhotoEvent::PhotoAnalyzed { faces_detected: 2, scene_tags: ["beach"], ... }
PhotoEvent::PersonIdentified { person_id, photo_id, confidence: 0.95 }
PhotoEvent::MomentCreated { moment_id, photo_count: 45, date_range }

// Queries:
memory.history
    .query()
    .where_variant(PhotoEvent::PhotoAnalyzed)
    .since(Duration::days(7))
    .where_field("scene_tags", contains("beach"))
    .collect()
// → "Photos analyzed last week with beach scenes"
```

### Associative Memory (Knowledge Graph)

```rust
knowledge: AssociativeMemory<PhotoKnowledge>

// Stores:
PhotoKnowledge::FaceCluster { person_id, embeddings, photo_ids }
PhotoKnowledge::PlaceCluster { place_id, center, photos }
PhotoKnowledge::ScenePattern { scene_type, typical_times }

// Queries:
memory.knowledge
    .query_similar("vacation photos")
    .where_variant(PhotoKnowledge::PlaceCluster)
    .min_similarity(0.7)
    .top_k(10)
// → "Places semantically similar to 'vacation photos'"

// Cross-domain:
memory.knowledge
    .query()
    .where_field("person_id", equals(alice_id))
    .and_related_concepts(PhotoKnowledge::PlaceCluster)
// → "Places where Alice frequently appears"
```

### Working Memory (Current State)

```rust
plan: WorkingMemory<AnalysisPlan>

// Stores:
AnalysisPlan {
    pending_locations: ["/New Photos"],
    photos_needing_faces: [uuid1, uuid2, ...],
    moments_to_generate: [(start_date, end_date), ...]
}

// Transactional updates:
plan.update(|mut p| {
    p.photos_needing_faces.push(new_photo_id);
    Ok(p)
}).await?
```

---

## AI Model Integration

### Models Used

1. **Face Detection** (RetinaFace, 12MB)
   - Input: Image bytes
   - Output: Bounding boxes + 512-dim embeddings
   - Registered as: `face_detection:photos_v1`

2. **Scene Classification** (ResNet50 Places365, 95MB)
   - Input: Image bytes
   - Output: Scene probabilities (beach, sunset, indoor, etc.)
   - Registered as: `scene_classification:resnet50`

3. **LLM for Titles** (Llama 3 via Ollama, managed separately)
   - Input: Scene tags + location + date
   - Output: Creative moment title
   - Registered as: `llm:local`

### Registration Flow

```rust
#[on_install]
async fn install(ctx: &InstallContext) -> InstallResult<()> {
    // Register face detection
    ctx.models().register(
        "face_detection",
        "photos_v1",
        ModelSource::Download {
            url: "https://models.spacedrive.com/photos/face_v1.onnx",
            sha256: "abc123...",
        }
    ).await?;

    // Register scene classification
    ctx.models().register(
        "scene_classification",
        "resnet50",
        ModelSource::Download {
            url: "https://models.spacedrive.com/photos/scene_v1.onnx",
            sha256: "def456...",
        }
    ).await?;

    Ok(())
}
```

Models stored in: `~/.spacedrive/models/face_detection/photos_v1.onnx`

---

## Permission Scoping

### Extension Requests

```rust
permissions = [
    Permission::ReadEntries,  // Broad request
    Permission::WriteSidecars(kinds = ["faces", "places"]),
    Permission::WriteTags,
    Permission::UseModel(category = "face_detection"),
]
```

### User Grants & Scopes

```
User during setup:
┌───────────────────────────────────┐
│ Photos Extension Permissions      │
├───────────────────────────────────┤
│ ✓ Read image files                │
│ ✓ Detect faces (local AI)         │
│ ✓ Add tags                         │
│                                    │
│ Grant access to:                  │
│ [x] /My Photos                     │
│ [x] /Family Photos                 │
│ [ ] /Work Documents                │
└───────────────────────────────────┘
```

### Runtime Enforcement

Every WASM host function checks:
1. Permission granted? (`WriteTags` ✓)
2. Entry in scope? (`/My Photos/...` ✓)
3. Execute or deny

---

## UI Integration

### Sidebar (from ui_manifest.json)

```
Photos
├── Library    (photo_grid)
├── Albums     (album_grid)
├── People     (person_cluster_grid)
├── Places     (map_view)
├── Moments    (moment_timeline)
└── Favorites  (photo_grid filtered)
```

### Context Menu

- Right-click photo → "Add to Album..."
- Right-click face → "This is..."
- Right-click album → "Set as Cover"

### Toolbar

- Location view → "Analyze for Faces" button
- Location view → "Identify Places" button
- Selection → "Create Moment"

---

## Advanced Features

### Smart Albums (Rule-Based)

```rust
#[model]
struct SmartAlbum {
    name: String,
    rules: Vec<AlbumRule>,  // "scene:beach" AND "person:family"
}

enum AlbumRule {
    HasTag(String),
    HasPerson(PersonId),
    AtPlace(PlaceId),
    DateRange(DateTime<Utc>, DateTime<Utc>),
    SceneType(String),
}

// Automatically updates as photos are tagged
```

### Memory-Based Suggestions

```rust
#[query("suggest featured photos")]
async fn suggest_featured(ctx: &QueryContext<PhotosMind>) -> QueryResult<Vec<Photo>> {
    let memory = ctx.memory().read().await;

    // Find photos with:
    // - High aesthetic score
    // - Multiple people
    // - Taken at interesting places
    // - Not recently featured

    let candidates = memory.history
        .query()
        .where_field("faces_detected", greater_than(2))
        .where_field("location", is_not_null())
        .since(Duration::days(365))
        .limit(100)
        .collect()
        .await?;

    // Rank by diversity and quality
    let featured = rank_by_diversity(candidates);

    Ok(featured)
}
```

---

## This Extension Demonstrates

✅ **Full SDK surface area** - All primitives used
✅ **Real-world complexity** - Matches commercial photo apps
✅ **Core/Extension separation** - Clear boundaries
✅ **User privacy** - Local processing, scoped access
✅ **Progressive enhancement** - Works with partial data
✅ **Durable operations** - All jobs resumable
✅ **Multi-device** - Sync face clusters via CRDT
✅ **AI-native** - Models, prompts, semantic search

**This is the reference implementation for the VDFS SDK.** 🎯

