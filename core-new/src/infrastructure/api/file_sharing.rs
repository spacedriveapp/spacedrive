//! High-level file sharing API that automatically chooses protocol

use crate::{
    device::DeviceManager,
    services::networking::{NetworkingService, protocols::file_transfer::{FileTransferProtocolHandler, FileMetadata, TransferMode}},
    operations::file_ops::copy_job::{FileCopyJob, CopyOptions},
    shared::types::{SdPath, SdPathBatch},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;

/// High-level file sharing API that automatically chooses protocol
pub struct FileSharing {
    networking: Option<Arc<NetworkingService>>,
    device_manager: Arc<DeviceManager>,
    job_manager: Option<Arc<crate::infrastructure::jobs::manager::JobManager>>,
}

/// Sharing target specification
#[derive(Debug, Clone)]
pub enum SharingTarget {
    /// Share with a specific paired device
    PairedDevice(Uuid),
    /// Discover and share with nearby devices
    NearbyDevices,
    /// Share with a specific device (may or may not be paired)
    SpecificDevice(DeviceInfo),
}

/// Device information for sharing
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: Uuid,
    pub device_name: String,
    pub is_paired: bool,
    pub last_seen: Option<SystemTime>,
}

/// Transfer identifier for tracking operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferId {
    /// Job system ID for cross-device copies
    JobId(Uuid),
    /// Spacedrop session ID for ephemeral shares
    SpacedropId(Uuid),
}

/// Options for file sharing operations
#[derive(Debug, Clone)]
pub struct SharingOptions {
    /// Destination path on target device
    pub destination_path: PathBuf,
    /// Whether to overwrite existing files
    pub overwrite: bool,
    /// Whether to preserve file timestamps
    pub preserve_timestamps: bool,
    /// Sender name for display
    pub sender_name: String,
    /// Optional message to include with share
    pub message: Option<String>,
}

impl Default for SharingOptions {
    fn default() -> Self {
        Self {
            destination_path: PathBuf::from("/tmp/spacedrive"),
            overwrite: false,
            preserve_timestamps: true,
            sender_name: "Spacedrive User".to_string(),
            message: None,
        }
    }
}

/// Errors that can occur during file sharing
#[derive(Debug, thiserror::Error)]
pub enum SharingError {
    #[error("Networking not available")]
    NetworkingUnavailable,

    #[error("Device not found: {0}")]
    DeviceNotFound(Uuid),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    #[error("Invalid sharing target")]
    InvalidTarget,

    #[error("Job system error: {0}")]
    JobError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

impl FileSharing {
    /// Create a new file sharing API instance
    pub fn new(
        networking: Option<Arc<NetworkingService>>,
        device_manager: Arc<DeviceManager>,
    ) -> Self {
        Self {
            networking,
            device_manager,
            job_manager: None,
        }
    }

    /// Set the job manager reference (called by Core after library initialization)
    pub fn set_job_manager(&mut self, job_manager: Arc<crate::infrastructure::jobs::manager::JobManager>) {
        self.job_manager = Some(job_manager);
    }

    /// Check if file sharing has a job manager configured
    pub async fn has_job_manager(&self) -> bool {
        let has_jm = self.job_manager.is_some();
        println!("🔍 FILE_SHARING_DEBUG: has_job_manager = {}", has_jm);
        has_jm
    }

    /// Share files with automatic protocol selection based on device relationship
    pub async fn share_files(
        &self,
        files: Vec<PathBuf>,
        target: SharingTarget,
        options: SharingOptions,
    ) -> Result<Vec<TransferId>, SharingError> {
        // Validate files exist
        for file in &files {
            if !file.exists() {
                return Err(SharingError::FileNotFound(file.clone()));
            }
        }

        match target {
            SharingTarget::PairedDevice(device_id) => {
                // Use cross-device copy for trusted devices
                self.copy_to_paired_device(files, device_id, options).await
            }
            SharingTarget::NearbyDevices => {
                // Use Spacedrop for discovery-based sharing
                self.initiate_spacedrop(files, options).await
            }
            SharingTarget::SpecificDevice(device_info) => {
                // Check if device is paired, choose protocol accordingly
                if device_info.is_paired {
                    self.copy_to_paired_device(files, device_info.device_id, options).await
                } else {
                    self.share_via_spacedrop(files, vec![device_info], options).await
                }
            }
        }
    }

