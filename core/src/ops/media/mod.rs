//! Media processing operations
//!
//! This module contains jobs for processing media files including:
//! - Thumbnail generation
//! - OCR (text extraction from images/PDFs)
//! - Speech-to-text (audio/video transcription)
//! - Gaussian splat generation (3D view synthesis from images)
//! - Video transcoding
//! - Audio metadata extraction
//! - Image optimization
//! - Blurhash generation for image placeholders

pub mod blurhash;
pub mod metadata_extractor;
pub mod ocr;
pub mod proxy;
pub mod splat;

#[cfg(feature = "ffmpeg")]
pub mod speech;
#[cfg(feature = "ffmpeg")]
pub mod thumbnail;
#[cfg(feature = "ffmpeg")]
pub mod thumbstrip;

pub use metadata_extractor::{extract_image_metadata, extract_image_metadata_with_blurhash};

#[cfg(feature = "ffmpeg")]
pub use metadata_extractor::{
	extract_audio_metadata, extract_video_metadata, extract_video_metadata_with_blurhash,
};
pub use ocr::{OcrJob, OcrProcessor};
pub use proxy::{ProxyJob, ProxyProcessor};
pub use splat::{GaussianSplatJob, GaussianSplatProcessor};

#[cfg(feature = "ffmpeg")]
pub use speech::{SpeechToTextJob, SpeechToTextProcessor};
#[cfg(feature = "ffmpeg")]
pub use thumbnail::ThumbnailJob;
#[cfg(feature = "ffmpeg")]
pub use thumbstrip::{ThumbstripJob, ThumbstripProcessor};
