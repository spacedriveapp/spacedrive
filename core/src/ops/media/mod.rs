//! Media processing operations
//!
//! This module contains jobs for processing media files including:
//! - Thumbnail generation
//! - Video transcoding
//! - Audio metadata extraction
//! - Image optimization

pub mod live_photo;
pub mod live_photo_query;
pub mod thumbnail;

pub use live_photo::{LivePhoto, LivePhotoDetector};
pub use live_photo_query::{LivePhotoPair, LivePhotoQuery};
pub use thumbnail::ThumbnailJob;