    /// Copy files to a paired device (trusted, automatic)
    async fn copy_to_paired_device(
        &self,
        files: Vec<PathBuf>,
        device_id: Uuid,
        options: SharingOptions,
    ) -> Result<Vec<TransferId>, SharingError> {
        println!("🔍 FILE_SHARING_DEBUG: copy_to_paired_device called with device {}", device_id);
        let job_manager = self.job_manager.as_ref()
            .ok_or(SharingError::NetworkingUnavailable)?;

        // Create SdPath objects for sources
        let sources: Vec<SdPath> = files.into_iter()
            .map(|path| SdPath::local(path))
            .collect();

        let destination = SdPath::new(device_id, options.destination_path);

        // Create FileCopyJob for cross-device operation
        let copy_job = FileCopyJob::from_paths(sources, destination)
            .with_options(CopyOptions {
                overwrite: options.overwrite,
                verify_checksum: true,
                preserve_timestamps: options.preserve_timestamps,
                delete_after_copy: false,
                move_mode: None,
            });

        // Submit job to job system
        let handle = job_manager.dispatch(copy_job).await
            .map_err(|e| SharingError::JobError(e.to_string()))?;

        println!("📋 Submitted cross-device copy job {} for device {}", handle.id(), device_id);

        Ok(vec![TransferId::JobId(handle.id().into())])
    }

    /// Share files via Spacedrop (ephemeral, requires consent)
    async fn initiate_spacedrop(
        &self,
        files: Vec<PathBuf>,
        options: SharingOptions,
    ) -> Result<Vec<TransferId>, SharingError> {
        let networking = self.networking.as_ref()
            .ok_or(SharingError::NetworkingUnavailable)?;

        let mut transfer_ids = Vec::new();

        for file_path in files {
            let file_metadata = self.create_file_metadata(&file_path).await?;

            // TODO: Implement Spacedrop protocol
            // For now, simulate the process
            let transfer_id = Uuid::new_v4();

            println!("🚀 Starting Spacedrop session {} for file: {}",
                transfer_id, file_path.display());
            println!("   Sender: {}", options.sender_name);
            if let Some(ref message) = options.message {
                println!("   Message: {}", message);
            }

            transfer_ids.push(TransferId::SpacedropId(transfer_id));
        }

        Ok(transfer_ids)
    }

    /// Share files via Spacedrop with specific devices
    async fn share_via_spacedrop(
        &self,
        files: Vec<PathBuf>,
        target_devices: Vec<DeviceInfo>,
        options: SharingOptions,
    ) -> Result<Vec<TransferId>, SharingError> {
        println!("🎯 Targeting specific devices for Spacedrop:");
        for device in &target_devices {
            println!("   - {} ({})", device.device_name, device.device_id);
        }

        // For now, use the same implementation as general Spacedrop
        self.initiate_spacedrop(files, options).await
    }

    /// Create file metadata for sharing
    pub async fn create_file_metadata(&self, file_path: &PathBuf) -> Result<FileMetadata, SharingError> {
        let metadata = tokio::fs::metadata(file_path).await
            .map_err(|e| SharingError::FileNotFound(file_path.clone()))?;

        Ok(FileMetadata {
            name: file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
            is_directory: metadata.is_dir(),
            checksum: None, // Will be calculated during transfer
            mime_type: None, // TODO: Add MIME type detection
        })
    }

