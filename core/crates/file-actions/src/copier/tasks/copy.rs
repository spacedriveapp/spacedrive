use std::{path::{Path, PathBuf}, fmt};

use heavy_lifting::{
    task::Task,
    job::{JobContext, JobError},
};
use tokio::fs;

use super::{
    copy_behavior::{CopyBehavior, determine_behavior},
    progress::CopyProgress,
};

#[derive(Debug)]
pub(crate) struct CopyTask {
    files: Vec<(PathBuf, PathBuf, u64)>, // (source_path, target_path, size)
}

impl CopyTask {
    pub fn new(files: Vec<(PathBuf, PathBuf, u64)>) -> Self {
        Self { files }
    }

    async fn find_available_name(path: impl AsRef<Path>) -> Result<PathBuf, JobError> {
        let path = path.as_ref();
        
        if !fs::try_exists(path).await.map_err(|e| JobError::IO(e.into()))? {
            return Ok(path.to_owned());
        }

        let file_stem = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| JobError::InvalidInput("File has no valid stem".into()))?;
            
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let parent = path.parent()
            .ok_or_else(|| JobError::InvalidInput("File has no parent directory".into()))?;

        for i in 1.. {
            let new_name = if extension.is_empty() {
                format!("{} ({})", file_stem, i)
            } else {
                format!("{} ({}).{}", file_stem, i, extension)
            };

            let new_path = parent.join(new_name);
            if !fs::try_exists(&new_path).await.map_err(|e| JobError::IO(e.into()))? {
                return Ok(new_path);
            }
        }

        unreachable!()
    }
}

#[async_trait::async_trait]
impl Task for CopyTask {
    type Error = JobError;

    fn name(&self) -> &'static str {
        "copy"
    }

    async fn run(&self, ctx: &impl JobContext) -> Result<(), JobError> {
        let total_files = self.files.len() as u64;
        let total_bytes: u64 = self.files.iter().map(|(_, _, size)| size).sum();

        ctx.progress(CopyProgress::Started {
            total_files,
            total_bytes,
        }).await;

        let mut files_copied = 0u64;
        let mut bytes_copied = 0u64;

        for (idx, (source, target, size)) in self.files.iter().enumerate() {
            let target = Self::find_available_name(target).await?;
            
            let file_name = source.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            ctx.progress(CopyProgress::File {
                name: file_name.clone(),
                current_file: (idx + 1) as u64,
                total_files,
                bytes: *size,
                source: source.clone(),
                target: target.clone(),
            }).await;

            let behavior = determine_behavior(source, &target);
            behavior.copy_file(source, &target, ctx).await?;

            files_copied += 1;
            bytes_copied += size;
        }

        ctx.progress(CopyProgress::Completed {
            files_copied,
            bytes_copied,
        }).await;

        Ok(())
    }
}
