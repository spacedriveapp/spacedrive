//! Create folder operations
//!
//! Provides actions for creating new folders, optionally with items to move into them.
//! Uses VolumeBackend for cloud storage compatibility.

pub mod action;
pub mod input;
pub mod output;

pub use action::CreateFolderAction;
pub use input::CreateFolderInput;
pub use output::CreateFolderOutput;
