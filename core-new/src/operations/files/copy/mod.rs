//! Modular file copy operations using the Strategy Pattern

pub mod action;
pub mod job;
pub mod routing;
pub mod strategy;

pub use job::{FileCopyJob, CopyOptions, MoveMode, FileCopyOutput, CopyProgress, CopyError};
pub use strategy::{CopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy, RemoteTransferStrategy};
pub use routing::CopyStrategyRouter;

// Re-export for backward compatibility
pub use job::MoveJob;