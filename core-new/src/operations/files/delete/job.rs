//! Delete job implementation

use crate::{
    infrastructure::jobs::prelude::*,
    shared::types::SdPathBatch,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::fs;

/// Delete operation modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeleteMode {
    /// Move to trash/recycle bin
    Trash,
    /// Permanent deletion (cannot be undone)
    Permanent,
    /// Secure deletion (overwrite data)
    Secure,
}

/// Options for file delete operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOptions {
    pub permanent: bool,
    pub recursive: bool,
}

impl Default for DeleteOptions {
    fn default() -> Self {
        Self {
            permanent: false,
            recursive: false,
        }
    }
}

/// Delete job for removing files and directories
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteJob {
    pub targets: SdPathBatch,
    pub mode: DeleteMode,
    pub confirm_permanent: bool,
    
    // Internal state for resumption
    #[serde(skip)]
    completed_deletions: Vec<usize>,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
}

/// Delete progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProgress {
    pub current_file: String,
    pub files_deleted: usize,
    pub total_files: usize,
    pub bytes_deleted: u64,
    pub total_bytes: u64,
    pub current_operation: String,
    pub estimated_remaining: Option<Duration>,
}

impl JobProgress for DeleteProgress {}

impl Job for DeleteJob {
    const NAME: &'static str = "delete_files";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Delete files and directories");
}

#[async_trait::async_trait]
impl JobHandler for DeleteJob {
    type Output = DeleteOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log(format!("Starting {} deletion of {} files", 
            match self.mode {
                DeleteMode::Trash => "trash",
                DeleteMode::Permanent => "permanent",
                DeleteMode::Secure => "secure",
            },
            self.targets.paths.len()
        ));
        
        // Safety check for permanent deletion
        if matches!(self.mode, DeleteMode::Permanent | DeleteMode::Secure) && !self.confirm_permanent {
            return Err(JobError::execution(
                "Permanent deletion requires explicit confirmation"
            ));
        }
        
        // Validate targets exist
        self.validate_targets(&ctx).await?;
        
        let total_files = self.targets.paths.len();
        let mut deleted_count = 0;
        let mut total_bytes = 0u64;
        let mut failed_deletions = Vec::new();
        
        // Calculate total size for progress
        let estimated_total_bytes = self.calculate_total_size(&ctx).await?;
        
        // Process deletions
        for (index, target) in self.targets.paths.iter().enumerate() {
            ctx.check_interrupt().await?;
            
            // Skip if already processed (for resumption)
            if self.completed_deletions.contains(&index) {
                continue;
            }
            
            if let Some(local_path) = target.as_local_path() {
                ctx.progress(Progress::structured(DeleteProgress {
                    current_file: local_path.display().to_string(),
                    files_deleted: deleted_count,
                    total_files,
                    bytes_deleted: total_bytes,
                    total_bytes: estimated_total_bytes,
                    current_operation: self.get_operation_name(),
                    estimated_remaining: None,
                }));
                
                match self.delete_path(local_path, &ctx).await {
                    Ok(bytes) => {
                        deleted_count += 1;
                        total_bytes += bytes;
                        self.completed_deletions.push(index);
                        
                        ctx.log(format!("Deleted: {}", local_path.display()));
                        
                        // Checkpoint every 10 files
                        if deleted_count % 10 == 0 {
                            ctx.checkpoint().await?;
                        }
                    }
                    Err(e) => {
                        failed_deletions.push(DeleteError {
                            path: local_path.to_path_buf(),
                            error: e.to_string(),
                        });
                        ctx.add_non_critical_error(format!(
                            "Failed to delete {}: {}", local_path.display(), e
                        ));
                    }
                }
            }
        }
        
        ctx.log(format!("Delete operation completed: {} deleted, {} failed", 
            deleted_count, failed_deletions.len()));
        
        Ok(DeleteOutput {
            deleted_count,
            failed_count: failed_deletions.len(),
            total_bytes,
            duration: self.started_at.elapsed(),
            failed_deletions,
            mode: self.mode.clone(),
        })
    }
}

impl DeleteJob {
    /// Create a new delete job
    pub fn new(targets: SdPathBatch, mode: DeleteMode) -> Self {
        Self {
            targets,
            mode,
            confirm_permanent: false,
            completed_deletions: Vec::new(),
            started_at: Instant::now(),
        }
    }
    
