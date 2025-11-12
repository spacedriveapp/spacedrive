# OCR and Speech-to-Text Implementation Plan

## Overview

Complete implementation plan for tesseract-rs OCR and whisper.cpp speech-to-text, including model management, audio extraction, and integration.

---

## Part 1: OCR Implementation with Tesseract

### Dependencies

```toml
# core/Cargo.toml
[dependencies]
tesseract = { version = "0.14", optional = true }
leptonica-sys = { version = "0.4", optional = true }

[features]
ocr = ["tesseract", "leptonica-sys"]
```

### System Requirements

**macOS:**
```bash
brew install tesseract
brew install leptonica
```

**Linux:**
```bash
apt-get install tesseract-ocr libtesseract-dev libleptonica-dev
# Language data
apt-get install tesseract-ocr-eng tesseract-ocr-spa tesseract-ocr-fra
```

**Windows:**
```powershell
# Download from GitHub releases
# https://github.com/UB-Mannheim/tesseract/wiki
```

### Tesseract Data Location

Tesseract needs trained language data files. Strategy:

**Option A: Use System Tesseract Data**
```rust
// Tesseract looks for TESSDATA_PREFIX env var
std::env::set_var("TESSDATA_PREFIX", "/usr/share/tesseract-ocr/5/tessdata/");
```

**Option B: Bundle in Spacedrive (Recommended)**
```
~/Library/Application Support/spacedrive/
└── models/
    └── tesseract/
        ├── eng.traineddata
        ├── spa.traineddata
        ├── fra.traineddata
        └── ...
```

Download on first use:
```rust
async fn ensure_tesseract_model(language: &str, data_dir: &Path) -> Result<PathBuf> {
    let model_path = data_dir.join("models/tesseract");
    let model_file = model_path.join(format!("{}.traineddata", language));

    if !model_file.exists() {
        // Download from GitHub
        let url = format!(
            "https://github.com/tesseract-ocr/tessdata_fast/raw/main/{}.traineddata",
            language
        );

        tokio::fs::create_dir_all(&model_path).await?;

        let response = reqwest::get(&url).await?;
        let bytes = response.bytes().await?;
        tokio::fs::write(&model_file, bytes).await?;
    }

    Ok(model_path)
}
```

### OCR Implementation

```rust
// core/src/ops/media/ocr/mod.rs

#[cfg(feature = "ocr")]
pub async fn extract_text_from_file(
    source_path: &Path,
    languages: &[String],
    data_dir: &Path,
) -> Result<String> {
    use tesseract::Tesseract;
    use tokio::task::spawn_blocking;

    let source = source_path.to_path_buf();
    let langs = languages.join("+");
    let data_dir = data_dir.to_path_buf();

    spawn_blocking(move || {
        // Ensure models are downloaded
        for lang in languages {
            let model_path = tokio::runtime::Handle::current()
                .block_on(ensure_tesseract_model(lang, &data_dir))?;
            std::env::set_var("TESSDATA_PREFIX", model_path);
        }

        // Initialize tesseract
        let mut tess = Tesseract::new(None, Some(&langs))
            .map_err(|e| anyhow::anyhow!("Tesseract init failed: {}", e))?;

        // Set image
        tess.set_image(&source.to_string_lossy())
            .map_err(|e| anyhow::anyhow!("Failed to set image: {}", e))?;

        // Configure
        tess.set_page_seg_mode(tesseract::PageSegMode::Auto);

        // Extract text
        let text = tess.get_text()
            .map_err(|e| anyhow::anyhow!("Text extraction failed: {}", e))?;

        Ok(text)
    }).await?
}

#[cfg(not(feature = "ocr"))]
pub async fn extract_text_from_file(
    _source_path: &Path,
    _languages: &[String],
    _data_dir: &Path,
) -> Result<String> {
    Err(anyhow::anyhow!("OCR feature not enabled. Rebuild with --features ocr"))
}
```

### OCR Processor Updates

