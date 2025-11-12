# Prompt Injection Detection System

## Overview

A security layer for Spacedrive that scans extracted text content (from OCR, speech-to-text, and file indexing) to detect potential prompt injection attacks before AI agents can access the data. This enables safer AI-powered workflows by identifying malicious content embedded in user files.

## Motivation

As Spacedrive integrates AI agents for file operations, search, and content analysis, files containing adversarial text could manipulate agent behavior through prompt injection. Examples:

- PDFs with hidden instructions in OCR'd text
- Image files with malicious text overlays
- Audio files with transcribed attack vectors
- Plain text files designed to hijack agent context

By detecting these threats at the data preparation layer, we can:
- Warn users before agents process suspicious files
- Implement trust levels for file content
- Create safe zones for AI operations
- Enable confident AI feature adoption

## Model Details

**Model:** `ProtectAI/deberta-v3-base-prompt-injection`

**Size:** ~738 MB (safetensors format)

**Performance:**
- Accuracy: 99.99%
- Precision: 99.98%
- Recall: 99.97%
- F1 Score: 99.98%

**Classification:**
- Label 0: No injection detected
- Label 1: Injection detected

**Inference Options:**
- Transformers (PyTorch)
- ONNX Runtime (optimized for CPU inference)

## Architecture

### Integration Points

The detection system integrates with existing Spacedrive infrastructure:

```
┌─────────────────────────────────────────────────────────┐
│                    Indexing Pipeline                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  File Discovery → Content Extraction → Analysis Jobs    │
│                        ↓                      ↓          │
│                   ┌─────────┐          ┌──────────┐     │
│                   │   OCR   │          │  Speech  │     │
│                   └─────────┘          └──────────┘     │
│                        ↓                      ↓          │
│                   ┌─────────────────────────────┐       │
│                   │ Prompt Injection Detection  │       │
│                   └─────────────────────────────┘       │
│                              ↓                           │
│                   ┌─────────────────────┐               │
│                   │  Trust Metadata DB  │               │
│                   └─────────────────────┘               │
└─────────────────────────────────────────────────────────┘
```

### File Structure

```
core/src/ops/
├── models/
│   ├── mod.rs
│   ├── manager.rs
│   └── injection_detector.rs    # New: Model wrapper & inference
├── media/
│   ├── ocr/
│   ├── speech/
│   └── injection/                # New: Detection job & processor
│       ├── mod.rs
│       ├── action.rs
│       └── processor.rs
└── indexing/
    └── phases/
        └── content.rs            # Integration point
```

### Data Flow

1. **Content Extraction**
   - OCR extracts text from images/PDFs
   - Speech-to-text transcribes audio
   - Text files are read directly

2. **Detection Job**
   - Queued after extraction completes
   - Runs inference on extracted text
   - Generates trust score (0-1 confidence)

3. **Metadata Storage**
   - Results stored in `file_metadata` table
   - Fields: `injection_score`, `injection_detected`, `scanned_at`
   - Cached to avoid re-scanning

4. **Agent Access**
   - Query files by trust level
   - Filter out high-risk content
   - Surface warnings in UI

## Implementation Plan

### Phase 1: Model Infrastructure

```rust
// core/src/ops/models/injection_detector.rs

use candle_core::{Device, Tensor};
use candle_transformers::models::deberta_v3::DebertaV3ForSequenceClassification;

pub struct InjectionDetector {
    model: DebertaV3ForSequenceClassification,
    tokenizer: Tokenizer,
    device: Device,
}

impl InjectionDetector {
    pub async fn load(model_path: PathBuf) -> Result<Self> {
        // Load model from downloaded weights
    }

    pub async fn detect(&self, text: &str) -> Result<DetectionResult> {
        // Tokenize, run inference, return classification
    }
}

pub struct DetectionResult {
    pub is_injection: bool,
    pub confidence: f32,
    pub label: u32,
}
```

### Phase 2: Detection Job

```rust
// core/src/ops/media/injection/mod.rs

pub struct InjectionDetectionAction {
    pub file_id: i32,
    pub text_content: String,
}

pub struct InjectionDetectionOutput {
    pub file_id: i32,
    pub is_injection: bool,
    pub confidence: f32,
}

// Processor follows existing pattern from OCR/speech
pub struct InjectionDetectionProcessor {
    detector: Arc<InjectionDetector>,
}
```

### Phase 3: Database Schema

```sql
-- Add to file_metadata or create dedicated table

ALTER TABLE file_metadata ADD COLUMN injection_detected BOOLEAN;
ALTER TABLE file_metadata ADD COLUMN injection_confidence REAL;
ALTER TABLE file_metadata ADD COLUMN injection_scanned_at TIMESTAMP;

-- Index for filtering safe files
CREATE INDEX idx_injection_safe ON file_metadata(injection_detected)
WHERE injection_detected = false;
```

### Phase 4: Job Orchestration

Integrate into existing job system:

```rust
// core/src/ops/indexing/phases/content.rs

// After OCR/speech extraction
if let Some(extracted_text) = ocr_output.text {
    if library.settings.injection_detection_enabled {
        job_manager.submit(InjectionDetectionAction {
            file_id: file.id,
            text_content: extracted_text,
        }).await?;
    }
}
```

