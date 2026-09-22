//! File delete operations

pub mod action;
mod compared;
pub mod input;
pub mod job;
pub mod output;
pub mod routing;
pub mod strategy;

pub use action::FileDeleteAction;
pub use input::{DeleteTargets, FileDeleteInput};
pub use job::*;
pub use output::FileDeleteOutput;
pub use routing::DeleteStrategyRouter;
pub use strategy::{DeleteResult, DeleteStrategy, LocalDeleteStrategy, RemoteDeleteStrategy};
