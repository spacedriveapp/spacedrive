//! File operations - queries and actions for the File domain

pub mod archive;
pub mod attributes_action;
pub mod copy;
pub mod create_folder;
pub mod delete;
#[cfg(test)]
pub(crate) mod fixture;
pub mod link;
pub mod merge;
pub mod organize;
pub mod plan;
pub(crate) mod planner;
pub mod query;
pub mod rename;
pub mod trash;
pub mod trash_view;
pub mod undo;

pub use create_folder::{CreateFolderAction, CreateFolderInput, CreateFolderOutput};
pub use query::*;
pub use rename::{FileRenameAction, FileRenameBatchAction, FileRenameBatchInput, FileRenameInput};