### Phase 5: Model Download

Leverage existing model manager:

```rust
// core/src/ops/models/manager.rs

pub enum ModelType {
    TesseractOcr,
    WhisperSpeech,
    InjectionDetection,  // New variant
}

impl ModelManager {
    pub async fn download_injection_detector(&self) -> Result<PathBuf> {
        self.download_model(
            ModelType::InjectionDetection,
            "ProtectAI/deberta-v3-base-prompt-injection",
            &["model.safetensors", "tokenizer.json", "spm.model"]
        ).await
    }
}
```

## Configuration

User settings in library config:

```rust
pub struct LibrarySettings {
    // Existing fields...

    /// Enable prompt injection scanning
    pub injection_detection_enabled: bool,

    /// Confidence threshold (0.0-1.0)
    /// Files scoring above this are flagged
    pub injection_threshold: f32,

    /// Auto-scan on indexing vs manual trigger
    pub injection_auto_scan: bool,
}

impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            injection_detection_enabled: false,  // Opt-in
            injection_threshold: 0.8,
            injection_auto_scan: true,
            // ...
        }
    }
}
```

## UI/UX

### Settings Panel

```
┌─────────────────────────────────────────┐
│ AI Safety                                │
├─────────────────────────────────────────┤
│                                          │
│ Enable prompt injection detection     │
│                                          │
│ Scans extracted text for malicious      │
│ content before AI agents can access it  │
│                                          │
│ Model: ProtectAI DeBERTa v3 (~738 MB)   │
│ Status: [Not Downloaded] [Download]     │
│                                          │
│ Confidence threshold: [====|---] 0.8    │
│                                          │
│ Auto-scan during indexing             │
│ ☐ Block agent access to flagged files   │
│                                          │
└─────────────────────────────────────────┘
```

### File Inspector

```
┌─────────────────────────────────────────┐
│ document.pdf                             │
├─────────────────────────────────────────┤
│                                          │
│ ️  Security Warning                     │
│                                          │
│ This file may contain prompt injection  │
│ Confidence: 95%                          │
│                                          │
│ [View Details] [Mark Safe] [Quarantine] │
│                                          │
└─────────────────────────────────────────┘
```

### Search Filters

```
Filter by trust level:
○ All files
● Safe only (no injections detected)
○ Flagged only
○ Unscanned
```

## Performance Considerations

### Inference Speed

- DeBERTa v3 base: ~100-200ms per sample on CPU
- ONNX optimization: ~50-100ms per sample
- Batch processing: Queue multiple files for efficient GPU usage

### Resource Usage

- Model RAM: ~2GB during inference
- Disk space: 738 MB per library (shared models possible)
- Background processing: Low priority job queue

### Scaling

- Cache results to avoid re-scanning
- Incremental scanning for large libraries
- Optional GPU acceleration for bulk operations
- Consider smaller/faster models for real-time use cases

## Security Properties

### Threat Model

**Protected against:**
- Embedded prompt injections in documents
- Malicious OCR'd text in images
- Adversarial transcriptions from audio
- Social engineering via file content

**Not protected against:**
- Zero-day injection techniques not in training data
- Attacks on the model itself (adversarial examples)
- Non-text attack vectors
- Direct agent prompt manipulation

### Trust Boundaries

```
Untrusted Input → Detection → Trusted Output
     ↓              ↓              ↓
  User Files    ML Model      Agent Access
```

Files move from untrusted to trusted only after passing detection threshold.

### False Positives

With 99.98% precision, expect ~0.02% false positive rate. For a 10,000 file library:
- ~2 legitimate files flagged incorrectly
- User can manually mark as safe
- Review process prevents over-blocking

## Future Enhancements

### Multi-Model Ensemble

Combine multiple detection models for higher confidence:
- ProtectAI DeBERTa (current)
- Lakera Guard
- Custom fine-tuned models

### Real-Time Protection

Stream text to detector as it's extracted:
- WebSocket updates for long OCR jobs
- Immediate feedback in UI
- Cancel processing on detection

### Adversarial Training

Fine-tune on Spacedrive-specific attack vectors:
- Community-reported injections
- Red team exercises
- Federated learning from opt-in users

### Agent Integration

Native agent API for trust checking:

```rust
impl Agent {
    async fn can_access_file(&self, file_id: i32) -> bool {
        let trust = db.get_trust_score(file_id).await?;
        trust.injection_detected == false
    }
}
```

## Open Questions

1. **Model hosting**: HuggingFace direct download vs CDN mirror?
2. **Licensing**: Apache 2.0 model but training data may have restrictions
3. **Quantization**: 4-bit/8-bit models to reduce size/improve speed?
4. **Cross-library sharing**: Single model download for all libraries?
5. **Mobile support**: On-device inference or cloud API fallback?

## References

- [ProtectAI Model Card](https://huggingface.co/ProtectAI/deberta-v3-base-prompt-injection)
- [DeBERTa v3 Paper](https://arxiv.org/abs/2111.09543)
- [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/)
- Existing Spacedrive docs: `OCR_SPEECH_IMPLEMENTATION_PLAN.md`, `PROCESSOR_SYSTEM_COMPLETE.md`
