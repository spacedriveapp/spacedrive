# Thumbnail System Design for Core-New

## Executive Summary

This document outlines the design for a modern thumbnail generation system for Spacedrive core-new, learning from the original implementation while leveraging core-new's improved job system architecture. The system will run as a separate job alongside indexing operations, providing efficient, scalable thumbnail generation with support for a wide variety of media formats.

## Design Principles

1. **Separation of Concerns**: Thumbnail generation is independent from indexing, allowing for flexible scheduling and processing
2. **Job-Based Architecture**: Leverages core-new's simplified job system with minimal boilerplate
3. **Content-Addressable Storage**: Uses CAS IDs from indexing for efficient deduplication and storage
4. **Library-Scoped Storage**: Thumbnails are stored within each library directory for portability
5. **Progressive Enhancement**: Thumbnails can be generated after initial indexing completes
6. **Format Flexibility**: Support for multiple thumbnail sizes and formats
7. **Efficient Storage**: Sharded directory structure for performance at scale

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                  Job System                     │
│  ┌─────────────────┐  ┌─────────────────────┐   │
│  │   IndexerJob    │  │  ThumbnailJob       │   │
│  │                 │  │                     │   │
│  │ • File Discovery│  │ • Queue Processing  │   │
│  │ • Metadata      │  │ • Image Generation  │   │
│  │ • CAS ID Gen    │  │ • WebP Encoding     │   │
│  └─────────────────┘  └─────────────────────┘   │
│           │                       │             │
│           └───────────────────────┘             │
│                       │                         │
└───────────────────────┼─────────────────────────┘
                        │
┌───────────────────────┼─────────────────────────┐
│                Library Directory                │
│                       │                         │
│  ┌─────────────────┐  │  ┌─────────────────────┐│
│  │   database.db   │  │  │     thumbnails/     ││
│  │                 │  │  │                     ││
│  │ • Entries       │  │  │ • Version Control   ││
│  │ • Content IDs   │  │  │ • Sharded Storage   ││
│  │ • Metadata      │  │  │ • WebP Files        ││
│  └─────────────────┘  │  └─────────────────────┘│
└───────────────────────┼─────────────────────────┘
                        │
                 ┌─────────────┐
                 │  File System │
                 │             │
                 │ • Media Files│
                 │ • Raw Images │
                 │ • Videos     │
                 │ • Documents  │
                 └─────────────┘
```

## Job System Integration

### ThumbnailJob Structure

Building on core-new's job system, the thumbnail job follows the established patterns:

```rust
use crate::infrastructure::jobs::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJob {
    /// Entry IDs to process for thumbnails
    pub entry_ids: Vec<Uuid>,

    /// Target thumbnail sizes
    pub sizes: Vec<u32>,

    /// Quality setting (0-100)
    pub quality: u8,

    /// Whether to regenerate existing thumbnails
    pub regenerate: bool,

    /// Batch size for processing
    pub batch_size: usize,

    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<ThumbnailState>,

    // Performance tracking
    #[serde(skip)]
    metrics: ThumbnailMetrics,
}

impl Job for ThumbnailJob {
    const NAME: &'static str = "thumbnail_generation";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Generate thumbnails for media files");
}

#[async_trait::async_trait]
impl JobHandler for ThumbnailJob {
    type Output = ThumbnailOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Implementation details below
    }
}
```

### Job Execution Phases

The thumbnail job operates in distinct phases, similar to the indexer:

1. **Discovery Phase**: Query database for entries that need thumbnails
2. **Processing Phase**: Generate thumbnails in batches
3. **Cleanup Phase**: Remove orphaned thumbnails (optional)

### Integration with IndexerJob

The thumbnail job can be triggered in several ways:

1. **Standalone Execution**: Run independently on existing entries
2. **Post-Indexing**: Automatically triggered after indexer completes <- Thought: I think we should make the add location create a "queued" job that is a child of the main job, I don't think the job system supports this yet so you might need to add it.
3. **Scheduled**: Periodic generation for new content
4. **On-Demand**: User-triggered regeneration

## Storage Architecture

### Directory Structure

Following the original system's proven approach with improvements:

```
<library_path>/thumbnails/
├── version.txt                    # Version for migration support
├── metadata.json                  # Thumbnail generation settings
└── <cas_id[0..2]>/               # 2-char sharding (00-ff)
    └── <cas_id[2..4]>/           # 2-char sub-sharding (00-ff)
        ├── <cas_id>_128.webp     # 128px thumbnail
        ├── <cas_id>_256.webp     # 256px thumbnail
        └── <cas_id>_512.webp     # 512px thumbnail
