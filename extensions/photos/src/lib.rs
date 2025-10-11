//! Photos Extension for Spacedrive
//!
//! Mirrors Apple Photos and Google Photos capabilities:
//! - Automatic face detection and clustering
//! - Place identification from GPS/EXIF
//! - Moment generation (time + location clustering)
//! - Smart search with scene understanding
//! - Memories and featured photos
//! - Albums and shared albums
//!
//! This demonstrates the full VDFS SDK specification.

mod actions;
mod agent;
mod config;
mod jobs;
mod models;
mod queries;
mod tasks;
mod utils;

pub use actions::*;
pub use config::*;
pub use models::*;
pub use queries::*;

use spacedrive_sdk::{extension, prelude::*};

#[extension(
    id = "com.spacedrive.photos",
    name = "Photos",
    version = "1.0.0",
    description = "Advanced photo management with faces, places, and intelligent organization",
    min_core_version = "2.0.0",
    required_features = ["exif_extraction", "ai_models"],
    permissions = [
        Permission::ReadEntries,
        Permission::ReadSidecars(kinds = vec!["exif", "thumbnail"]),
        Permission::WriteSidecars(kinds = vec!["faces", "places", "scene"]),
        Permission::WriteTags,
        Permission::WriteCustomFields(namespace = "photos"),
        Permission::UseModel(category = "face_detection", preference = ModelPreference::LocalOnly),
        Permission::UseModel(category = "scene_classification", preference = ModelPreference::LocalOnly),
        Permission::DispatchJobs,
    ]
)]
pub struct Photos {
	config: PhotosConfig,
}