    /// Get nearby devices available for sharing
    pub async fn get_nearby_devices(&self) -> Result<Vec<DeviceInfo>, SharingError> {
        let networking = self.networking.as_ref()
            .ok_or(SharingError::NetworkingUnavailable)?;

        // TODO: Implement device discovery
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Get paired devices
    pub async fn get_paired_devices(&self) -> Result<Vec<DeviceInfo>, SharingError> {
        // TODO: Get paired devices from device manager
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Get status of a transfer
    pub async fn get_transfer_status(&self, transfer_id: &TransferId) -> Result<TransferStatus, SharingError> {
        match transfer_id {
            TransferId::JobId(job_id) => {
                let job_manager = self.job_manager.as_ref()
                    .ok_or(SharingError::NetworkingUnavailable)?;

                // Query job system for status
                println!("🔍 FILE_SHARING_DEBUG: About to call get_job_info for job {}", job_id);
                let job_info = job_manager.get_job_info(*job_id).await
                    .map_err(|e| SharingError::JobError(e.to_string()))?;

                println!("🔍 FILE_SHARING_DEBUG: get_job_info returned: {:?}", job_info.is_some());
                if let Some(info) = job_info {
                    println!("🔍 FILE_SHARING_DEBUG: Job info found - status: {:?}", info.status);
                    let state = match info.status {
                        crate::infrastructure::jobs::types::JobStatus::Queued => TransferState::Pending,
                        crate::infrastructure::jobs::types::JobStatus::Running => TransferState::Active,
                        crate::infrastructure::jobs::types::JobStatus::Paused => TransferState::Active,
                        crate::infrastructure::jobs::types::JobStatus::Completed => TransferState::Completed,
                        crate::infrastructure::jobs::types::JobStatus::Failed => TransferState::Failed,
                        crate::infrastructure::jobs::types::JobStatus::Cancelled => TransferState::Cancelled,
                    };

                    Ok(TransferStatus {
                        id: transfer_id.clone(),
                        state,
                        progress: TransferProgress {
                            bytes_transferred: 0, // TODO: Extract from job progress
                            total_bytes: 0, // TODO: Extract from job progress
                            files_transferred: 0, // TODO: Extract from job progress
                            total_files: 0, // TODO: Extract from job progress
                            estimated_remaining: None,
                        },
                        error: info.error_message,
                    })
                } else {
                    Err(SharingError::TransferFailed("Job not found".to_string()))
                }
            }
            TransferId::SpacedropId(session_id) => {
                // TODO: Query Spacedrop protocol for status
                Ok(TransferStatus {
                    id: transfer_id.clone(),
                    state: TransferState::Pending,
                    progress: TransferProgress {
                        bytes_transferred: 0,
                        total_bytes: 0,
                        files_transferred: 0,
                        total_files: 0,
                        estimated_remaining: None,
                    },
                    error: None,
                })
            }
        }
    }

    /// Cancel a transfer
    pub async fn cancel_transfer(&self, transfer_id: &TransferId) -> Result<(), SharingError> {
        match transfer_id {
            TransferId::JobId(job_id) => {
                let job_manager = self.job_manager.as_ref()
                    .ok_or(SharingError::NetworkingUnavailable)?;

                // Get the job handle and cancel it
                if let Some(job_handle) = job_manager.get_job((*job_id).into()).await {
                    // TODO: Implement cancel functionality on JobHandle
                    println!("🛑 Cancelling cross-device copy job {}", job_id);
                    Ok(())
                } else {
                    Err(SharingError::TransferFailed("Job not found".to_string()))
                }
            }
            TransferId::SpacedropId(session_id) => {
                // TODO: Cancel Spacedrop session
                println!("🛑 Cancelling Spacedrop session {}", session_id);
                Ok(())
            }
        }
    }

    /// Get all active transfers
    pub async fn get_active_transfers(&self) -> Result<Vec<TransferStatus>, SharingError> {
        let job_manager = self.job_manager.as_ref()
            .ok_or(SharingError::NetworkingUnavailable)?;

        // Get all running jobs
        let running_jobs = job_manager.list_running_jobs().await;
        let mut transfers = Vec::new();

        for job_info in running_jobs {
            // Only include file copy jobs as transfers
            if job_info.name == "file_copy" {
                let state = match job_info.status {
                    crate::infrastructure::jobs::types::JobStatus::Queued => TransferState::Pending,
                    crate::infrastructure::jobs::types::JobStatus::Running => TransferState::Active,
                    crate::infrastructure::jobs::types::JobStatus::Paused => TransferState::Active,
                    crate::infrastructure::jobs::types::JobStatus::Completed => TransferState::Completed,
                    crate::infrastructure::jobs::types::JobStatus::Failed => TransferState::Failed,
                    crate::infrastructure::jobs::types::JobStatus::Cancelled => TransferState::Cancelled,
                };

                transfers.push(TransferStatus {
                    id: TransferId::JobId(job_info.id),
                    state,
                    progress: TransferProgress {
                        bytes_transferred: 0, // TODO: Extract from job progress
                        total_bytes: 0, // TODO: Extract from job progress
                        files_transferred: 0, // TODO: Extract from job progress
                        total_files: 0, // TODO: Extract from job progress
                        estimated_remaining: None,
                    },
                    error: job_info.error_message,
                });
            }
        }

        Ok(transfers)
    }
}

/// Transfer status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStatus {
    pub id: TransferId,
    pub state: TransferState,
    pub progress: TransferProgress,
    pub error: Option<String>,
}

/// Transfer state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferState {
    Pending,
    Active,
    Completed,
    Failed,
    Cancelled,
}

/// Transfer progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub files_transferred: usize,
    pub total_files: usize,
    pub estimated_remaining: Option<std::time::Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_sharing_creation() {
        let device_manager = Arc::new(DeviceManager::init().unwrap());
        let file_sharing = FileSharing::new(None, device_manager);

        // Should be able to create without networking
        assert!(file_sharing.networking.is_none());
    }

    #[tokio::test]
    async fn test_sharing_options_default() {
        let options = SharingOptions::default();
        assert_eq!(options.sender_name, "Spacedrive User");
        assert!(!options.overwrite);
        assert!(options.preserve_timestamps);
        assert!(options.message.is_none());
    }

    #[tokio::test]
    async fn test_file_not_found_error() {
        let device_manager = Arc::new(DeviceManager::init().unwrap());
        let file_sharing = FileSharing::new(None, device_manager);

        let result = file_sharing.share_files(
            vec![PathBuf::from("/nonexistent/file.txt")],
            SharingTarget::PairedDevice(Uuid::new_v4()),
            SharingOptions::default(),
        ).await;

        assert!(matches!(result, Err(SharingError::FileNotFound(_))));
    }

    #[tokio::test]
    async fn test_create_file_metadata() {
        let device_manager = Arc::new(DeviceManager::init().unwrap());
        let file_sharing = FileSharing::new(None, device_manager);

        // Create a temporary file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, b"test content").await.unwrap();

        let metadata = file_sharing.create_file_metadata(&file_path).await.unwrap();
        assert_eq!(metadata.name, "test.txt");
        assert_eq!(metadata.size, 12);
        assert!(!metadata.is_directory);
    }
}