//! Media processing operations
//!
//! This module contains jobs for processing media files including:
//! - Thumbnail generation
//! - Video transcoding
//! - Audio metadata extraction
//! - Image optimization

pub mod thumbnail;

pub use thumbnail::ThumbnailJob;
