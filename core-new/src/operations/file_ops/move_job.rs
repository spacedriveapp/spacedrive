//! Move/rename job implementation

use crate::{
    infrastructure::jobs::prelude::*,
    shared::types::{SdPath, SdPathBatch},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::Instant,
};
use tokio::fs;
use uuid::Uuid;

/// Move operation modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveMode {
    /// Move files to a new location
    Move,
    /// Rename a single file/directory
    Rename,
    /// Cut and paste operation (same as move but different UX context)
    Cut,
}

/// Move job for relocating files and directories
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveJob {
    pub sources: SdPathBatch,
    pub destination: SdPath,
    pub mode: MoveMode,
    pub overwrite: bool,
    pub preserve_timestamps: bool,
    
    // Internal state for resumption
    #[serde(skip)]
    completed_moves: Vec<usize>,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
}

/// Move progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveProgress {
    pub current_file: String,
    pub files_moved: usize,
    pub total_files: usize,
    pub bytes_moved: u64,
    pub total_bytes: u64,
    pub current_operation: String,
    pub estimated_remaining: Option<std::time::Duration>,
}

impl JobProgress for MoveProgress {}

impl Job for MoveJob {
    const NAME: &'static str = "move_files";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Move or rename files and directories");
}

#[async_trait::async_trait]
impl JobHandler for MoveJob {
    type Output = MoveOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log(format!("Starting {} operation on {} files", 
            match self.mode {
                MoveMode::Move => "move",
                MoveMode::Rename => "rename", 
                MoveMode::Cut => "cut",
            },
            self.sources.paths.len()
        ));
        
        // Validate inputs
        self.validate_operation(&ctx).await?;
        
        // Group by device for efficient processing
        let by_device: HashMap<Uuid, Vec<SdPath>> = self.sources.by_device()
            .into_iter()
            .map(|(device_id, paths)| (device_id, paths.into_iter().cloned().collect()))
            .collect();
        let total_files = self.sources.paths.len();
        let mut moved_count = 0;
        let mut total_bytes = 0u64;
        let mut failed_moves = Vec::new();
        
        // Calculate total size for progress
        let estimated_total_bytes = self.calculate_total_size(&ctx).await?;
        
        // Process each device group
        for (device_id, device_paths) in by_device {
            ctx.check_interrupt().await?;
            
            if device_id == self.destination.device_id {
                // Same device - can use efficient rename
                self.process_same_device_moves(
                    device_paths.iter().collect(), 
                    &ctx, 
                    &mut moved_count,
                    &mut total_bytes,
                    &mut failed_moves,
                    total_files,
                    estimated_total_bytes
                ).await?;
            } else {
                // Cross-device - need to copy then delete
                self.process_cross_device_moves(
                    device_paths.iter().collect(),
                    &ctx,
                    &mut moved_count, 
                    &mut total_bytes,
                    &mut failed_moves,
                    total_files,
                    estimated_total_bytes
                ).await?;
            }
        }
        
        ctx.log(format!("Move operation completed: {} moved, {} failed", 
            moved_count, failed_moves.len()));
        
