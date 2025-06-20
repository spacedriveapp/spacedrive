//! Thumbnail generation system
//!
//! This module provides thumbnail generation capabilities for various media types
//! including images, videos, and documents. It operates as a separate job that
//! can run independently or be triggered after indexing operations.

mod job;
mod state;
mod generator;
mod error;
mod utils;

pub use job::{ThumbnailJob, ThumbnailJobConfig};
pub use state::{ThumbnailState, ThumbnailPhase, ThumbnailEntry, ThumbnailStats};
pub use generator::{ThumbnailGenerator, ThumbnailInfo, ImageGenerator, VideoGenerator};
pub use error::{ThumbnailError, ThumbnailResult};
pub use utils::ThumbnailUtils;