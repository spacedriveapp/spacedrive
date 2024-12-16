use std::path::{Path, PathBuf};
use async_trait::async_trait;
use heavy_lifting::job::{JobContext, JobError};
use tokio::{fs, io::{self, AsyncReadExt, AsyncWriteExt}};

use super::progress::CopyProgress;

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
}

/// Fast copy using fs::copy, suitable for local files
pub struct FastCopyBehavior;

#[async_trait]
impl CopyBehavior for FastCopyBehavior {
    async fn copy_file(
        &self,
        source: impl AsRef<Path> + Send,
        target: impl AsRef<Path> + Send,
        _ctx: &impl JobContext,
    ) -> Result<(), JobError> {
        fs::copy(&source, &target)
            .await
            .map_err(|e| JobError::IO(e.into()))?;
        Ok(())
    }
}

/// Stream copy with progress reporting, suitable for remote files or when progress tracking is needed
pub struct StreamCopyBehavior {
    buffer_size: usize,
}

impl StreamCopyBehavior {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    pub fn default() -> Self {
        Self::new(8192) // 8KB default buffer
    }
}

#[async_trait]
impl CopyBehavior for StreamCopyBehavior {
    async fn copy_file(
        &self,
        source: impl AsRef<Path> + Send,
        target: impl AsRef<Path> + Send,
        ctx: &impl JobContext,
    ) -> Result<(), JobError> {
        let mut source_file = fs::File::open(&source)
            .await
            .map_err(|e| JobError::IO(e.into()))?;

        let mut target_file = fs::File::create(&target)
            .await
            .map_err(|e| JobError::IO(e.into()))?;

        let total_size = source_file
            .metadata()
            .await
            .map_err(|e| JobError::IO(e.into()))?
            .len();

        let file_name = source.as_ref()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut buffer = vec![0; self.buffer_size];
        let mut bytes_copied = 0;

        loop {
            let n = source_file
                .read(&mut buffer)
                .await
                .map_err(|e| JobError::IO(e.into()))?;
            
            if n == 0 {
                break;
            }

            target_file
                .write_all(&buffer[..n])
                .await
                .map_err(|e| JobError::IO(e.into()))?;

            bytes_copied += n as u64;
            
            ctx.progress(CopyProgress::FileProgress {
                name: file_name.clone(),
                bytes_copied,
                total_bytes: total_size,
            }).await;
        }

        target_file
            .flush()
            .await
            .map_err(|e| JobError::IO(e.into()))?;

        Ok(())
    }
}

pub fn determine_behavior(_source: impl AsRef<Path>, _target: impl AsRef<Path>) -> Box<dyn CopyBehavior> {
    // TODO: Implement logic to determine if we should use fast or stream copy
    // For now always use stream copy for testing progress reporting
    Box::new(StreamCopyBehavior::default())
}
