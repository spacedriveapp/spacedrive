//! File rename operations
//!
//! Provides a dedicated action API for renaming files and directories.
//! Wraps FileCopyJob::new_rename() for execution while providing input validation.

pub mod action;
pub mod input;
pub mod validation;

pub use action::FileRenameAction;
pub use input::FileRenameInput;
pub use validation::{validate_filename, FilenameValidationError};
