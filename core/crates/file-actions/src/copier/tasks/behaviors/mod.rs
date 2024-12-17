use std::path::Path;

mod fast;
mod stream;
mod utils;
mod delete;

pub use fast::FastCopyBehavior;
pub use stream::StreamCopyBehavior;
pub use delete::{DeleteBehavior, LocalDeleteBehavior};

use async_trait::async_trait;
use heavy_lifting::job::{JobContext, JobError};

const FAST_COPY_SIZE_THRESHOLD: u64 = 10 * 1024 * 1024; // 10MB
const MAX_RETRIES: u32 = 3;

/// Behavior trait for different copy strategies
#[async_trait]
pub trait CopyBehavior: Send + Sync {
    /// Copy a file using this behavior
    async fn copy_file(
        &self,
        source: impl AsRef<Path> + Send,
        target: impl AsRef<Path> + Send,
        ctx: &impl JobContext,
    ) -> Result<(), JobError>;

    /// Check if this behavior is suitable for the given source and target
    fn is_suitable(&self, source: impl AsRef<Path>, target: impl AsRef<Path>) -> bool;
}

/// Determine the most appropriate copy behavior for the given source and target
pub fn determine_behavior(source: impl AsRef<Path>, target: impl AsRef<Path>) -> Box<dyn CopyBehavior> {
    let behaviors: Vec<Box<dyn CopyBehavior>> = vec![
        Box::new(FastCopyBehavior),
        Box::new(StreamCopyBehavior::default()),
    ];

    behaviors
        .into_iter()
        .find(|b| b.is_suitable(&source, &target))
        .unwrap_or_else(|| Box::new(StreamCopyBehavior::default()))
}
