# Photos Extension for Spacedrive

A comprehensive photo management extension that brings Apple Photos and Google Photos capabilities to Spacedrive.

## Features

### Face Recognition
- **Automatic face detection** using RetinaFace model
- **Face clustering** with DBSCAN algorithm
- **Person identification** with name labeling
- **Face-based search** ("Show me photos of Alice")
- **Cross-device sync** of face clusters

### Place Identification
- **GPS extraction** from EXIF data
- **Geographic clustering** (groups photos within 500m)
- **Reverse geocoding** using AI
- **Place-based search** ("Photos from Paris")
- **Map view** of photo locations

### Moments
- **Automatic moment generation** (time + location clustering)
- **AI-generated titles** ("Summer in Paris")
- **Weekly memories** (scheduled agent task)
- **Moment timeline** view

### ️ Scene Understanding
- **Scene classification** (ResNet50 on Places365)
- **Smart tags** (#beach, #sunset, #food)
- **Scene-based search** ("Photos with sunsets")
- **Quality scoring** (aesthetic assessment)

### Albums & Organization
- **Manual albums** (user-created)
- **Smart albums** (rule-based)
- **Favorites** (special album)
- **Hidden photos** (privacy)
- **Shared albums** (multi-device)

## Architecture

### Models

```rust
Photo       // Links to image file + EXIF + face/scene sidecars
Person      // Face cluster with name and embeddings
Place       // Geographic location with radius
Album       // Collection of photos
Moment      // Time/location-based photo group
```

### Agent Memory

```rust
PhotosMind {
    history: TemporalMemory<PhotoEvent>,     // Analysis timeline
    knowledge: AssociativeMemory<PhotoKnowledge>, // Face/place graph
    plan: WorkingMemory<AnalysisPlan>,       // Pending work
}
```

### Jobs

- `analyze_photos_batch` - Face detection on photos
- `identify_places_in_location` - Place clustering and naming
- `analyze_scenes` - Scene classification
- `create_moments` - Automatic moment generation
- `cluster_faces_into_people` - Face clustering
- `generate_face_tags` - Tag generation from sidecars

### Actions

- `create_album` - User creates album
- `identify_person` - Name a face cluster
- `remove_photo_from_album` - Organize albums
- `hide_photo` - Privacy controls

## Usage

### Installation

1. Install extension from Spacedrive Extension Store
2. Grant permissions to specific photo locations:
   - `/Users/alice/Photos`
   - `/Volumes/External/Family Photos`
3. Extension downloads AI models (~107MB total)
4. User initiates analysis: "Analyze for Faces" button

### Analysis Flow

```
User enables Photos on "/My Photos"
  ↓
Extension dispatches analyze_photos_batch job
  ↓
For each photo:
  - Read EXIF (from Core)
  - Detect faces → save to sidecar
  - Classify scene → save to sidecar
  ↓
Cluster faces into people
  ↓
Generate tags from sidecars
  ↓
User can search "#person:alice" or "photos from beach"
```

### Data Storage

```
~/.spacedrive/
  └── models/
      ├── face_detection/
      │   └── photos_v1.onnx (12MB)
      └── scene_classification/
          └── resnet50.onnx (95MB)

.sdlibrary/
  ├── sidecars/
  │   └── content/{ab}/{cd}/{uuid}/
  │       └── extensions/com.spacedrive.photos/
  │           ├── faces/default.json       # Face detection results
  │           ├── scene/default.json       # Scene classification
  │           └── aesthetics/default.json  # Quality score
  └── extensions/com.spacedrive.photos/
      └── data.db                           # Person, Place, Moment, Album, Photo rows
```

## SDK Features Demonstrated

### Implemented in Example

- `#[extension]` with permissions and dependencies
- `#[model]` for Photo, Person, Place, Album, Moment
- `#[agent]` with lifecycle hooks and event handlers
- `#[agent_memory]` with enum-based Temporal/Associative memory
- `#[job]` and `#[task]` for durable processing
- `#[action]` with preview-execute pattern
- `#[query]` for user searches
- Extension-owned sidecars
- Virtual models with persistence
- User-scoped permissions
- Model registration on install
- AI with Jinja templates
- Tag generation from sidecars
- Custom memory query methods

### What runs today

Under a daemon built with `--features wasm`, `analyze_photos` runs end to end
over a library: it reads each photo through its record, asks for face
detection, and because the core has no inference provider the host answers
`not_available`; the job warns once, skips, and completes, so a later run with
a detector picks the photos up. `core/tests/wasm_extension_test.rs` proves
this against a fixture library.

### Not yet backed by the core

- Inference. There is no provider for face detection, scene classification,
  embeddings or language models, so `analyze_scenes`, `identify_places` and
  `create_moments` have nothing to run on.
- Tags, custom fields and `dispatch_jobs`.
- Agents, actions and queries.
- Model downloads named in `manifest.json`.
- File-kind and preview registration.

## Capabilities Compared

### Apple Photos
- Face recognition and clustering
- Place identification
- Memories
- Albums (manual and smart)
- Favorites and hidden
- Search by person/place/scene
- Map view
- ️ Shared albums (requires P2P implementation)

### Google Photos
- Face grouping
- Place detection
- Search by content
- Albums
- Automatic creations (moments)
- Scene/object detection
- ️ Cloud backup (Spacedrive handles differently)

### Spacedrive Photos Advantages
- **100% local** - No cloud upload required
- **Privacy-first** - Face data never leaves devices
- **Multi-device sync** - Via P2P, not cloud
- **Zero recurring cost** - No subscription needed
- **User-scoped** - Only analyzes chosen locations
- **Extensible** - Open SDK for customization

## Building

```bash
cd extensions/photos
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/photos_extension.wasm ./photos.wasm
```

## License

Same as Spacedrive Core