```rust
// core/src/ops/media/ocr/processor.rs

pub async fn process(&self, db: &DatabaseConnection, entry: &ProcessorEntry) -> Result<ProcessorResult> {
    // ... existing code ...

    // Get data directory
    let data_dir = self.library.data_dir(); // Need to expose this

    // Extract text
    let extracted_text = super::extract_text_from_file(
        &entry.path,
        &self.languages,
        &data_dir,
    ).await?;

    // ... rest of implementation
}
```

---

## Part 2: Speech-to-Text Implementation with Whisper

### Dependencies

```toml
# core/Cargo.toml
[dependencies]
# Whisper.cpp bindings
whisper-rs = { version = "0.12", optional = true }

# Audio decoding
symphonia = { version = "0.5", features = ["all"] }

# FFmpeg for video audio extraction
ffmpeg-next = { version = "6.1", optional = true }

[features]
speech = ["whisper-rs", "ffmpeg-next"]
```

### System Requirements

**macOS:**
```bash
brew install ffmpeg
```

**Linux:**
```bash
apt-get install ffmpeg libavcodec-dev libavformat-dev libavutil-dev
```

**Windows:**
```powershell
# Download FFmpeg from ffmpeg.org
```

### Whisper Model Management

**Model Storage:**
```
~/Library/Application Support/spacedrive/
└── models/
    └── whisper/
        ├── ggml-tiny.bin      (75 MB)
        ├── ggml-base.bin      (142 MB)
        ├── ggml-small.bin     (466 MB)
        ├── ggml-medium.bin    (1.5 GB)
        └── ggml-large-v3.bin  (3.1 GB)
```

**Model Download System:**
```rust
// core/src/ops/media/speech/model.rs

pub struct WhisperModelManager {
    models_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl WhisperModel {
    fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin",
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
            Self::Large => "ggml-large-v3.bin",
        }
    }

    fn download_url(&self) -> &'static str {
        match self {
            Self::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            Self::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            Self::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            Self::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            Self::Large => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        }
    }

    fn size_bytes(&self) -> u64 {
        match self {
            Self::Tiny => 75 * 1024 * 1024,
            Self::Base => 142 * 1024 * 1024,
            Self::Small => 466 * 1024 * 1024,
            Self::Medium => 1500 * 1024 * 1024,
            Self::Large => 3100 * 1024 * 1024,
        }
    }
}

impl WhisperModelManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            models_dir: data_dir.join("models/whisper"),
        }
    }

    /// Ensure model is downloaded and return path
    pub async fn ensure_model(&self, model: WhisperModel) -> Result<PathBuf> {
        let model_path = self.models_dir.join(model.filename());

        if model_path.exists() {
            // Verify size matches expected
            if let Ok(metadata) = tokio::fs::metadata(&model_path).await {
                if metadata.len() == model.size_bytes() {
                    return Ok(model_path);
                }
            }
            // Size mismatch - redownload
            tokio::fs::remove_file(&model_path).await?;
        }

        // Download model
        self.download_model(&model).await?;

        Ok(model_path)
    }

    async fn download_model(&self, model: &WhisperModel) -> Result<()> {
        use futures::StreamExt;

        tokio::fs::create_dir_all(&self.models_dir).await?;

        let model_path = self.models_dir.join(model.filename());
        let temp_path = model_path.with_extension("tmp");

        tracing::info!(
            "Downloading whisper model {} ({} MB)...",
            model.filename(),
            model.size_bytes() / 1024 / 1024
        );

        // Stream download with progress
        let response = reqwest::get(model.download_url()).await?;
        let total_size = response.content_length().unwrap_or(0);

        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
            downloaded += chunk.len() as u64;

            if downloaded % (10 * 1024 * 1024) == 0 {
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                tracing::info!("Download progress: {:.1}%", progress);
            }
        }

        // Rename to final location
        tokio::fs::rename(&temp_path, &model_path).await?;

        tracing::info!("Model downloaded successfully: {}", model.filename());

        Ok(())
    }

    /// List available models (downloaded)
    pub async fn list_downloaded_models(&self) -> Result<Vec<WhisperModel>> {
        let mut models = Vec::new();

        for model in [
            WhisperModel::Tiny,
            WhisperModel::Base,
            WhisperModel::Small,
            WhisperModel::Medium,
            WhisperModel::Large,
        ] {
            let path = self.models_dir.join(model.filename());
            if path.exists() {
                models.push(model);
            }
        }

        Ok(models)
    }

    /// Delete a model
    pub async fn delete_model(&self, model: WhisperModel) -> Result<()> {
        let path = self.models_dir.join(model.filename());
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }
}
```

