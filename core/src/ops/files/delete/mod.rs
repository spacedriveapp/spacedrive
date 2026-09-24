//! File delete operations

pub mod action;
mod compared;
mod duplicates;
pub mod input;
pub mod job;
pub mod output;
mod preflight;
pub(crate) use preflight::last_copies;
pub mod routing;
pub mod strategy;

pub use action::FileDeleteAction;
pub use input::{DeleteTargets, Duplicates, FileDeleteInput, Keep};
pub use job::*;
pub use output::FileDeleteOutput;
pub use routing::DeleteStrategyRouter;
pub use strategy::{DeleteResult, DeleteStrategy, LocalDeleteStrategy, RemoteDeleteStrategy};