        Ok(MoveOutput {
            moved_count,
            failed_count: failed_moves.len(),
            total_bytes,
            duration: self.started_at.elapsed(),
            failed_moves,
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
            completed_moves: Vec::new(),
            started_at: Instant::now(),
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
    
    /// Validate the move operation
    async fn validate_operation(&self, ctx: &JobContext<'_>) -> JobResult<()> {
        // Check source paths exist
        for (i, source) in self.sources.paths.iter().enumerate() {
            if let Some(local_path) = source.as_local_path() {
                if !fs::try_exists(local_path).await.unwrap_or(false) {
                    return Err(JobError::execution(format!(
                        "Source file does not exist: {}", 
                        local_path.display()
                    )));
                }
            }
        }
        
        // Check destination directory exists (for move operations)
        if matches!(self.mode, MoveMode::Move | MoveMode::Cut) {
            if let Some(dest_parent) = self.destination.path.parent() {
                if let Some(local_dest) = self.destination.as_local_path() {
                    if !fs::try_exists(dest_parent).await.unwrap_or(false) {
                        return Err(JobError::execution(format!(
                            "Destination directory does not exist: {}", 
                            dest_parent.display()
                        )));
                    }
                }
            }
        }
        
        // Check for self-moves
        for source in &self.sources.paths {
            if source.path == self.destination.path {
                return Err(JobError::execution("Cannot move file to itself"));
            }
            
            // Check if moving into subdirectory of itself
            if self.destination.path.starts_with(&source.path) {
                return Err(JobError::execution("Cannot move directory into itself"));
            }
        }
        
        Ok(())
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
            let metadata = fs::metadata(&current_path).await?;
            
            if metadata.is_file() {
                total += metadata.len();
            } else if metadata.is_dir() {
                let mut dir = fs::read_dir(&current_path).await?;
                while let Some(entry) = dir.next_entry().await? {
                    stack.push(entry.path());
                }
            }
        }
        
        Ok(total)
    }
    
    /// Process moves within the same device (efficient rename)
    async fn process_same_device_moves(
        &mut self,
        paths: Vec<&SdPath>,
        ctx: &JobContext<'_>,
        moved_count: &mut usize,
        total_bytes: &mut u64,
        failed_moves: &mut Vec<MoveError>,
        total_files: usize,
        estimated_total_bytes: u64,
    ) -> JobResult<()> {
        for source in paths {
            ctx.check_interrupt().await?;
            
            if let Some(local_source) = source.as_local_path() {
                let dest_path = match self.mode {
                    MoveMode::Rename => &self.destination.path,
                    _ => &self.destination.path.join(
                        local_source.file_name().unwrap_or_default()
                    ),
                };
                
                ctx.progress(Progress::structured(MoveProgress {
                    current_file: local_source.display().to_string(),
                    files_moved: *moved_count,
                    total_files,
                    bytes_moved: *total_bytes,
                    total_bytes: estimated_total_bytes,
                    current_operation: "Moving".to_string(),
                    estimated_remaining: None,
                }));
                
                match self.move_local_file(local_source, dest_path).await {
                    Ok(bytes) => {
                        *moved_count += 1;
                        *total_bytes += bytes;
                        ctx.log(format!("Moved: {} -> {}", 
                            local_source.display(), dest_path.display()));
                    }
                    Err(e) => {
                        failed_moves.push(MoveError {
                            source: local_source.to_path_buf(),
                            destination: dest_path.clone(),
                            error: e.to_string(),
                        });
                        ctx.add_non_critical_error(format!(
                            "Failed to move {}: {}", local_source.display(), e
                        ));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Process cross-device moves (copy + delete)
    async fn process_cross_device_moves(
        &mut self,
        paths: Vec<&SdPath>,
        ctx: &JobContext<'_>,
        moved_count: &mut usize,
        total_bytes: &mut u64,
        failed_moves: &mut Vec<MoveError>,
        total_files: usize,
        estimated_total_bytes: u64,
    ) -> JobResult<()> {
        for source in paths {
            ctx.check_interrupt().await?;
            
            if let Some(local_source) = source.as_local_path() {
                let dest_path = self.destination.path.join(
                    local_source.file_name().unwrap_or_default()
                );
                
                ctx.progress(Progress::structured(MoveProgress {
                    current_file: local_source.display().to_string(),
                    files_moved: *moved_count,
                    total_files,
                    bytes_moved: *total_bytes,
                    total_bytes: estimated_total_bytes,
                    current_operation: "Copying (cross-device)".to_string(),
                    estimated_remaining: None,
                }));
                
                match self.copy_then_delete_file(local_source, &dest_path).await {
                    Ok(bytes) => {
                        *moved_count += 1;
                        *total_bytes += bytes;
                        ctx.log(format!("Moved (cross-device): {} -> {}", 
                            local_source.display(), dest_path.display()));
                    }
                    Err(e) => {
                        failed_moves.push(MoveError {
                            source: local_source.to_path_buf(),
                            destination: dest_path,
                            error: e.to_string(),
                        });
                        ctx.add_non_critical_error(format!(
                            "Failed to move {}: {}", local_source.display(), e
                        ));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Move a file on the same device (atomic rename)
    async fn move_local_file(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<u64, std::io::Error> {
        // Get file size before moving
        let size = fs::metadata(source).await?.len();
        
        // Check if destination exists
        if !self.overwrite && fs::try_exists(destination).await? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Destination already exists and overwrite is disabled"
            ));
        }
        
        // Create destination directory if needed
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Atomic rename
        fs::rename(source, destination).await?;
        
        Ok(size)
    }
    
    /// Copy then delete for cross-device moves
    async fn copy_then_delete_file(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<u64, std::io::Error> {
        // Create destination directory if needed
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Copy the file
        let size = if fs::metadata(source).await?.is_file() {
            fs::copy(source, destination).await?
        } else {
            self.copy_directory_recursive(source, destination).await?
        };
        
        // Preserve timestamps if requested
        if self.preserve_timestamps {
            let metadata = fs::metadata(source).await?;
            if let (Ok(accessed), Ok(modified)) = (metadata.accessed(), metadata.modified()) {
                // Note: Setting timestamps requires platform-specific code
                // This is a simplified version
            }
        }
        
        // Delete source after successful copy
        if fs::metadata(source).await?.is_file() {
            fs::remove_file(source).await?;
        } else {
            fs::remove_dir_all(source).await?;
        }
        
        Ok(size)
    }
    
    /// Copy a directory using iterative approach
    async fn copy_directory_recursive(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<u64, std::io::Error> {
        fs::create_dir_all(destination).await?;
        let mut total_size = 0u64;
        let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];
        
        while let Some((src_path, dest_path)) = stack.pop() {
            if src_path.is_file() {
                total_size += fs::copy(&src_path, &dest_path).await?;
            } else if src_path.is_dir() {
                fs::create_dir_all(&dest_path).await?;
                let mut dir = fs::read_dir(&src_path).await?;
                
                while let Some(entry) = dir.next_entry().await? {
                    let entry_src = entry.path();
                    let entry_dest = dest_path.join(entry.file_name());
                    stack.push((entry_src, entry_dest));
                }
            }
        }
        
        Ok(total_size)
    }
}

/// Error information for failed moves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveError {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub error: String,
}

/// Job output for move operations
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveOutput {
    pub moved_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub duration: std::time::Duration,
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