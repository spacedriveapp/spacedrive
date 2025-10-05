//! Thumbnail generation system
//!
//! This module provides thumbnail generation capabilities for various media types
//! including images, videos, and documents. It operates as a separate job that
//! can run independently or be triggered after indexing operations.

pub mod action;
mod error;
mod generator;
mod job;
mod state;
mod utils;

pub use action::ThumbnailAction;
pub use error::{ThumbnailError, ThumbnailResult};
pub use generator::{ImageGenerator, ThumbnailGenerator, ThumbnailInfo, VideoGenerator};
pub use job::{ThumbnailJob, ThumbnailJobConfig};
pub use state::{ThumbnailEntry, ThumbnailPhase, ThumbnailState, ThumbnailStats};
pub use utils::ThumbnailUtils;
