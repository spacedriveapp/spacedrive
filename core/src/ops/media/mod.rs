//! Media processing operations
//!
//! This module contains jobs for processing media files including:
//! - Thumbnail generation
//! - OCR (text extraction from images/PDFs)
//! - Speech-to-text (audio/video transcription)
//! - Video transcoding
//! - Audio metadata extraction
//! - Image optimization

pub mod metadata_extractor;
pub mod ocr;
pub mod proxy;
pub mod speech;
pub mod thumbnail;
pub mod thumbstrip;

pub use metadata_extractor::extract_image_metadata;

#[cfg(feature = "ffmpeg")]
pub use metadata_extractor::{extract_audio_metadata, extract_video_metadata};
pub use ocr::{OcrJob, OcrProcessor};
pub use proxy::{ProxyJob, ProxyProcessor};
pub use speech::{SpeechToTextJob, SpeechToTextProcessor};
pub use thumbnail::ThumbnailJob;
pub use thumbstrip::{ThumbstripJob, ThumbstripProcessor};