### Audio Extraction from Video

```rust
// core/src/ops/media/speech/audio.rs

use anyhow::Result;
use std::path::Path;
use tokio::process::Command;

/// Extract audio from video file to temporary WAV using FFmpeg
pub async fn extract_audio_from_video(video_path: &Path) -> Result<(PathBuf, Vec<f32>)> {
    let temp_dir = std::env::temp_dir();
    let temp_audio = temp_dir.join(format!(
        "spacedrive_audio_{}.wav",
        uuid::Uuid::new_v4()
    ));

    // Extract audio using FFmpeg
    // -i input.mp4: Input file
    // -vn: No video
    // -acodec pcm_s16le: 16-bit PCM
    // -ar 16000: 16kHz sample rate (whisper requirement)
    // -ac 1: Mono
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", video_path.to_str().unwrap(),
            "-vn",
            "-acodec", "pcm_s16le",
            "-ar", "16000",
            "-ac", "1",
            "-y", // Overwrite
            temp_audio.to_str().unwrap(),
        ])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "FFmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Load WAV file and convert to f32 samples
    let samples = load_wav_as_f32(&temp_audio).await?;

    Ok((temp_audio, samples))
}

/// Load WAV file and convert to f32 samples for whisper
async fn load_wav_as_f32(wav_path: &Path) -> Result<Vec<f32>> {
    use tokio::task::spawn_blocking;

    let path = wav_path.to_path_buf();

    spawn_blocking(move || {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let file = std::fs::File::open(&path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        hint.with_extension("wav");

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;

        let mut format = probed.format;
        let track = format.default_track().unwrap();
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;

        let mut samples = Vec::new();

        while let Ok(packet) = format.next_packet() {
            let decoded = decoder.decode(&packet)?;

            let mut sample_buf = SampleBuffer::<f32>::new(
                decoded.capacity() as u64,
                *decoded.spec(),
            );
            sample_buf.copy_interleaved_ref(decoded);

            samples.extend_from_slice(sample_buf.samples());
        }

        Ok(samples)
    }).await?
}

/// Clean up temporary audio file
pub async fn cleanup_temp_audio(path: PathBuf) -> Result<()> {
    if path.exists() {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}
```

### Whisper Implementation

