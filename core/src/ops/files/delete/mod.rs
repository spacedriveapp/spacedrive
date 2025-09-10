//! File delete operations

pub mod action;
pub mod input;
pub mod job;
pub mod output;

pub use action::FileDeleteAction;
pub use input::FileDeleteInput;
pub use job::*;
pub use output::FileDeleteOutput;