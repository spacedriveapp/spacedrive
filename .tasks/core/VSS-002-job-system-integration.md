---
id: VSS-002
title: "Sidecar Generation Job System Integration"
status: To Do
assignee: jamiepine
parent: CORE-008
priority: High
tags: [vss, jobs, generation, indexing]
whitepaper: "Section 4.1.5"
last_updated: 2025-11-01
related_tasks: [CORE-008, JOB-000]
dependencies: [VSS-001]
---

## Description

Implement automatic sidecar generation by integrating with the job system. This completes the "generate once, use everywhere" workflow for derivative data.

Currently, `SidecarManager::enqueue_generation()` creates pending records but never dispatches actual generation jobs (TODO at `core/src/service/sidecar_manager.rs:273`).

See `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md` Section "Generation Pipeline" for complete specification.

## Implementation Files

- `core/src/service/sidecar_manager.rs` - Remove TODO, dispatch jobs
- `core/src/ops/media/thumbnail/job.rs` - Already exists, needs sidecar integration
- `core/src/ops/media/ocr/job.rs` - New job for OCR extraction
- `core/src/ops/media/transcript/job.rs` - New job for transcription
- `core/src/ops/indexing/phases/intelligence_queueing.rs` - New indexing phase

## Tasks

### Thumbnail Generation Job
- [ ] Integrate existing `ThumbnailJob` with SidecarManager
- [ ] Implement idempotent generation (fast-path if exists)
- [ ] Record completion in sidecars table
- [ ] Update sidecar_availability for local device
- [ ] Support multiple variants in one job
- [ ] Add configurable quality/size settings

### OCR Extraction Job
- [ ] Create `OcrExtractionJob` struct
- [ ] Integrate Tesseract or alternative OCR library
- [ ] Extract text from images and PDFs
- [ ] Write results as JSON sidecar
- [ ] Handle multi-language documents
- [ ] Add confidence scoring

### Transcript Generation Job
- [ ] Create `TranscriptGenerationJob` struct
- [ ] Integrate Whisper or alternative STT library
- [ ] Extract audio from video files
- [ ] Generate timestamped transcripts
- [ ] Write results as JSON sidecar
- [ ] Support multiple languages

### Intelligence Queueing Phase
- [ ] Create new indexing phase after Content Identification
- [ ] Determine which sidecars to generate based on file type
- [ ] Enqueue appropriate generation jobs
- [ ] Make phase configurable (enable/disable by kind)
- [ ] Respect device-specific generation policies

### Job Dispatch Integration
- [ ] Replace TODO in `SidecarManager::enqueue_generation()`
- [ ] Dispatch appropriate job based on kind
- [ ] Pass content_uuid, variant, format to job
- [ ] Handle job failures and retries
- [ ] Implement job priority (thumbnails > proxies)

## Acceptance Criteria

### Automatic Generation
- [ ] Images automatically get thumbnails after indexing
- [ ] Documents automatically get OCR extraction
- [ ] Videos automatically get transcripts (if enabled)
- [ ] Generation happens asynchronously, non-blocking

### Job Behavior
- [ ] Jobs are idempotent (check before generating)
- [ ] Jobs record completion in database
- [ ] Jobs update availability for local device
- [ ] Failed jobs can be retried
- [ ] Jobs respect device-specific policies

### Integration
- [ ] Intelligence queueing phase hooks into indexer
- [ ] Phase runs after content identification
- [ ] Phase is configurable per location
- [ ] Existing files can trigger regeneration on-demand

### Performance
- [ ] Thumbnail generation: <500ms per image
- [ ] OCR extraction: <2s per document
- [ ] Transcript generation: <1x realtime for audio/video
- [ ] Batch processing for efficiency

## Implementation Notes

### Job Contract

All generation jobs must follow this contract:

```rust
#[derive(Job)]
pub struct SidecarGenerationJob {
    pub content_uuid: Uuid,
    pub kind: SidecarKind,
    pub variant: SidecarVariant,
    pub config: JobConfig,
}

impl SidecarGenerationJob {
    async fn execute(&self, ctx: JobContext) -> Result<JobOutput> {
        // 1. Fast-path: check if sidecar already exists
        let path = ctx.sidecar_manager.compute_path(...)?;
        if fs::exists(&path.absolute_path).await? {
            return Ok(JobOutput::AlreadyExists);
        }

        // 2. Find source file for this content
        let source = ctx.find_entry_by_content_uuid(&self.content_uuid).await?;

        // 3. Generate sidecar
        let sidecar_data = self.generate(source).await?;

        // 4. Write to deterministic path
        fs::create_dir_all(path.parent()).await?;
        fs::write(&path.absolute_path, sidecar_data).await?;

        // 5. Record in database
        ctx.sidecar_manager.record_sidecar(...).await?;

        Ok(JobOutput::Generated { size })
    }
}
```

### Indexing Integration

```rust
impl IndexingPipeline {
    async fn run_phases(&self, entry: Entry) -> Result<()> {
        // Existing phases
        self.discovery_phase(entry).await?;
        self.processing_phase(entry).await?;
        self.aggregation_phase(entry).await?;
        self.content_identification_phase(entry).await?;

        // NEW: Intelligence queueing phase
        if let Some(content_uuid) = entry.content_uuid {
            self.intelligence_queueing_phase(content_uuid, &entry.file_type).await?;
        }

        Ok(())
    }

    async fn intelligence_queueing_phase(
        &self,
        content_uuid: Uuid,
        file_type: &FileType,
    ) -> Result<()> {
        let specs = self.compute_sidecar_specs(file_type);

        for spec in specs {
            self.sidecar_manager.enqueue_generation(
                &self.library,
                &content_uuid,
                &spec.kind,
                &spec.variant,
                &spec.format,
            ).await?;
        }

        Ok(())
    }
}
```

### Device-Specific Policies

```rust
pub struct DeviceGenerationPolicy {
    pub device_type: DeviceType,
    pub enabled_kinds: HashSet<SidecarKind>,
    pub variants: HashMap<SidecarKind, Vec<SidecarVariant>>,
}

impl DeviceGenerationPolicy {
    pub fn for_mobile() -> Self {
        Self {
            device_type: DeviceType::Mobile,
            enabled_kinds: hashset![SidecarKind::Thumb],
            variants: hashmap! {
                SidecarKind::Thumb => vec!["grid@2x", "icon"],
            },
        }
    }

    pub fn for_desktop() -> Self {
        Self {
            device_type: DeviceType::Desktop,
            enabled_kinds: hashset![
                SidecarKind::Thumb,
                SidecarKind::Ocr,
                SidecarKind::Transcript,
                SidecarKind::Embeddings,
            ],
            variants: hashmap! {
                SidecarKind::Thumb => vec!["grid@2x", "detail@1x", "grid@3x"],
                SidecarKind::Ocr => vec!["default"],
                SidecarKind::Transcript => vec!["default"],
                SidecarKind::Embeddings => vec!["all-MiniLM-L6-v2"],
            },
        }
    }
}
```

## Timeline

Estimated: 1 week focused work

- Day 1-2: Thumbnail job integration and testing
- Day 3: OCR extraction job implementation
- Day 4: Transcript generation job (stub, full impl later)
- Day 5: Intelligence queueing phase and indexer hook
- Day 6-7: Testing, device policies, documentation

## Dependencies

Requires VSS-001 (SdPath integration) to be complete for full testing, but can be developed in parallel.