```rust
// core/src/ops/media/speech/mod.rs

#[cfg(feature = "speech")]
pub async fn transcribe_audio_file(
    source_path: &Path,
    model: &str,
    language: Option<&str>,
    data_dir: &Path,
) -> Result<String> {
    use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
    use tokio::task::spawn_blocking;

    let source = source_path.to_path_buf();
    let model_name = model.to_string();
    let lang = language.map(|s| s.to_string());
    let data_dir = data_dir.to_path_buf();

    spawn_blocking(move || {
        // Get model path (download if needed)
        let model_manager = super::model::WhisperModelManager::new(&data_dir);
        let whisper_model = match model_name.as_str() {
            "tiny" => super::model::WhisperModel::Tiny,
            "base" => super::model::WhisperModel::Base,
            "small" => super::model::WhisperModel::Small,
            "medium" => super::model::WhisperModel::Medium,
            "large" => super::model::WhisperModel::Large,
            _ => super::model::WhisperModel::Base,
        };

        let model_path = tokio::runtime::Handle::current()
            .block_on(model_manager.ensure_model(whisper_model))?;

        // Load whisper context
        let ctx_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(&model_path.to_string_lossy(), ctx_params)
            .map_err(|e| anyhow::anyhow!("Whisper context init failed: {}", e))?;

        // Get audio samples
        let (temp_audio, audio_samples) = if is_video_file(&source) {
            // Extract audio from video using FFmpeg
            tokio::runtime::Handle::current()
                .block_on(super::audio::extract_audio_from_video(&source))?
        } else {
            // Load audio file directly
            let samples = tokio::runtime::Handle::current()
                .block_on(super::audio::load_wav_as_f32(&source))?;
            (source.clone(), samples)
        };

        // Create transcription params
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language if specified
        if let Some(l) = lang {
            params.set_language(Some(&l));
        } else {
            params.set_language(None); // Auto-detect
        }

        // Enable timestamps for SRT generation
        params.set_print_timestamps(true);
        params.set_print_progress(false);
        params.set_print_special(false);

        // Run transcription
        let mut state = ctx.create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create state: {}", e))?;

        state.full(params, &audio_samples)
            .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

        // Generate SRT format
        let segment_count = state.full_n_segments()?;
        let mut srt = String::new();

        for i in 0..segment_count {
            let start_ts = state.full_get_segment_t0(i)? as f64 / 100.0; // centiseconds to seconds
            let end_ts = state.full_get_segment_t1(i)? as f64 / 100.0;
            let text = state.full_get_segment_text(i)?;

            srt.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                format_srt_timestamp(start_ts),
                format_srt_timestamp(end_ts),
                text.trim()
            ));
        }

        // Clean up temp audio file if it was extracted from video
        if temp_audio != source {
            tokio::runtime::Handle::current()
                .block_on(super::audio::cleanup_temp_audio(temp_audio))?;
        }

        Ok(srt)
    }).await?
}

fn is_video_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or("").to_lowercase().as_str(),
            "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "flv"
        )
    } else {
        false
    }
}

fn format_srt_timestamp(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 1000.0) as u32;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}

#[cfg(not(feature = "speech"))]
pub async fn transcribe_audio_file(
    _source_path: &Path,
    _model: &str,
    _language: Option<&str>,
    _data_dir: &Path,
) -> Result<String> {
    Err(anyhow::anyhow!("Speech-to-text feature not enabled. Rebuild with --features speech"))
}
```

---

## Part 3: Model Management UI/API

### Core Query: List Available Models

```rust
// core/src/ops/media/speech/query.rs

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WhisperModelInfo {
    pub name: String,
    pub size_mb: u64,
    pub downloaded: bool,
    pub path: Option<String>,
}

pub struct ListWhisperModelsQuery;

impl CoreQuery for ListWhisperModelsQuery {
    type Input = ();
    type Output = Vec<WhisperModelInfo>;

    async fn execute(_input: (), context: &CoreContext) -> Result<Self::Output> {
        let data_dir = context.data_dir(); // Need to expose this
        let manager = WhisperModelManager::new(&data_dir);

        let mut models = Vec::new();
        for model in [Tiny, Base, Small, Medium, Large] {
            let path = manager.models_dir.join(model.filename());
            models.push(WhisperModelInfo {
                name: model.filename().to_string(),
                size_mb: model.size_bytes() / 1024 / 1024,
                downloaded: path.exists(),
                path: if path.exists() {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                },
            });
        }

        Ok(models)
    }
}
```

### Core Action: Download Model

```rust
// core/src/ops/media/speech/action.rs

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadWhisperModelInput {
    pub model: String, // "tiny", "base", "small", "medium", "large"
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadWhisperModelOutput {
    pub model_path: String,
    pub size_bytes: u64,
}

pub struct DownloadWhisperModelAction {
    input: DownloadWhisperModelInput,
}

impl CoreAction for DownloadWhisperModelAction {
    type Input = DownloadWhisperModelInput;
    type Output = DownloadWhisperModelOutput;

    async fn execute(self, context: &CoreContext) -> Result<Self::Output> {
        let data_dir = context.data_dir();
        let manager = WhisperModelManager::new(&data_dir);

        let model = match self.input.model.as_str() {
            "tiny" => WhisperModel::Tiny,
            "base" => WhisperModel::Base,
            "small" => WhisperModel::Small,
            "medium" => WhisperModel::Medium,
            "large" => WhisperModel::Large,
            _ => return Err(ActionError::InvalidInput("Invalid model name".into())),
        };

        let path = manager.ensure_model(model.clone()).await?;
        let size = model.size_bytes();

        Ok(DownloadWhisperModelOutput {
            model_path: path.to_string_lossy().to_string(),
            size_bytes: size,
        })
    }
}
```

