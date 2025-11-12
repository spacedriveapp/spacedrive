# OCR and Speech-to-Text System - Implementation Complete

## What's Built

### 1. OCR (Optical Character Recognition)
**Module:** `core/src/ops/media/ocr/`

**Files Created:**
- `mod.rs` - Module interface with `extract_text_from_file()` and filetype integration
- `processor.rs` - `OcrProcessor` for atomic text extraction
- `job.rs` - `OcrJob` for batch OCR processing
- `action.rs` - `ExtractTextAction` for UI-triggered extraction

**Features:**
- Extract text from images (PNG, JPEG, TIFF, BMP, GIF, WebP)
- Extract text from PDFs
- Store extracted text in `content_identity.text_content`
- Configurable languages (e.g., ["eng", "spa", "fra"])
- Configurable confidence threshold
- Integration with filetype system (uses ContentKind)
- Tesseract integration (placeholder - needs library)

### 2. Speech-to-Text (Audio/Video Transcription)
**Module:** `core/src/ops/media/speech/`

**Files Created:**
- `mod.rs` - Module interface with `transcribe_audio_file()` and filetype integration
- `processor.rs` - `SpeechToTextProcessor` for atomic transcription
- `job.rs` - `SpeechToTextJob` for batch transcription
- `action.rs` - `TranscribeAudioAction` for UI-triggered transcription

**Features:**
- Transcribe audio files (MP3, WAV, FLAC, OGG, etc.)
- Transcribe video files (MP4, WebM, QuickTime, MKV)
- Generate .srt subtitle files as sidecars
- Configurable whisper model (tiny, base, small, medium, large)
- Auto-detect language or specify explicitly
- Integration with filetype system (uses ContentKind)
- Whisper.rs integration (placeholder - needs library)

### 3. Processor Integration
Both processors are integrated into:
- **Responder** - Can run on watcher events (disabled by default - expensive)
- **LocationProcessorConfig** - Configurable per-location
- **UI Actions** - Manual triggering from Inspector

## Configuration

### Default Location Config

```json
{
  "watcher_processors": [
    {
      "processor_type": "content_hash",
      "enabled": true
    },
    {
      "processor_type": "thumbnail",
      "enabled": true,
      "settings": {
        "variants": ["grid@1x", "grid@2x"],
        "quality": 80
      }
    },
    {
      "processor_type": "ocr",
      "enabled": false,
      "settings": {
        "languages": ["eng"],
        "min_confidence": 0.6
      }
    },
    {
      "processor_type": "speech_to_text",
      "enabled": false,
      "settings": {
        "model": "base",
        "language": null
      }
    }
  ]
}
```

## Usage

### From Frontend (Manual)

**Extract Text from Image/PDF:**
```typescript
const extractText = useLibraryMutation('media.ocr.extract');

extractText.mutate({
  entry_uuid: selectedFile.id,
  languages: ["eng"],
  force: false
});
```

**Transcribe Audio/Video:**
```typescript
const transcribe = useLibraryMutation('media.speech.transcribe');

transcribe.mutate({
  entry_uuid: selectedFile.id,
  model: "base", // or "small", "medium", "large"
  language: null // Auto-detect
});
```

### Batch Processing (Jobs)

**Run OCR on entire location:**
```typescript
const runOcr = useLibraryMutation('jobs.dispatch');

runOcr.mutate({
  job_type: "ocr",
  config: {
    location_id: locationId,
    languages: ["eng", "spa"],
    min_confidence: 0.7,
    reprocess: false
  }
});
```

**Run Speech-to-Text on library:**
```typescript
runOcr.mutate({
  job_type: "speech_to_text",
  config: {
    location_id: null, // All locations
    model: "small",
    language: "en",
    reprocess: false
  }
});
```

## Implementation TODOs

### OCR Integration
**Add dependency:**
```toml
# core/Cargo.toml
[dependencies]
tesseract = { version = "0.14", optional = true }

[features]
tesseract = ["dep:tesseract"]
```

**Implement:**
```rust
// core/src/ops/media/ocr/mod.rs

#[cfg(feature = "tesseract")]
pub async fn extract_text_from_file(source_path: &Path, languages: &[String]) -> Result<String> {
    use tesseract::Tesseract;

    let source = source_path.to_path_buf();
    let langs = languages.join("+");

    tokio::task::spawn_blocking(move || {
        let mut tess = Tesseract::new(None, Some(&langs))?;
        tess.set_image(&source.to_string_lossy())?;
        tess.set_page_seg_mode(tesseract::PageSegMode::Auto);
        let text = tess.get_text()?;
        Ok(text)
    }).await?
}
```

