use std::{path::{Path, PathBuf}, fmt, time::{Duration, Instant}};

use heavy_lifting::{
    task::{Task, TaskStatus},
    job::{JobContext, JobError},
};
use tokio::fs;
use serde::{Serialize, Deserialize};

use crate::copier::progress::CopyProgress;
use super::{
    batch::{batch_copy_files, collect_copy_entries},
    conflict::resolve_name_conflicts,
    behaviors::{CopyBehavior, FastCopyBehavior, StreamCopyBehavior},
};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CopyTask {
    batches: Vec<Vec<(PathBuf, PathBuf, u64)>>, // (source_path, target_path, size)
    current_batch: usize,
    progress: CopyProgress,
}

impl CopyTask {
    pub async fn new(source: PathBuf, target: PathBuf) -> Result<Self, JobError> {
        // First collect all files and directories
        let (files, dirs) = collect_copy_entries(&source, &target).await?;
        
        // Create all necessary directories
        for (_, dir) in dirs {
            fs::create_dir_all(&dir)
                .await
                .map_err(|e| JobError::IO(e.into()))?;
        }
        
        // Resolve any name conflicts
        let files = resolve_name_conflicts(files).await?;
        
        // Batch the files for optimal copying
        let batches = batch_copy_files(files).await?;
        
        Ok(Self {
            batches,
            current_batch: 0,
            progress: CopyProgress::default(),
        })
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

    async fn handle_error(&mut self, error: JobError, ctx: &impl JobContext) -> Result<TaskStatus, JobError> {
        // If we have a current file, we can try to resume from there
        if let Some((source, target)) = &self.current_file {
            // Clean up the partially copied file
            if fs::try_exists(target).await.map_err(|e| JobError::IO(e.into()))? {
                fs::remove_file(target).await.map_err(|e| JobError::IO(e.into()))?;
            }

            // Return a shutdown status with our current state
            Ok(TaskStatus::Shutdown(Box::new(self.clone())))
        } else {
            Err(error)
        }
    }

    pub async fn serialize(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }
}

#[async_trait::async_trait]
impl Task for CopyTask {
    type Error = JobError;

    fn name(&self) -> &'static str {
        "copy"
    }

    async fn run(&mut self, ctx: &impl JobContext) -> Result<TaskStatus, JobError> {
        let total_files = self.batches.iter().map(|batch| batch.len()).sum::<usize>() as u64;
        let total_bytes: u64 = self.batches.iter().map(|batch| batch.iter().map(|(_, _, size)| size).sum::<u64>()).sum();

        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        ctx.progress(CopyProgress::Started {
            total_files,
            total_bytes,
        }).await;

        let mut files_copied = 0;
        let mut bytes_copied = 0;
        let start = Instant::now();

        for batch in self.batches.iter().enumerate() {
            for (idx, (source, target, size)) in batch.1.iter().enumerate() {
                let target = Self::find_available_name(target).await?;
                
                let file_name = source.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                ctx.progress(CopyProgress::File {
                    name: file_name.clone(),
                    current_file: (files_copied + idx as u64 + 1),
                    total_files,
                    bytes: *size,
                    source: source.clone(),
                    target: target.clone(),
                }).await;

                let behavior = determine_behavior(source, &target);
                match behavior.copy_file(source, &target, ctx).await {
                    Ok(()) => {
                        files_copied += 1;
                        bytes_copied += size;
                    }
                    Err(e) => {
                        // Clean up and return shutdown status
                        if fs::try_exists(&target).await.map_err(|e| JobError::IO(e.into()))? {
                            fs::remove_file(&target).await.map_err(|e| JobError::IO(e.into()))?;
                        }
                        
                        self.current_batch = batch.0;
                        self.progress = CopyProgress::default();
                        
                        return Ok(TaskStatus::Shutdown(Box::new(self.clone())));
                    }
                }
            }
        }

        let duration = start.elapsed();
        let average_speed = if duration.as_secs() > 0 {
            bytes_copied / duration.as_secs()
        } else {
            bytes_copied
        };

        ctx.progress(CopyProgress::Completed {
            files_copied,
            bytes_copied,
            total_duration: duration,
            average_speed,
        }).await;

        Ok(TaskStatus::Complete)
    }
}