```

**Sharding Benefits:**

- 256 top-level directories (00-ff)
- 256 second-level directories per top-level
- 65,536 total shard directories
- Excellent filesystem performance even with millions of thumbnails

### Thumbnail Naming Convention

- **Format**: `<cas_id>_<size>.webp`
- **Size**: Pixel dimension (e.g., 128, 256, 512)
- **Extension**: Always `.webp` for consistency and efficiency

### Version Control

```json
{
	"version": 2,
	"quality": 85,
	"sizes": [128, 256, 512],
	"created_at": "2024-01-01T00:00:00Z",
	"updated_at": "2024-01-01T00:00:00Z",
	"total_thumbnails": 15432,
	"storage_used_bytes": 256789012
}
```

## Job Implementation Details

### ThumbnailJob Core Logic

```rust
#[async_trait::async_trait]
impl JobHandler for ThumbnailJob {
    type Output = ThumbnailOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Initialize or restore state
        let state = self.get_or_create_state(&ctx).await?;

        // Discovery phase: Find entries needing thumbnails
        if state.phase == ThumbnailPhase::Discovery {
            self.run_discovery_phase(state, &ctx).await?;
        }

        // Processing phase: Generate thumbnails in batches
        if state.phase == ThumbnailPhase::Processing {
            self.run_processing_phase(state, &ctx).await?;
        }

        // Cleanup phase: Remove orphaned thumbnails
        if state.phase == ThumbnailPhase::Cleanup {
            self.run_cleanup_phase(state, &ctx).await?;
        }