### Whisper Integration
**Add dependency:**
```toml
# core/Cargo.toml
[dependencies]
whisper-rs = { version = "0.11", optional = true }
symphonia = "0.5" # For audio decoding

[features]
whisper = ["dep:whisper-rs"]
```

**Implement:**
```rust
// core/src/ops/media/speech/mod.rs

#[cfg(feature = "whisper")]
pub async fn transcribe_audio_file(
    source_path: &Path,
    model: &str,
    language: Option<&str>,
) -> Result<String> {
    use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};

    let source = source_path.to_path_buf();
    let model_path = format!("models/ggml-{}.bin", model);
    let lang = language.map(|s| s.to_string());

    tokio::task::spawn_blocking(move || {
        // Load model (consider caching this globally!)
        let ctx = WhisperContext::new(&model_path)?;

        // Decode audio file to PCM samples
        let audio_samples = decode_audio_file(&source)?;

        // Set up transcription params
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if let Some(l) = lang {
            params.set_language(Some(&l));
        }
        params.set_print_timestamps(true);

        // Run transcription
        ctx.full(params, &audio_samples)?;

        // Convert to SRT format
        let mut srt = String::new();
        for i in 0..ctx.full_n_segments() {
            let start = ctx.full_get_segment_t0(i) as f64 / 100.0;
            let end = ctx.full_get_segment_t1(i) as f64 / 100.0;
            let text = ctx.full_get_segment_text(i)?;

            srt.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                format_timestamp(start),
                format_timestamp(end),
                text
            ));
        }

        Ok(srt)
    }).await?
}

fn decode_audio_file(path: &Path) -> Result<Vec<f32>> {
    // Use symphonia to decode audio to 16kHz mono PCM
    // (Whisper expects 16kHz mono audio)
    todo!("Implement audio decoding with symphonia")
}

fn format_timestamp(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 1000.0) as u32;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}
```

## Architecture

### Processor Flow
```
Single File (UI/Watcher)
  ↓
OcrProcessor.process() OR SpeechToTextProcessor.process()
  ↓
extract_text_from_file() OR transcribe_audio_file()
  ↓
Store in content_identity.text_content OR create sidecar
  ↓
Emit ResourceChanged event
```

### Job Flow
```
Batch Processing
  ↓
OcrJob.run() OR SpeechToTextJob.run()
  ↓
Discovery Phase: Find all eligible files
  ↓
Processing Phase: Loop through entries
  ↓
  For each entry: Call processor.process()
  ↓
  Checkpoint every N files
  ↓
Complete Phase: Emit summary
```

## Filetype Integration

Both processors use the `FileTypeRegistry` to determine file support:

**OCR:** Supports `ContentKind::Image` and `ContentKind::Document`
**Speech:** Supports `ContentKind::Audio` and `ContentKind::Video`

This is more maintainable than hardcoded MIME lists - when new file types are added to the registry, they automatically work with processors!

## Testing

### Unit Tests (TODO)
```rust
#[tokio::test]
async fn test_ocr_processor() {
    let processor = OcrProcessor::new(library);
    let entry = ProcessorEntry { /* PNG file */ };
    let result = processor.process(&ctx, &entry).await.unwrap();
    assert!(result.success);
    // Check content_identity.text_content was updated
}
```

### Integration Tests
1. Create test image with known text
2. Run OcrProcessor
3. Verify extracted text matches
4. Verify stored in database

### Manual Tests
1. Take screenshot with text
2. Enable OCR in location config
3. Check database for extracted text
4. Call `media.ocr.extract` action from frontend

## Future Enhancements

### Model Management
- Download whisper models on demand
- Model size selection UI
- Model caching and lifecycle

### Advanced OCR
- Layout analysis
- Table detection
- Handwriting recognition
- Multi-column text

### Advanced Transcription
- Speaker diarization
- Punctuation restoration
- Custom vocabulary
- Timestamp alignment

### Performance
- GPU acceleration for whisper
- Parallel batch processing
- Incremental processing for large files

## Next Steps

1. **Add tesseract-rs dependency** and implement OCR
2. **Add whisper-rs dependency** and implement transcription
3. **Add audio decoding** with symphonia
4. **Build UI** for location processor configuration
5. **Add Inspector buttons** for "Extract Text" and "Transcribe"
6. **Test with real files**

---

**Status:** Architecture Complete, Library Integration Pending
**Files Created:** 10 new files
**Lines Added:** ~1,200 lines
**Ready For:** Library integration and testing