    /// Create a trash operation
    pub fn trash(targets: SdPathBatch) -> Self {
        Self::new(targets, DeleteMode::Trash)
    }
    
    /// Create a permanent delete operation (requires confirmation)
    pub fn permanent(targets: SdPathBatch, confirmed: bool) -> Self {
        let mut job = Self::new(targets, DeleteMode::Permanent);
        job.confirm_permanent = confirmed;
        job
    }
    
    /// Create a secure delete operation (requires confirmation)
    pub fn secure(targets: SdPathBatch, confirmed: bool) -> Self {
        let mut job = Self::new(targets, DeleteMode::Secure);
        job.confirm_permanent = confirmed;
        job
    }
    
    /// Validate that all targets exist
    async fn validate_targets(&self, ctx: &JobContext<'_>) -> JobResult<()> {
        for target in &self.targets.paths {
            if let Some(local_path) = target.as_local_path() {
                if !fs::try_exists(local_path).await.unwrap_or(false) {
                    return Err(JobError::execution(format!(
                        "Target does not exist: {}", 
                        local_path.display()
                    )));
                }
            }
        }
        Ok(())
    }
    
    /// Calculate total size for progress reporting
    async fn calculate_total_size(&self, ctx: &JobContext<'_>) -> JobResult<u64> {
        let mut total = 0u64;
        
        for target in &self.targets.paths {
            if let Some(local_path) = target.as_local_path() {
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
    
    /// Delete a single path according to the specified mode
    async fn delete_path(
        &self,
        path: &std::path::Path,
        ctx: &JobContext<'_>
    ) -> Result<u64, std::io::Error> {
        let size = self.get_path_size(path).await.unwrap_or(0);
        
        match self.mode {
            DeleteMode::Trash => {
                self.move_to_trash(path).await?;
            }
            DeleteMode::Permanent => {
                self.permanent_delete(path).await?;
            }
            DeleteMode::Secure => {
                self.secure_delete(path).await?;
            }
        }
        
        Ok(size)
    }
    
    /// Move file to system trash/recycle bin
    async fn move_to_trash(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        // On Unix systems, we typically move to ~/.local/share/Trash/files/
        // On Windows, we'd use the Recycle Bin API
        // On macOS, we'd move to ~/.Trash/
        
        #[cfg(unix)]
        {
            self.move_to_trash_unix(path).await?;
        }
        
        #[cfg(windows)]
        {
            self.move_to_trash_windows(path).await?;
        }
        
        #[cfg(target_os = "macos")]
        {
            self.move_to_trash_macos(path).await?;
        }
        
        Ok(())
    }
    
    #[cfg(unix)]
    async fn move_to_trash_unix(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        // Follow XDG Trash specification
        let home = std::env::var("HOME").map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set")
        })?;
        
        let trash_dir = std::path::Path::new(&home).join(".local/share/Trash/files");
        fs::create_dir_all(&trash_dir).await?;
        
        let filename = path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
        })?;
        
        let trash_path = trash_dir.join(filename);
        
        // Find unique name if file already exists in trash
        let final_trash_path = self.find_unique_trash_name(&trash_path).await?;
        
        // Move to trash
        fs::rename(path, final_trash_path).await?;
        