        Ok(ThumbnailOutput {
            generated_count: state.generated_count,
            skipped_count: state.skipped_count,
            error_count: state.error_count,
            total_size_bytes: state.total_size_bytes,
            duration: state.started_at.elapsed(),
            metrics: self.metrics.clone(),
        })
    }
}
```

### Discovery Phase Implementation

```rust
async fn run_discovery_phase(
    &mut self,
    state: &mut ThumbnailState,
    ctx: &JobContext<'_>,
) -> JobResult<()> {
    ctx.progress(Progress::indeterminate("Discovering files for thumbnail generation"));

    // Query database for entries that need thumbnails
    let query = format!(
        "SELECT id, cas_id, mime_type, size, relative_path
         FROM entries
         WHERE content_id IS NOT NULL
         AND mime_type LIKE 'image/%'
         OR mime_type LIKE 'video/%'
         OR mime_type = 'application/pdf'
         ORDER BY size DESC"  // Process larger files first for better progress feedback
    );

    let entries = ctx.library_db().query_all(&query).await?;

    // Filter entries that already have thumbnails (unless regenerating)
    for entry in entries {
        let cas_id = entry.cas_id;

        if !self.regenerate && self.has_all_thumbnails(&cas_id, ctx.library()).await? {
            state.skipped_count += 1;
            continue;
        }

        state.pending_entries.push(ThumbnailEntry {
            entry_id: entry.id,
            cas_id,
            mime_type: entry.mime_type,
            file_size: entry.size,
            relative_path: entry.relative_path,
        });
    }

    // Create batches for processing
    state.batches = state.pending_entries
        .chunks(self.batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    state.phase = ThumbnailPhase::Processing;
    ctx.progress(Progress::count(0, state.batches.len()));

    Ok(())
}
```

### Processing Phase Implementation

```rust
async fn run_processing_phase(
    &mut self,
    state: &mut ThumbnailState,
    ctx: &JobContext<'_>,
) -> JobResult<()> {
    for (batch_idx, batch) in state.batches.iter().enumerate() {
        ctx.check_interrupt().await?;

        // Process batch concurrently
        let tasks: Vec<_> = batch.iter().map(|entry| {
            self.generate_thumbnail_for_entry(entry, ctx.library())
        }).collect();

        let results = futures::future::join_all(tasks).await;

        // Process results
        for result in results {
            match result {
                Ok(thumbnail_info) => {
                    state.generated_count += 1;
                    state.total_size_bytes += thumbnail_info.size_bytes;
                }
                Err(e) => {
                    state.error_count += 1;
                    ctx.add_non_critical_error(e);
                }
            }
        }

        // Update progress
        ctx.progress(Progress::count(batch_idx + 1, state.batches.len()));

        // Checkpoint every 10 batches
        if batch_idx % 10 == 0 {
            ctx.checkpoint().await?;
        }
    }

    state.phase = ThumbnailPhase::Cleanup;
    Ok(())
}
```

## Thumbnail Generation Engine

### Multi-Format Support

The thumbnail generator supports multiple media types:

```rust
pub enum ThumbnailGenerator {
    Image(ImageGenerator),
    Video(VideoGenerator),
    Document(DocumentGenerator),
}

impl ThumbnailGenerator {
    pub async fn generate(
        &self,
        source_path: &Path,
        output_path: &Path,
        size: u32,
        quality: u8,
    ) -> Result<ThumbnailInfo> {
        match self {
            Self::Image(gen) => gen.generate(source_path, output_path, size, quality).await,
            Self::Video(gen) => gen.generate(source_path, output_path, size, quality).await,
            Self::Document(gen) => gen.generate(source_path, output_path, size, quality).await,
        }
    }
}
```

### Image Generator

```rust
pub struct ImageGenerator;

impl ImageGenerator {
    pub async fn generate(
        &self,
        source_path: &Path,
        output_path: &Path,
        size: u32,
        quality: u8,
    ) -> Result<ThumbnailInfo> {
        // Open and decode image
        let img = image::open(source_path)?;

        // Apply EXIF orientation correction
        let img = self.apply_orientation(img, source_path)?;

        // Calculate target dimensions maintaining aspect ratio
        let (target_width, target_height) = self.calculate_dimensions(
            img.width(), img.height(), size
        );

        // Resize using high-quality algorithm
        let thumbnail = img.resize(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        );

        // Encode as WebP
        let webp_data = self.encode_webp(thumbnail, quality)?;

        // Write to file
        tokio::fs::write(output_path, webp_data).await?;

        Ok(ThumbnailInfo {
            size_bytes: webp_data.len(),
            dimensions: (target_width, target_height),
            format: "webp".to_string(),
        })
    }
}
```

### Video Generator

```rust
pub struct VideoGenerator {
    ffmpeg_path: PathBuf,
}

impl VideoGenerator {
    pub async fn generate(
        &self,
        source_path: &Path,
        output_path: &Path,
        size: u32,
        quality: u8,
    ) -> Result<ThumbnailInfo> {
        // Extract frame at 10% of video duration
        let frame_time = self.calculate_frame_time(source_path).await?;

        // Generate thumbnail using FFmpeg
        let mut cmd = tokio::process::Command::new(&self.ffmpeg_path);
        cmd.args([
            "-i", source_path.to_str().unwrap(),
            "-ss", &frame_time,
            "-vframes", "1",
            "-vf", &format!("scale={}:{}:force_original_aspect_ratio=decrease", size, size),
            "-quality", &quality.to_string(),
            "-f", "webp",
            output_path.to_str().unwrap(),
        ]);

        let output = cmd.output().await?;

        if !output.status.success() {
            return Err(ThumbnailError::VideoProcessing(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        let file_size = tokio::fs::metadata(output_path).await?.len();

        Ok(ThumbnailInfo {
            size_bytes: file_size as usize,
            dimensions: (size, size), // Actual dimensions would need to be extracted
            format: "webp".to_string(),
        })
    }
}
```

## Database Integration

### Entry Model Extensions

The existing entry model already supports thumbnails through content identity:

```rust
// No changes needed to entry model - CAS ID provides the link
pub struct Entry {
    pub id: i32,
    pub content_id: Option<i32>,  // Links to content_identity table
    // ... other fields
}

pub struct ContentIdentity {
    pub id: i32,
    pub cas_id: String,  // Used as thumbnail identifier
    // ... other fields
}
```

### Thumbnail Queries

```sql
-- Find entries needing thumbnails
SELECT e.id, ci.cas_id, e.mime_type, e.size, e.relative_path
FROM entries e
JOIN content_identity ci ON e.content_id = ci.id
WHERE ci.cas_id IS NOT NULL
  AND (e.mime_type LIKE 'image/%'
       OR e.mime_type LIKE 'video/%'
       OR e.mime_type = 'application/pdf')
  AND NOT EXISTS (
    SELECT 1 FROM thumbnails t WHERE t.cas_id = ci.cas_id
  );

-- Track thumbnail generation status (optional optimization)
CREATE TABLE IF NOT EXISTS thumbnails (
    cas_id TEXT PRIMARY KEY,
    sizes TEXT NOT NULL,  -- JSON array of generated sizes
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    file_size INTEGER NOT NULL
);
```

## Performance Considerations

### Concurrent Processing

- **Batch Size**: Process 10-50 entries per batch for optimal memory usage
- **Concurrency**: Generate 2-4 thumbnails simultaneously (CPU-bound)
- **Memory Management**: Load/unload images per batch to control memory usage
- **Interruption**: Support graceful cancellation between batches

### Storage Optimization

- **Deduplication**: Use CAS IDs to avoid generating duplicate thumbnails
- **Compression**: WebP format provides excellent compression ratios
- **Sharding**: Two-level directory sharding for filesystem efficiency
- **Cleanup**: Remove orphaned thumbnails during maintenance

### Error Handling

- **Non-Critical Errors**: Continue processing other files when one fails
- **Retry Logic**: Retry failed generations with exponential backoff
- **Format Fallback**: Fall back to different thumbnail sizes if generation fails
- **Logging**: Detailed error logging for debugging

## API Integration

### Library Extensions

Add thumbnail methods to the Library struct:

```rust
impl Library {
    /// Check if thumbnail exists for a CAS ID
    pub async fn has_thumbnail(&self, cas_id: &str, size: u32) -> bool {
        self.thumbnail_path(cas_id, size).exists()
    }

    /// Get thumbnail path for a CAS ID and size
    pub fn thumbnail_path(&self, cas_id: &str, size: u32) -> PathBuf {
        if cas_id.len() < 4 {
            return self.thumbnails_dir().join(format!("{}_{}.webp", cas_id, size));
        }

        let shard1 = &cas_id[0..2];
        let shard2 = &cas_id[2..4];

        self.thumbnails_dir()
            .join(shard1)
            .join(shard2)
            .join(format!("{}_{}.webp", cas_id, size))
    }

    /// Get thumbnail data
    pub async fn get_thumbnail(&self, cas_id: &str, size: u32) -> Result<Vec<u8>> {
        let path = self.thumbnail_path(cas_id, size);
        Ok(tokio::fs::read(path).await?)
    }

    /// Start thumbnail generation job
    pub async fn generate_thumbnails(&self, entry_ids: Vec<Uuid>) -> Result<JobHandle> {
        let job = ThumbnailJob::new(entry_ids);
        self.jobs().dispatch(job).await
    }
}
```

## Migration Strategy

### From Original System

1. **Version Detection**: Check existing thumbnail version in `version.txt`
2. **Directory Migration**: Move thumbnails to new sharded structure if needed
3. **Metadata Migration**: Convert existing metadata to new format
4. **Gradual Rollout**: Generate new thumbnails alongside existing ones

### Configuration Migration

```rust
impl LibraryConfig {
    /// Migrate thumbnail settings from original system
    pub fn migrate_thumbnail_settings(&mut self, original_config: &OriginalConfig) {
        self.settings.thumbnail_quality = original_config.thumbnail_quality.unwrap_or(85);
        self.settings.thumbnail_sizes = original_config.thumbnail_sizes
            .unwrap_or_else(|| vec![128, 256, 512]);
    }
}
```

## Implementation Timeline

### Phase 1: Core Infrastructure (1-2 weeks)

- [ ] Create `ThumbnailJob` with basic structure
- [ ] Implement thumbnail storage utilities in `Library`
- [ ] Add thumbnail generation engine for images
- [ ] Basic job execution and progress reporting

### Phase 2: Multi-Format Support (1-2 weeks)

- [ ] Add video thumbnail support with FFmpeg
- [ ] Add PDF thumbnail support
- [ ] Implement batch processing and concurrency
- [ ] Add error handling and retry logic

### Phase 3: Integration and Optimization (1 week)

- [ ] Integrate with indexer job triggering
- [ ] Add database optimization tables
- [ ] Implement cleanup and maintenance
- [ ] Performance testing and tuning

### Phase 4: Advanced Features (1 week)

- [ ] Scheduled thumbnail generation
- [ ] Thumbnail regeneration commands
- [ ] Migration from original system
- [ ] API endpoints for serving thumbnails

## Benefits Over Original System

1. **Cleaner Architecture**: Separated from indexing, follows job system patterns
2. **Better Resumability**: Leverages core-new's checkpoint system
3. **Improved Performance**: Batch processing and better concurrency control
4. **Enhanced Error Handling**: Non-critical errors don't stop the entire job
5. **Greater Flexibility**: Multiple trigger mechanisms and processing modes
6. **Library-Scoped**: Thumbnails are contained within library directories
7. **Modern Dependencies**: Uses maintained crates and modern Rust patterns

## Conclusion

This thumbnail system design provides a robust, scalable solution for thumbnail generation in core-new. By leveraging the improved job system architecture and maintaining compatibility with the original storage approach, it offers the best of both worlds: modern implementation patterns with proven storage efficiency.

The system is designed to be:

- **Maintainable**: Clear separation of concerns and minimal boilerplate
- **Performant**: Efficient storage, batch processing, and concurrent generation
- **Reliable**: Comprehensive error handling and resumable operations
- **Extensible**: Easy to add new formats and processing options

This design positions the thumbnail system as a first-class citizen in the core-new architecture while maintaining the performance and reliability expectations established by the original implementation.
