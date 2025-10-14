//! Add cloud volume operation

pub mod action;
pub mod output;

pub use action::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput};
pub use output::VolumeAddCloudOutput;