---

## Part 4: Integration Checklist

### Step 1: Add Dependencies (Week 1)
- [ ] Add tesseract-rs to Cargo.toml with feature flag
- [ ] Add whisper-rs to Cargo.toml with feature flag
- [ ] Add symphonia for audio decoding
- [ ] Add ffmpeg-next for video audio extraction
- [ ] Test that features compile

### Step 2: Implement Model Management (Week 1)
- [ ] Create `WhisperModelManager` in `core/src/ops/media/speech/model.rs`
- [ ] Implement model download with progress
- [ ] Implement model verification
- [ ] Add model listing query
- [ ] Add model download action

### Step 3: Implement OCR (Week 2)
- [ ] Create tesseract model downloader
- [ ] Implement `extract_text_from_file()` with real tesseract
- [ ] Add error handling for missing models
- [ ] Test with PNG, JPEG, PDF files
- [ ] Verify text storage in database

### Step 4: Implement Audio Extraction (Week 2)
- [ ] Create `audio.rs` module in speech
- [ ] Implement FFmpeg video audio extraction
- [ ] Implement symphonia WAV loading
- [ ] Test with MP4, MOV, WebM videos
- [ ] Add cleanup for temp files

### Step 5: Implement Whisper (Week 3)
- [ ] Implement `transcribe_audio_file()` with real whisper
- [ ] Add SRT timestamp formatting
- [ ] Test with audio files
- [ ] Test with video files (via FFmpeg extraction)
- [ ] Verify sidecar creation

### Step 6: Build UI (Week 3-4)
- [ ] Settings page for model management
- [ ] Model download with progress bar
- [ ] Inspector "Extract Text" button
- [ ] Inspector "Generate Subtitles" button
- [ ] Location processor config UI

