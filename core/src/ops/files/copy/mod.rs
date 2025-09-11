//! Modular file copy operations using the Strategy Pattern

pub mod action;
pub mod database;
pub mod input;
pub mod job;
pub mod output;
pub mod routing;
pub mod strategy;

pub use job::{FileCopyJob, CopyOptions, MoveMode, CopyProgress, CopyError};
pub use output::FileCopyActionOutput;
pub use strategy::{CopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy, RemoteTransferStrategy};
pub use routing::CopyStrategyRouter;

// Re-export for backward compatibility
pub use job::MoveJob;