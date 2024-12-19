// This module provides the file copying functionality for Spacedrive.
// It exports the core components needed for file copy operations,
// including the copy job system, progress tracking, and copy behaviors.
//
// Features:
// - Parallel file copying with configurable concurrency
// - Detailed progress tracking with speed and ETA estimates
// - Automatic retry mechanism for failed operations
// - Support for both local and cross-device copies
// - Streaming and fast copy behaviors for different use cases

mod job;
mod progress;
mod tasks;

pub use job::CopyJob;
pub use progress::CopyProgress;
pub use tasks::{CopyBehavior, FastCopyBehavior, StreamCopyBehavior};
