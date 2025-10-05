//! Modular file copy operations using the Strategy Pattern

pub mod action;
pub mod database;
pub mod input;
pub mod job;
pub mod output;
pub mod routing;
pub mod strategy;

pub use job::{CopyError, CopyOptions, CopyProgress, FileCopyJob, MoveMode};
pub use output::FileCopyActionOutput;
pub use routing::CopyStrategyRouter;
pub use strategy::{
	CopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy, RemoteTransferStrategy,
};

// Re-export for backward compatibility
pub use job::MoveJob;