### Step 7: Performance Optimization (Week 4)
- [ ] Cache whisper context (don't reload model per file)
- [ ] Parallel batch processing
- [ ] GPU acceleration for whisper (if available)
- [ ] Optimize FFmpeg args for speed

---

## Part 5: File Structure

```
core/src/ops/media/
├── ocr/
│   ├── mod.rs         Module interface + filetype integration
│   ├── processor.rs   OcrProcessor (atomic operation)
│   ├── job.rs         OcrJob (batch processing)
│   ├── action.rs      ExtractTextAction (UI trigger)
│   └── model.rs       TODO: Tesseract model management
│
├── speech/
│   ├── mod.rs         Module interface + filetype integration
│   ├── processor.rs   SpeechToTextProcessor (atomic operation)
│   ├── job.rs         SpeechToTextJob (batch processing)
│   ├── action.rs      TranscribeAudioAction (UI trigger)
│   ├── model.rs       TODO: WhisperModelManager
│   └── audio.rs       TODO: FFmpeg audio extraction + symphonia
│
└── thumbnail/
    ├── mod.rs         Existing
    ├── processor.rs   ThumbnailProcessor
    └── ...

~/Library/Application Support/spacedrive/
└── models/            TODO: Create on first use
    ├── tesseract/     TODO: Language data files
    │   ├── eng.traineddata
    │   ├── spa.traineddata
    │   └── ...
    └── whisper/       TODO: Model binaries
        ├── ggml-tiny.bin
        ├── ggml-base.bin
        ├── ggml-small.bin
        ├── ggml-medium.bin
        └── ggml-large-v3.bin
```

---

## Part 6: Configuration Updates

### AppConfig Updates

```rust
// core/src/config/app_config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProcessingConfig {
    /// OCR settings
    pub ocr: OcrConfig,
    /// Speech-to-text settings
    pub speech: SpeechConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Default languages
    pub default_languages: Vec<String>,
    /// Minimum confidence threshold
    pub min_confidence: f32,
    /// Auto-download language data
    pub auto_download_models: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechConfig {
    /// Default whisper model
    pub default_model: String,
    /// Auto-download models
    pub auto_download_models: bool,
    /// Max model size to auto-download (in MB)
    pub max_auto_download_size: u64,
}

impl Default for MediaProcessingConfig {
    fn default() -> Self {
        Self {
            ocr: OcrConfig {
                default_languages: vec!["eng".to_string()],
                min_confidence: 0.6,
                auto_download_models: true,
            },
            speech: SpeechConfig {
                default_model: "base".to_string(),
                auto_download_models: false, // Models are large!
                max_auto_download_size: 200, // Only auto-download up to 200MB
            },
        }
    }
}
```

---

## Part 7: Testing Strategy

### Unit Tests

```rust
// core/src/ops/media/ocr/tests.rs

#[tokio::test]
#[cfg(feature = "ocr")]
async fn test_ocr_extraction() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create test image with known text
    let img = image::ImageBuffer::from_fn(800, 100, |x, y| {
        image::Rgb([255u8, 255u8, 255u8])
    });
    // Draw text on image...
    let img_path = temp_dir.path().join("test.png");
    img.save(&img_path).unwrap();

    // Run OCR
    let text = extract_text_from_file(
        &img_path,
        &["eng"],
        temp_dir.path()
    ).await.unwrap();

    assert!(text.contains("expected text"));
}
```

### Integration Tests

```rust
// core/tests/ocr_integration_test.rs

#[tokio::test]
async fn test_ocr_processor_with_database() {
    let test_env = create_test_environment().await;

    // Create entry for test image
    let entry_id = create_test_entry(&test_env, "test.png").await;

    // Run OCR processor
    let processor = OcrProcessor::new(test_env.library.id());
    let result = processor.process(&ctx, &entry).await.unwrap();

    assert!(result.success);

    // Verify text in database
    let ci = load_content_identity(&test_env.db, entry_id).await;
    assert!(ci.text_content.is_some());
}
```

---

## Part 8: Error Handling

### Common Errors and Solutions

**OCR Errors:**
- Missing tesseract → Show error with install instructions
- Missing language data → Auto-download or show instructions
- Unsupported image format → Convert to supported format first
- Low confidence → Return partial results with warning

**Speech Errors:**
- Missing whisper model → Prompt to download
- Audio extraction failed → Check FFmpeg installation
- Out of memory → Suggest smaller model or chunking
- No speech detected → Return empty subtitle file

### Error Messages

```rust
pub enum OcrError {
    TesseractNotInstalled,
    LanguageDataMissing(String),
    ExtractionFailed(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::TesseractNotInstalled => {
                write!(f, "Tesseract not installed. Install with: brew install tesseract")
            }
            Self::LanguageDataMissing(lang) => {
                write!(f, "Language data missing for '{}'. Download from models menu.", lang)
            }
            Self::ExtractionFailed(e) => write!(f, "OCR failed: {}", e),
        }
    }
}
```

---

## Part 9: Performance Considerations

### Model Caching

**Problem:** Loading whisper models is slow (seconds)
**Solution:** Cache WhisperContext globally

```rust
// core/src/ops/media/speech/cache.rs

use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::collections::HashMap;

static WHISPER_CONTEXT_CACHE: Lazy<Mutex<HashMap<String, Arc<WhisperContext>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn get_or_create_context(model_path: &Path) -> Result<Arc<WhisperContext>> {
    let key = model_path.to_string_lossy().to_string();

    let mut cache = WHISPER_CONTEXT_CACHE.lock().unwrap();

    if let Some(ctx) = cache.get(&key) {
        return Ok(Arc::clone(ctx));
    }

    // Load model (slow)
    let ctx = WhisperContext::new(&key)?;
    let arc_ctx = Arc::new(ctx);
    cache.insert(key, Arc::clone(&arc_ctx));

    Ok(arc_ctx)
}
```

### Batch Processing Optimization

```rust
// Process multiple files in parallel (up to N concurrent)
use futures::stream::{self, StreamExt};

const MAX_CONCURRENT_OCR: usize = 4; // CPU bound
const MAX_CONCURRENT_SPEECH: usize = 2; // Very CPU/GPU bound

stream::iter(entries)
    .map(|entry| async move {
        processor.process(&ctx, &entry).await
    })
    .buffer_unordered(MAX_CONCURRENT_OCR)
    .collect::<Vec<_>>()
    .await;
```

---

## Part 10: Frontend Integration

### Settings Page

```tsx
// packages/interface/src/components/Settings/MediaProcessing.tsx

function MediaProcessingSettings() {
  const { data: models } = useCoreQuery({
    type: 'media.speech.list_models',
    input: {}
  });

  const downloadModel = useCoreMutation('media.speech.download_model');

  return (
    <div>
      <h2>Speech-to-Text Models</h2>
      {models.map(model => (
        <div key={model.name}>
          <span>{model.name} ({model.size_mb} MB)</span>
          {model.downloaded ? (
            <Badge>Downloaded</Badge>
          ) : (
            <Button onClick={() => downloadModel.mutate({ model: model.name })}>
              Download
            </Button>
          )}
        </div>
      ))}
    </div>
  );
}
```

### Inspector Actions

```tsx
// packages/interface/src/components/Explorer/Inspector.tsx

function FileInspector({ file }: { file: File }) {
  const extractText = useLibraryMutation('media.ocr.extract');
  const transcribe = useLibraryMutation('media.speech.transcribe');

  const isImage = file.content_kind === 'Image';
  const isVideo = file.content_kind === 'Video';
  const isAudio = file.content_kind === 'Audio';

  return (
    <div>
      {isImage && (
        <Button onClick={() => extractText.mutate({
          entry_uuid: file.id,
          languages: ["eng"],
          force: false
        })}>
          Extract Text (OCR)
        </Button>
      )}

      {(isVideo || isAudio) && (
        <Button onClick={() => transcribe.mutate({
          entry_uuid: file.id,
          model: "base",
          language: null
        })}>
          Generate Subtitles
        </Button>
      )}

      {file.content_identity?.text_content && (
        <div className="mt-4">
          <h3>Extracted Text</h3>
          <pre className="bg-app-box p-2 rounded">
            {file.content_identity.text_content}
          </pre>
        </div>
      )}
    </div>
  );
}
```

---

## Part 11: Deployment Plan

### Development Build
```bash
# Enable both features for development
cargo build --features ocr,speech

# Or just one
cargo build --features ocr
cargo build --features speech
```

### Production Build
```bash
# Linux/macOS
cargo build --release --features ocr,speech

# Windows (may need separate OCR/speech builds due to dependencies)
cargo build --release --features ocr
```

### Docker
```dockerfile
FROM rust:1.75 as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    tesseract-ocr \
    libtesseract-dev \
    libleptonica-dev \
    ffmpeg \
    libavcodec-dev \
    libavformat-dev

# Build with features
WORKDIR /app
COPY . .
RUN cargo build --release --features ocr,speech
```

---

## Part 12: Timeline and Estimates

| Phase | Duration | Complexity | Blocker |
|-------|----------|------------|---------|
| Model Management | 3 days | Medium | None |
| OCR Implementation | 2 days | Low | Tesseract setup |
| Audio Extraction | 2 days | Medium | FFmpeg bindings |
| Whisper Implementation | 3 days | High | Model loading, audio format |
| UI Integration | 2 days | Low | Backend complete |
| Testing | 2 days | Medium | Real files needed |
| Documentation | 1 day | Low | None |
| **Total** | **15 days** | | |

---

## Part 13: Success Criteria

### OCR
- [ ] Extract text from PNG screenshot
- [ ] Extract text from scanned PDF
- [ ] Store in database correctly
- [ ] Text searchable in Spacedrive
- [ ] Works with multiple languages
- [ ] Handles errors gracefully

### Speech-to-Text
- [ ] Transcribe MP3 audio file
- [ ] Transcribe MP4 video file
- [ ] Generate valid SRT format
- [ ] Sidecar created and linked
- [ ] Subtitles viewable in UI
- [ ] Model caching works (fast second run)

---

**Ready to implement?** Start with model management, then OCR (simpler), then speech-to-text.
