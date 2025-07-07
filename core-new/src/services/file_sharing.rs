//! File sharing service providing high-level file transfer operations

use crate::{
	context::CoreContext,
	operations::files::copy::{CopyOptions, FileCopyJob},
	services::networking::protocols::file_transfer::FileMetadata,
	shared::types::SdPath,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::SystemTime};
use uuid::Uuid;

/// File sharing service
pub struct FileSharingService {
	context: Arc<CoreContext>,
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

impl FileSharingService {
	/// Create a new file sharing service
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self { context }
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
					self.copy_to_paired_device(files, device_info.device_id, options)
						.await
				} else {
					self.share_via_spacedrop(files, vec![device_info], options)
						.await
				}
			}
		}
	}

	/// Share files with a paired device
	pub async fn share_with_device(
		&self,
		files: Vec<PathBuf>,
		device_id: Uuid,
		destination_path: Option<PathBuf>,
	) -> Result<TransferId, SharingError> {
		// Get networking service from context
		let _networking = self
			.context
			.get_networking()
			.await
			.ok_or(SharingError::NetworkingUnavailable)?;

		// Get the current library to access its job manager
		let library = self
			.context
			.library_manager
			.get_primary_library()
			.await
			.ok_or(SharingError::JobError(
				"No active library for job dispatch".to_string(),
			))?;

		// Create and dispatch the FileCopyJob
		let job_manager = library.jobs();
		let sources = files.into_iter().map(SdPath::local).collect();
		let destination = SdPath::new(device_id, destination_path.unwrap_or_default());
		let copy_job = FileCopyJob::from_paths(sources, destination);

		let handle = job_manager
			.dispatch(copy_job)
			.await
			.map_err(|e| SharingError::JobError(e.to_string()))?;

		Ok(TransferId::JobId(handle.id().into()))
	}

	/// Copy files to a paired device (trusted, automatic)
	async fn copy_to_paired_device(
		&self,
		files: Vec<PathBuf>,
		device_id: Uuid,
		options: SharingOptions,
	) -> Result<Vec<TransferId>, SharingError> {
		let library = self
			.context
			.library_manager
			.get_primary_library()
			.await
			.ok_or(SharingError::JobError(
				"No active library for job dispatch".to_string(),
			))?;

		let job_manager = library.jobs();

		// Create SdPath objects for sources
		let sources: Vec<SdPath> = files.into_iter().map(|path| SdPath::local(path)).collect();

		let destination = SdPath::new(device_id, options.destination_path);

		// Create FileCopyJob for cross-device operation
		let copy_job = FileCopyJob::from_paths(sources, destination).with_options(CopyOptions {
			overwrite: options.overwrite,
			verify_checksum: true,
			preserve_timestamps: options.preserve_timestamps,
			delete_after_copy: false,
			move_mode: None,
			copy_method: crate::operations::files::copy::input::CopyMethod::Auto,
		});

		// Submit job to job system
		let handle = job_manager
			.dispatch(copy_job)
			.await
			.map_err(|e| SharingError::JobError(e.to_string()))?;

		Ok(vec![TransferId::JobId(handle.id().into())])
	}

	/// Share files via Spacedrop (ephemeral, requires consent)
	async fn initiate_spacedrop(
		&self,
		files: Vec<PathBuf>,
		options: SharingOptions,
	) -> Result<Vec<TransferId>, SharingError> {
		let _networking = self
			.context
			.get_networking()
			.await
			.ok_or(SharingError::NetworkingUnavailable)?;

		let mut transfer_ids = Vec::new();

		for file_path in files {
			let _file_metadata = self.create_file_metadata(&file_path).await?;

			// TODO: Implement Spacedrop protocol
			// For now, simulate the process
			let transfer_id = Uuid::new_v4();

			transfer_ids.push(TransferId::SpacedropId(transfer_id));
		}

		Ok(transfer_ids)
	}

	/// Share files via Spacedrop with specific devices
	async fn share_via_spacedrop(
		&self,
		files: Vec<PathBuf>,
		_target_devices: Vec<DeviceInfo>,
		options: SharingOptions,
	) -> Result<Vec<TransferId>, SharingError> {
		// For now, use the same implementation as general Spacedrop
		self.initiate_spacedrop(files, options).await
	}

	/// Create file metadata for sharing
	pub async fn create_file_metadata(
		&self,
		file_path: &PathBuf,
	) -> Result<FileMetadata, SharingError> {
		let metadata = tokio::fs::metadata(file_path)
			.await
			.map_err(|_e| SharingError::FileNotFound(file_path.clone()))?;

		Ok(FileMetadata {
			name: file_path
				.file_name()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string(),
			size: metadata.len(),
			modified: metadata.modified().ok(),
			is_directory: metadata.is_dir(),
			checksum: None,  // Will be calculated during transfer
			mime_type: None, // TODO: Add MIME type detection
		})
	}

	/// Get nearby devices available for sharing
	pub async fn get_nearby_devices(&self) -> Result<Vec<DeviceInfo>, SharingError> {
		let _networking = self
			.context
			.get_networking()
			.await
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
	pub async fn get_transfer_status(
		&self,
		transfer_id: &TransferId,
	) -> Result<TransferStatus, SharingError> {
		match transfer_id {
			TransferId::JobId(job_id) => {
				let library = self
					.context
					.library_manager
					.get_primary_library()
					.await
					.ok_or(SharingError::JobError(
						"No active library for job dispatch".to_string(),
					))?;

				let job_manager = library.jobs();

				// Query job system for status
				let job_info = job_manager
					.get_job_info(*job_id)
					.await
					.map_err(|e| SharingError::JobError(e.to_string()))?;

				if let Some(info) = job_info {
					let state = match info.status {
						crate::infrastructure::jobs::types::JobStatus::Queued => {
							TransferState::Pending
						}
						crate::infrastructure::jobs::types::JobStatus::Running => {
							TransferState::Active
						}
						crate::infrastructure::jobs::types::JobStatus::Paused => {
							TransferState::Active
						}
						crate::infrastructure::jobs::types::JobStatus::Completed => {
							TransferState::Completed
						}
						crate::infrastructure::jobs::types::JobStatus::Failed => {
							TransferState::Failed
						}
						crate::infrastructure::jobs::types::JobStatus::Cancelled => {
							TransferState::Cancelled
						}
					};

					Ok(TransferStatus {
						id: transfer_id.clone(),
						state,
						progress: TransferProgress {
							bytes_transferred: 0, // TODO: Extract from job progress
							total_bytes: 0,       // TODO: Extract from job progress
							files_transferred: 0, // TODO: Extract from job progress
							total_files: 0,       // TODO: Extract from job progress
							estimated_remaining: None,
						},
						error: info.error_message,
					})
				} else {
					Err(SharingError::TransferFailed("Job not found".to_string()))
				}
			}
			TransferId::SpacedropId(_session_id) => {
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
				let library = self
					.context
					.library_manager
					.get_primary_library()
					.await
					.ok_or(SharingError::JobError(
						"No active library for job dispatch".to_string(),
					))?;

				let job_manager = library.jobs();

				// Get the job handle and cancel it
				if let Some(_job_handle) = job_manager.get_job((*job_id).into()).await {
					// TODO: Implement cancel functionality on JobHandle
					Ok(())
				} else {
					Err(SharingError::TransferFailed("Job not found".to_string()))
				}
			}
			TransferId::SpacedropId(_session_id) => {
				// TODO: Cancel Spacedrop session
				Ok(())
			}
		}
	}

	/// Get all active transfers
	pub async fn get_active_transfers(&self) -> Result<Vec<TransferStatus>, SharingError> {
		let library = self
			.context
			.library_manager
			.get_primary_library()
			.await
			.ok_or(SharingError::JobError(
				"No active library for job dispatch".to_string(),
			))?;

		let job_manager = library.jobs();

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
					crate::infrastructure::jobs::types::JobStatus::Completed => {
						TransferState::Completed
					}
					crate::infrastructure::jobs::types::JobStatus::Failed => TransferState::Failed,
					crate::infrastructure::jobs::types::JobStatus::Cancelled => {
						TransferState::Cancelled
					}
				};

				transfers.push(TransferStatus {
					id: TransferId::JobId(job_info.id),
					state,
					progress: TransferProgress {
						bytes_transferred: 0, // TODO: Extract from job progress
						total_bytes: 0,       // TODO: Extract from job progress
						files_transferred: 0, // TODO: Extract from job progress
						total_files: 0,       // TODO: Extract from job progress
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
	use crate::{
		device::DeviceManager, infrastructure::events::EventBus,
		keys::library_key_manager::LibraryKeyManager, library::LibraryManager,
	};
	use tempfile::tempdir;

	#[tokio::test]
	async fn test_file_sharing_service_creation() {
		let events = Arc::new(EventBus::default());
		let device_manager = Arc::new(DeviceManager::init().unwrap());
		let library_manager = Arc::new(LibraryManager::new_with_dir(
			std::env::temp_dir().join("test_libraries"),
			events.clone(),
		));
		let volume_manager = Arc::new(crate::volume::VolumeManager::new(
			crate::volume::VolumeDetectionConfig::default(),
			events.clone(),
		));
		let library_key_manager = Arc::new(LibraryKeyManager::new().unwrap());
		let context = Arc::new(CoreContext::new(
			events,
			device_manager,
			library_manager,
			volume_manager,
			library_key_manager,
		));

		let _file_sharing = FileSharingService::new(context);
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
	async fn test_create_file_metadata() {
		let events = Arc::new(EventBus::default());
		let device_manager = Arc::new(DeviceManager::init().unwrap());
		let library_manager = Arc::new(LibraryManager::new_with_dir(
			std::env::temp_dir().join("test_libraries"),
			events.clone(),
		));
		let volume_manager = Arc::new(crate::volume::VolumeManager::new(
			crate::volume::VolumeDetectionConfig::default(),
			events.clone(),
		));
		let library_key_manager = Arc::new(LibraryKeyManager::new().unwrap());
		let context = Arc::new(CoreContext::new(
			events,
			device_manager,
			library_manager,
			volume_manager,
			library_key_manager,
		));
		let file_sharing = FileSharingService::new(context);

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
