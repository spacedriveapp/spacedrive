//! Simplified FileCopyJob using the Strategy Pattern

use super::routing::CopyStrategyRouter;
use crate::{
    infrastructure::jobs::prelude::*,
    shared::types::{SdPath, SdPathBatch},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Move operation modes for UI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveMode {
    /// Standard move operation
    Move,
    /// Rename a single file/directory
    Rename,
    /// Cut and paste operation (same as move but different UX context)
    Cut,
}

/// Options for file copy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
    pub overwrite: bool,
    pub verify_checksum: bool,
    pub preserve_timestamps: bool,
    pub delete_after_copy: bool,
    pub move_mode: Option<MoveMode>,
}

impl Default for CopyOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            verify_checksum: false,
            preserve_timestamps: true,
            delete_after_copy: false,
            move_mode: None,
        }
    }
}

/// File copy job using the Strategy Pattern
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct FileCopyJob {
    pub sources: SdPathBatch,
    pub destination: SdPath,
    #[serde(default)]
    pub options: CopyOptions,

    // Internal state for resumption
    #[serde(default)]
    completed_indices: Vec<usize>,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
}

impl Job for FileCopyJob {
    const NAME: &'static str = "file_copy";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Copy or move files to a destination");
}

#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
    type Output = FileCopyOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log(format!(
            "Starting copy operation on {} files",
            self.sources.paths.len()
        ));

        // Group by device for efficient processing
        let by_device: HashMap<Uuid, Vec<SdPath>> = self
            .sources
            .by_device()
            .into_iter()
            .map(|(device_id, paths)| (device_id, paths.into_iter().cloned().collect()))
            .collect();

        let total_files = self.sources.paths.len();
        let mut copied_count = 0;
        let mut total_bytes = 0u64;
        let mut failed_copies = Vec::new();
        let is_move = self.options.delete_after_copy;
        let volume_manager = ctx.volume_manager();

        // Calculate total size for progress
        let estimated_total_bytes = self.calculate_total_size(&ctx).await?;

        // Process each source using the appropriate strategy
        for source in &self.sources.paths {
            ctx.check_interrupt().await?;

            let final_destination = if self.sources.paths.len() > 1 {
                // If multiple sources, treat destination as a directory
                self.destination.join(source.path.file_name().unwrap_or_default())
            } else {
                self.destination.clone()
            };

            // Update progress
            ctx.progress(Progress::structured(CopyProgress {
                current_file: source.display(),
                files_copied: copied_count,
                total_files,
                bytes_copied: total_bytes,
                total_bytes: estimated_total_bytes,
                current_operation: CopyStrategyRouter::describe_strategy(
                    source,
                    &final_destination,
                    is_move,
                    volume_manager.as_deref(),
                ).await,
                estimated_remaining: None,
            }));

            // 1. Select the strategy
            let strategy = CopyStrategyRouter::select_strategy(
                source,
                &final_destination,
                is_move,
                volume_manager.as_deref(),
            ).await;

            // 2. Execute the strategy
            match strategy.execute(&ctx, source, &final_destination).await {
                Ok(bytes) => {
                    copied_count += 1;
                    total_bytes += bytes;

                    // If this is a move operation and the strategy didn't handle deletion,
                    // we need to delete the source after successful copy
                    if is_move && source.device_id == final_destination.device_id {
                        // For same-device moves, LocalMoveStrategy handles deletion atomically
                        // For cross-volume moves, LocalStreamCopyStrategy needs manual deletion
                        if let Some(vm) = volume_manager.as_deref() {
                            if let (Some(source_path), Some(dest_path)) = 
                                (source.as_local_path(), final_destination.as_local_path()) {
                                if !vm.same_volume(source_path, dest_path).await {
                                    // Cross-volume move - delete source
                                    if let Err(e) = self.delete_source_file(source_path).await {
                                        failed_copies.push(CopyError {
                                            source: source.path.clone(),
                                            destination: final_destination.path.clone(),
                                            error: format!("Copy succeeded but failed to delete source: {}", e),
                                        });
                                        ctx.add_non_critical_error(format!(
                                            "Failed to delete source after move {}: {}",
                                            source.display(),
                                            e
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    failed_copies.push(CopyError {
                        source: source.path.clone(),
                        destination: final_destination.path.clone(),
                        error: e.to_string(),
                    });
                    ctx.add_non_critical_error(format!(
                        "Failed to {} {}: {}",
                        if is_move { "move" } else { "copy" },
                        source.display(),
                        e
                    ));
                }
            }

            // Checkpoint every 20 files
            if copied_count % 20 == 0 {
                ctx.checkpoint().await?;
            }
        }

        ctx.log(format!(
            "Copy operation completed: {} copied, {} failed",
            copied_count,
            failed_copies.len()
        ));

        Ok(FileCopyOutput {
            copied_count,
            failed_count: failed_copies.len(),
            total_bytes,
            duration: self.started_at.elapsed(),
            failed_copies,
            is_move_operation: self.options.delete_after_copy,
        })
    }
}

/// Copy progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyProgress {
    pub current_file: String,
    pub files_copied: usize,
    pub total_files: usize,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub current_operation: String,
    pub estimated_remaining: Option<Duration>,
}

impl JobProgress for CopyProgress {}

/// Error information for failed copies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyError {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub error: String,
}

impl FileCopyJob {
    /// Create a new file copy job with sources and destination
    pub fn new(sources: SdPathBatch, destination: SdPath) -> Self {
        Self {
            sources,
            destination,
            options: Default::default(),
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        }
    }

    /// Create an empty job (used by derive macro)
    pub fn empty() -> Self {
        Self {
            sources: SdPathBatch::new(Vec::new()),
            destination: SdPath::new(uuid::Uuid::new_v4(), PathBuf::new()),
            options: Default::default(),
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        }
    }

    /// Create from individual paths
    pub fn from_paths(sources: Vec<SdPath>, destination: SdPath) -> Self {
        Self::new(SdPathBatch::new(sources), destination)
    }

    /// Set copy options
    pub fn with_options(mut self, options: CopyOptions) -> Self {
        self.options = options;
        self
    }

    /// Create a move job using the copy job with delete_after_copy
    pub fn new_move(sources: SdPathBatch, destination: SdPath, move_mode: MoveMode) -> Self {
        let mut options = CopyOptions::default();
        options.delete_after_copy = true;
        options.move_mode = Some(move_mode);
        Self {
            sources,
            destination,
            options,
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        }
    }

    /// Create a rename operation
    pub fn new_rename(source: SdPath, new_name: String) -> Self {
        let destination = SdPath::new(
            source.device_id,
            source.path.with_file_name(new_name)
        );

        Self::new_move(
            SdPathBatch::new(vec![source]),
            destination,
            MoveMode::Rename
        )
    }

    /// Calculate total size for progress reporting
    async fn calculate_total_size(&self, ctx: &JobContext<'_>) -> JobResult<u64> {
        let mut total = 0u64;

        for source in &self.sources.paths {
            if let Some(local_path) = source.as_local_path() {
                total += self.get_path_size(local_path).await.unwrap_or(0);
            }
        }

        Ok(total)
    }

    /// Get size of a path (file or directory) using iterative approach
    async fn get_path_size(&self, path: &std::path::Path) -> Result<u64, std::io::Error> {
        let mut total = 0u64;
        let mut stack = vec![path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            let metadata = tokio::fs::metadata(&current_path).await?;

            if metadata.is_file() {
                total += metadata.len();
            } else if metadata.is_dir() {
                let mut dir = tokio::fs::read_dir(&current_path).await?;
                while let Some(entry) = dir.next_entry().await? {
                    stack.push(entry.path());
                }
            }
        }

        Ok(total)
    }

    /// Delete source file after successful cross-volume move
    async fn delete_source_file(&self, source: &std::path::Path) -> Result<(), std::io::Error> {
        let metadata = tokio::fs::metadata(source).await?;

        if metadata.is_file() {
            tokio::fs::remove_file(source).await
        } else if metadata.is_dir() {
            tokio::fs::remove_dir_all(source).await
        } else {
            Ok(())
        }
    }
}

/// Output from file copy job
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyOutput {
    pub copied_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub duration: Duration,
    pub failed_copies: Vec<CopyError>,
    pub is_move_operation: bool,
}

impl From<FileCopyOutput> for JobOutput {
    fn from(output: FileCopyOutput) -> Self {
        if output.is_move_operation {
            JobOutput::FileMove {
                moved_count: output.copied_count,
                failed_count: output.failed_count,
                total_bytes: output.total_bytes,
            }
        } else {
            JobOutput::FileCopy {
                copied_count: output.copied_count,
                total_bytes: output.total_bytes,
            }
        }
    }
}

/// Backward compatibility wrapper for move operations
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct MoveJob {
    pub sources: SdPathBatch,
    pub destination: SdPath,
    pub mode: MoveMode,
    pub overwrite: bool,
    pub preserve_timestamps: bool,
}

impl Job for MoveJob {
    const NAME: &'static str = "move_files";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Move or rename files and directories");
}

#[async_trait::async_trait]
impl JobHandler for MoveJob {
    type Output = MoveOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Convert to FileCopyJob with move options
        let mut copy_options = CopyOptions::default();
        copy_options.delete_after_copy = true;
        copy_options.move_mode = Some(self.mode.clone());
        copy_options.overwrite = self.overwrite;
        copy_options.preserve_timestamps = self.preserve_timestamps;

        let mut copy_job = FileCopyJob {
            sources: self.sources.clone(),
            destination: self.destination.clone(),
            options: copy_options,
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        };

        // Run the copy job
        let copy_output = copy_job.run(ctx).await?;

        // Convert output to move format
        Ok(MoveOutput {
            moved_count: copy_output.copied_count,
            failed_count: copy_output.failed_count,
            total_bytes: copy_output.total_bytes,
            duration: copy_output.duration,
            failed_moves: copy_output.failed_copies.into_iter().map(|e| MoveError {
                source: e.source,
                destination: e.destination,
                error: e.error,
            }).collect(),
        })
    }
}

impl MoveJob {
    /// Create a new move job
    pub fn new(sources: SdPathBatch, destination: SdPath, mode: MoveMode) -> Self {
        Self {
            sources,
            destination,
            mode,
            overwrite: false,
            preserve_timestamps: true,
        }
    }

    /// Create an empty job (used by derive macro)
    pub fn empty() -> Self {
        Self {
            sources: SdPathBatch::new(Vec::new()),
            destination: SdPath::new(uuid::Uuid::new_v4(), PathBuf::new()),
            mode: MoveMode::Move,
            overwrite: false,
            preserve_timestamps: true,
        }
    }

    /// Create a rename operation
    pub fn rename(source: SdPath, new_name: String) -> Self {
        let destination = SdPath::new(
            source.device_id,
            source.path.with_file_name(new_name)
        );

        Self::new(
            SdPathBatch::new(vec![source]),
            destination,
            MoveMode::Rename
        )
    }
}

/// Error information for failed moves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveError {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub error: String,
}

/// Output from move operations
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveOutput {
    pub moved_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub duration: Duration,
    pub failed_moves: Vec<MoveError>,
}

impl From<MoveOutput> for JobOutput {
    fn from(output: MoveOutput) -> Self {
        JobOutput::FileMove {
            moved_count: output.moved_count,
            failed_count: output.failed_count,
            total_bytes: output.total_bytes,
        }
    }
}