# Model Download Integration Strategy

## Problem

Speech-to-text and OCR jobs need models to be downloaded before they can run. How do we handle this?

## Solution: Auto-Download with Job Chaining

### Pattern

```rust
// In SpeechToTextJob::run()

async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
    // Check if model exists
    let data_dir = get_data_dir();
    let manager = WhisperModelManager::new(&data_dir);
    let model = WhisperModel::from_str(&self.config.model).unwrap();

    if !manager.is_downloaded(&model).await {
        ctx.log(format!("Model {} not found, downloading...", model.display_name()));

        // Dispatch download job
        let download_job = ModelDownloadJob::for_whisper_model(model, data_dir.clone());
        let download_handle = ctx.library().jobs().dispatch(download_job).await?;

        ctx.log(format!("Waiting for model download (job {})...", download_handle.id()));

        // Wait for download to complete
        // Option A: Poll job status
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            let job_info = ctx.library().jobs().get_job_info(download_handle.id()).await?;

            match job_info.status {
                JobStatus::Completed => {
                    ctx.log("Model download complete");
                    break;
                }
                JobStatus::Failed | JobStatus::Cancelled => {
                    return Err(JobError::execution(
                        "Model download failed or was cancelled"
                    ));
                }
                _ => {
                    // Still running/queued
                    ctx.check_interrupt().await?; // Allow this job to be paused too
                }
            }
        }

        // Option B: Subscribe to job events (better!)
        let mut event_rx = ctx.library().event_bus().subscribe();

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    if let Ok(Event::JobCompleted { job_id, .. }) = event {
                        if job_id == download_handle.id().to_string() {
                            ctx.log("Model download complete");
                            break;
                        }
                    }
                    if let Ok(Event::JobFailed { job_id, .. }) = event {
                        if job_id == download_handle.id().to_string() {
                            return Err(JobError::execution("Model download failed"));
                        }
                    }
                }
                _ = ctx.check_interrupt() => {
                    return Err(JobError::cancelled());
                }
            }
        }
    }

    // Model is now guaranteed to exist, continue with transcription
    // ... rest of job logic
}
```

## Better Pattern: Lazy Download in Discovery Phase

```rust
// In SpeechToTextJob

impl SpeechToTextJob {
    async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        ctx.log("Starting discovery phase");

        // Check if model exists BEFORE discovering files
        let data_dir = get_data_dir();
        let manager = WhisperModelManager::new(&data_dir);
        let model = WhisperModel::from_str(&self.config.model)
            .ok_or_else(|| JobError::execution("Invalid model"))?;

        if !manager.is_downloaded(&model).await {
            ctx.log(format!(
                "Model {} required but not downloaded. Dispatching download job...",
                model.display_name()
            ));

            // Dispatch download job
            let download_job = ModelDownloadJob::for_whisper_model(
                model,
                data_dir.clone()
            );

            let download_handle = ctx.library().jobs().dispatch(download_job).await?;

            ctx.log(format!(
                "Waiting for model download to complete (job: {})...",
                download_handle.id()
            ));

            // Wait for completion
            self.wait_for_job_completion(ctx, download_handle.id()).await?;

            ctx.log("Model download complete, continuing discovery");
        }

        // Now discover files
        // ... existing discovery logic
    }

    async fn wait_for_job_completion(
        &self,
        ctx: &JobContext<'_>,
        job_id: &JobId,
    ) -> JobResult<()> {
        use crate::infra::event::Event;

        let mut event_rx = ctx.library().event_bus().subscribe();

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Ok(Event::JobCompleted { job_id: id, .. }) if id == job_id.to_string() => {
                            return Ok(());
                        }
                        Ok(Event::JobFailed { job_id: id, error, .. }) if id == job_id.to_string() => {
                            return Err(JobError::execution(format!(
                                "Dependency job failed: {}",
                                error
                            )));
                        }
                        Ok(Event::JobCancelled { job_id: id, .. }) if id == job_id.to_string() => {
                            return Err(JobError::cancelled());
                        }
                        _ => {} // Ignore other events
                    }
                }
                _ = ctx.check_interrupt() => {
                    return Err(JobError::cancelled());
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    // Timeout after 30 seconds of no events
                    return Err(JobError::execution("Timeout waiting for model download"));
                }
            }
        }
    }
}
```

## Even Better: Model Check Helper

Create a reusable helper:

```rust
// core/src/ops/models/ensure.rs

use crate::infra::job::prelude::*;
use crate::ops::models::{ModelDownloadJob, WhisperModel, WhisperModelManager};
use std::path::Path;

/// Ensure a whisper model is downloaded, downloading it if necessary
///
/// This should be called in the discovery phase of any job that needs a model.
/// It will:
/// 1. Check if model exists
/// 2. If not, dispatch download job and wait for completion
/// 3. Return when model is ready
///
/// The calling job can be paused/cancelled during the wait.
pub async fn ensure_whisper_model(
    ctx: &JobContext<'_>,
    model: WhisperModel,
    data_dir: &Path,
) -> JobResult<std::path::PathBuf> {
    let manager = WhisperModelManager::new(data_dir);
    let model_path = manager.get_model_path(&model);

    // Check if already downloaded
    if manager.is_downloaded(&model).await {
        ctx.log(format!("Using existing model: {}", model.display_name()));
        return Ok(model_path);
    }

    // Need to download
    ctx.log(format!(
        "Model {} not found. Starting download ({} MB)...",
        model.display_name(),
        model.size_bytes() / 1024 / 1024
    ));

    // Create and dispatch download job
    let download_job = ModelDownloadJob::for_whisper_model(model, data_dir.to_path_buf());
    let handle = ctx.library().jobs().dispatch(download_job).await?;

    ctx.log(format!("Download job dispatched: {}", handle.id()));

    // Wait for completion
    wait_for_job(ctx, handle.id()).await?;

    ctx.log("Model download complete");

    Ok(model_path)
}

/// Wait for a job to complete, fail, or be cancelled
async fn wait_for_job(ctx: &JobContext<'_>, job_id: &crate::infra::job::types::JobId) -> JobResult<()> {
    use crate::infra::event::Event;

    let mut event_rx = ctx.library().event_bus().subscribe();
    let job_id_str = job_id.to_string();

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(Event::JobCompleted { job_id, .. }) if job_id == job_id_str => {
                        return Ok(());
                    }
                    Ok(Event::JobFailed { job_id, error, .. }) if job_id == job_id_str => {
                        return Err(JobError::execution(format!("Model download failed: {}", error)));
                    }
                    Ok(Event::JobCancelled { job_id, .. }) if job_id == job_id_str => {
                        return Err(JobError::execution("Model download was cancelled"));
                    }
                    _ => {} // Ignore other events
                }
            }
            _ = ctx.check_interrupt() => {
                // User paused/cancelled THIS job while waiting
                // The download job will continue in the background
                return Err(JobError::cancelled());
            }
        }
    }
}
```

## Usage in Speech Job

```rust
// core/src/ops/media/speech/job.rs

impl SpeechToTextJob {
    async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        ctx.log("Starting speech-to-text discovery");

        // Ensure model is downloaded FIRST
        let data_dir = crate::config::default_data_dir()?;
        let model = WhisperModel::from_str(&self.config.model)
            .ok_or_else(|| JobError::execution("Invalid model"))?;

        // This will download if needed and wait
        let model_path = crate::ops::models::ensure_whisper_model(
            ctx,
            model,
            &data_dir,
        ).await?;

        ctx.log(format!("Model ready at: {}", model_path.display()));

        // Now discover files
        // ... existing discovery logic
    }
}
```

## Usage in OCR Job

```rust
// core/src/ops/media/ocr/job.rs

impl OcrJob {
    async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
        // Ensure tesseract language data is downloaded
        for lang in &self.config.languages {
            ctx.log(format!("Checking tesseract data for language: {}", lang));

            // TODO: Implement tesseract model management
            // let path = crate::ops::models::ensure_tesseract_language(
            //     ctx,
            //     lang,
            //     &data_dir,
            // ).await?;
        }

        // Discover files...
    }
}
```

## Frontend Integration

### Automatic Download

When user triggers speech job, it automatically downloads the model:

```typescript
const transcribeLocation = useLibraryMutation('jobs.dispatch');

// User clicks "Transcribe All Videos"
transcribeLocation.mutate({
  job_type: 'speech_to_text',
  config: {
    location_id: currentLocation.id,
    model: 'base', // Will auto-download if missing
    language: 'en'
  }
});

// User sees:
// 1. Job queued
// 2. "Model base not found. Starting download..."
// 3. Download progress (via nested job)
// 4. "Model download complete"
// 5. Transcription begins
```

### Explicit Download

User can also download models proactively:

```typescript
const downloadModel = useCoreMutation('models.whisper.download');
const { data: models } = useCoreQuery({
  type: 'models.whisper.list',
  input: {}
});

// Settings page
{models.map(model => (
  <div key={model.id}>
    <span>{model.name} ({model.size_mb} MB)</span>
    {model.downloaded ? (
      <Badge>Downloaded</Badge>
    ) : (
      <Button onClick={() => downloadModel.mutate({ model: model.id })}>
        Download
      </Button>
    )}
  </div>
))}
```

## Job Status UI

The download job shows progress in the jobs panel:

```
Jobs:
├─ Speech-to-Text Transcription (Queued)
│  └─ Model Download (Running)
│     ├─ Downloading whisper-base.bin
│     └─ 45 MB / 142 MB (31%)
```

## Error Handling

### Model Download Fails

```rust
// Speech job discovers model missing
// Dispatches download job
// Download fails (network error)
// Speech job receives JobFailed event
// Speech job fails with: "Model download failed: Network timeout"
// User sees clear error message
```

### User Cancels During Download

```rust
// Speech job waiting for model download
// User cancels speech job
// Speech job returns JobError::cancelled()
// Download job CONTINUES in background (reusable for next attempt)
```

### Model Download Paused

```rust
// User pauses download job
// Speech job keeps waiting (polls status)
// User resumes download
// Download completes
// Speech job continues automatically
```

## Recommendations

**Implement `ensure_whisper_model()` helper:**
- Put in `core/src/ops/models/ensure.rs`
- Reusable across all jobs
- Handles download + wait logic
- Clean error handling

**Speech/OCR jobs just call it:**
```rust
let model_path = ensure_whisper_model(ctx, model, &data_dir).await?;
// Model guaranteed to exist now
```

**Benefits:**
- No duplicate download logic
- Jobs don't need to know about job dispatching
- Works with pause/resume/cancel
- Clear progress in UI (nested jobs)
- Downloads are resumable
- Downloads are reusable (multiple jobs can wait for same download)

**Want me to implement the `ensure_whisper_model()` helper now?**