        Ok(())
    }
    
    #[cfg(windows)]
    async fn move_to_trash_windows(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        // On Windows, we'd typically use the SHFileOperation API
        // For now, we'll use a simple implementation that moves to a temp trash folder
        let temp_dir = std::env::temp_dir().join("spacedrive_trash");
        fs::create_dir_all(&temp_dir).await?;
        
        let filename = path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
        })?;
        
        let trash_path = temp_dir.join(filename);
        let final_trash_path = self.find_unique_trash_name(&trash_path).await?;
        
        fs::rename(path, final_trash_path).await?;
        
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn move_to_trash_macos(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let home = std::env::var("HOME").map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set")
        })?;
        
        let trash_dir = std::path::Path::new(&home).join(".Trash");
        
        let filename = path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
        })?;
        
        let trash_path = trash_dir.join(filename);
        let final_trash_path = self.find_unique_trash_name(&trash_path).await?;
        
        fs::rename(path, final_trash_path).await?;
        
        Ok(())
    }
    
    /// Find a unique name in the trash directory
    async fn find_unique_trash_name(&self, base_path: &std::path::Path) -> Result<PathBuf, std::io::Error> {
        let mut candidate = base_path.to_path_buf();
        let mut counter = 1;
        
        while fs::try_exists(&candidate).await? {
            let stem = base_path.file_stem().unwrap_or_default();
            let extension = base_path.extension();
            
            let new_name = if let Some(ext) = extension {
                format!("{} ({})", stem.to_string_lossy(), counter)
            } else {
                format!("{} ({})", stem.to_string_lossy(), counter)
            };
            
            candidate = base_path.with_file_name(new_name);
            if let Some(ext) = extension {
                candidate.set_extension(ext);
            }
            
            counter += 1;
        }
        
        Ok(candidate)
    }
    
    /// Permanently delete file or directory
    async fn permanent_delete(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let metadata = fs::metadata(path).await?;
        
        if metadata.is_file() {
            fs::remove_file(path).await?;
        } else if metadata.is_dir() {
            fs::remove_dir_all(path).await?;
        }
        
        Ok(())
    }
    
    /// Securely delete file by overwriting with random data
    async fn secure_delete(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let metadata = fs::metadata(path).await?;
        
        if metadata.is_file() {
            // Overwrite file with random data multiple times
            self.secure_overwrite_file(path, metadata.len()).await?;
            fs::remove_file(path).await?;
        } else if metadata.is_dir() {
            // Recursively secure delete directory contents
            self.secure_delete_directory(path).await?;
            fs::remove_dir_all(path).await?;
        }
        
        Ok(())
    }
    
    /// Securely overwrite a file with random data
    async fn secure_overwrite_file(&self, path: &std::path::Path, size: u64) -> Result<(), std::io::Error> {
        use rand::RngCore;
        use tokio::io::{AsyncWriteExt, AsyncSeekExt};
        
        // Open file for writing
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(path)
            .await?;
        
        // Overwrite with random data (3 passes)
        for _ in 0..3 {
            file.seek(std::io::SeekFrom::Start(0)).await?;
            
            // Write random data in chunks
            let mut remaining = size;
            
            while remaining > 0 {
                let chunk_size = std::cmp::min(remaining, 64 * 1024) as usize; // 64KB chunks
                
                // Generate random data synchronously to avoid Send issues
                let buffer = {
                    let mut rng = rand::thread_rng();
                    let mut buf = vec![0u8; chunk_size];
                    rng.fill_bytes(&mut buf);
                    buf
                };
                
                file.write_all(&buffer).await?;
                remaining -= chunk_size as u64;
            }
            
            file.flush().await?;
            file.sync_all().await?;
        }
        
        Ok(())
    }
    
    /// Secure delete directory using iterative approach
    async fn secure_delete_directory(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let mut stack = vec![path.to_path_buf()];
        
        while let Some(current_path) = stack.pop() {
            let mut dir = fs::read_dir(&current_path).await?;
            
            while let Some(entry) = dir.next_entry().await? {
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    let metadata = fs::metadata(&entry_path).await?;
                    self.secure_overwrite_file(&entry_path, metadata.len()).await?;
                    fs::remove_file(&entry_path).await?;
                } else if entry_path.is_dir() {
                    stack.push(entry_path);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get operation name for progress display
    fn get_operation_name(&self) -> String {
        match self.mode {
            DeleteMode::Trash => "Moving to trash".to_string(),
            DeleteMode::Permanent => "Permanently deleting".to_string(),
            DeleteMode::Secure => "Securely deleting".to_string(),
        }
    }
}

/// Error information for failed deletions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteError {
    pub path: PathBuf,
    pub error: String,
}

/// Job output for delete operations
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOutput {
    pub deleted_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub duration: Duration,
    pub failed_deletions: Vec<DeleteError>,
    pub mode: DeleteMode,
}

impl From<DeleteOutput> for JobOutput {
    fn from(output: DeleteOutput) -> Self {
        JobOutput::FileDelete {
            deleted_count: output.deleted_count,
            failed_count: output.failed_count,
            total_bytes: output.total_bytes,
        }
    }
}