//! File copy job implementation

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
use tokio::fs;
use uuid::Uuid;

/// Options for file copy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
	pub overwrite: bool,
	pub verify_checksum: bool,
	pub preserve_timestamps: bool,
}

impl Default for CopyOptions {
	fn default() -> Self {
		Self {
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
		}
	}
}

/// File copy job
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct FileCopyJob {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	#[serde(default)]
	pub options: CopyOptions,

	// Internal state for resumption
	#[serde(skip)]
	completed_indices: Vec<usize>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl Job for FileCopyJob {
	const NAME: &'static str = "file_copy";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Copy files to a destination");
}

#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
	type Output = FileCopyOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		println!("🔍 FILECOPY_DEBUG: FileCopyJob::run called with {} sources", self.sources.paths.len());
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

		// Calculate total size for progress
		let estimated_total_bytes = self.calculate_total_size(&ctx).await?;

		// Process each device group
		for (device_id, device_paths) in by_device {
			ctx.check_interrupt().await?;

			if device_id == self.destination.device_id {
				// Same device - efficient local copy
				self.process_same_device_copies(
					device_paths.iter().collect(),
					&ctx,
					&mut copied_count,
					&mut total_bytes,
					&mut failed_copies,
					total_files,
					estimated_total_bytes,
				)
				.await?;
			} else {
				// Cross-device copy
				println!("🔍 FILECOPY_DEBUG: Processing cross-device copies for device {}", device_id);
				self.process_cross_device_copies(
					device_paths.iter().collect(),
					&ctx,
					&mut copied_count,
					&mut total_bytes,
					&mut failed_copies,
					total_files,
					estimated_total_bytes,
				)
				.await?;
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

	/// Process copies within the same device
	async fn process_same_device_copies(
		&mut self,
		paths: Vec<&SdPath>,
		ctx: &JobContext<'_>,
		copied_count: &mut usize,
		total_bytes: &mut u64,
		failed_copies: &mut Vec<CopyError>,
		total_files: usize,
		estimated_total_bytes: u64,
	) -> JobResult<()> {
		for source in paths {
			ctx.check_interrupt().await?;

			if let Some(local_source) = source.as_local_path() {
				let dest_path = self
					.destination
					.path
					.join(local_source.file_name().unwrap_or_default());

				ctx.progress(Progress::structured(CopyProgress {
					current_file: local_source.display().to_string(),
					files_copied: *copied_count,
					total_files,
					bytes_copied: *total_bytes,
					total_bytes: estimated_total_bytes,
					current_operation: "Copying".to_string(),
					estimated_remaining: None,
				}));

				match self.copy_local_file(local_source, &dest_path).await {
					Ok(bytes) => {
						*copied_count += 1;
						*total_bytes += bytes;
						ctx.log(format!(
							"Copied: {} -> {}",
							local_source.display(),
							dest_path.display()
						));
					}
					Err(e) => {
						failed_copies.push(CopyError {
							source: local_source.to_path_buf(),
							destination: dest_path,
							error: e.to_string(),
						});
						ctx.add_non_critical_error(format!(
							"Failed to copy {}: {}",
							local_source.display(),
							e
						));
					}
				}

				// Checkpoint every 20 files
				if *copied_count % 20 == 0 {
					ctx.checkpoint().await?;
				}
			}
		}

		Ok(())
	}

	/// Process cross-device copies
	async fn process_cross_device_copies(
		&mut self,
		paths: Vec<&SdPath>,
		ctx: &JobContext<'_>,
		copied_count: &mut usize,
		total_bytes: &mut u64,
		failed_copies: &mut Vec<CopyError>,
		total_files: usize,
		estimated_total_bytes: u64,
	) -> JobResult<()> {
		println!("🔍 FILECOPY_DEBUG: process_cross_device_copies called with {} paths", paths.len());
		for source in paths {
			ctx.check_interrupt().await?;

			ctx.progress(Progress::structured(CopyProgress {
				current_file: source.display(),
				files_copied: *copied_count,
				total_files,
				bytes_copied: *total_bytes,
				total_bytes: estimated_total_bytes,
				current_operation: "Cross-device transfer".to_string(),
				estimated_remaining: None,
			}));

			// Initiate cross-device file transfer using networking stack
			match self.transfer_file_to_device(source, ctx).await {
				Ok(bytes_transferred) => {
					*copied_count += 1;
					*total_bytes += bytes_transferred;
					ctx.log(format!(
						"Cross-device copy completed: {} -> device:{}",
						source.display(),
						self.destination.device_id
					));
				}
				Err(e) => {
					failed_copies.push(CopyError {
						source: source.path.clone(),
						destination: self.destination.path.clone(),
						error: format!("Cross-device transfer failed: {}", e),
					});
					ctx.add_non_critical_error(format!(
						"Failed to transfer {} to device {}: {}",
						source.display(),
						self.destination.device_id,
						e
					));
				}
			}

			// Checkpoint progress every few files
			if *copied_count % 5 == 0 {
				ctx.checkpoint().await?;
			}
		}

		Ok(())
	}

	/// Transfer a single file to a remote device using the networking stack
	async fn transfer_file_to_device(
		&self,
		source: &SdPath,
		ctx: &JobContext<'_>,
	) -> Result<u64, String> {
		println!("🔍 FILECOPY_DEBUG: transfer_file_to_device called for source: {}", source.display());
		// Get networking service
		let networking = ctx.networking_service()
			.ok_or_else(|| "Networking service not available".to_string())?;

		// Get local path
		let local_path = source.as_local_path()
			.ok_or_else(|| "Source must be local path".to_string())?;

		// Read file metadata
		let metadata = tokio::fs::metadata(local_path).await
			.map_err(|e| format!("Failed to read file metadata: {}", e))?;

		let file_size = metadata.len();

		ctx.log(format!(
			"🚀 Initiating cross-device transfer: {} ({} bytes) -> device:{}",
			local_path.display(),
			file_size,
			self.destination.device_id
		));

		// Create file metadata for transfer
		let file_metadata = crate::infrastructure::networking::protocols::FileMetadata {
			name: local_path.file_name()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string(),
			size: file_size,
			modified: metadata.modified().ok(),
			is_directory: metadata.is_dir(),
			checksum: Some(self.calculate_file_checksum(local_path).await?),
			mime_type: None,
		};

		// Get file transfer protocol handler
		let networking_guard = networking.read().await;
		let protocol_registry = networking_guard.protocol_registry();
		let registry_guard = protocol_registry.read().await;
		
		let file_transfer_handler = registry_guard.get_handler("file_transfer")
			.ok_or_else(|| "File transfer protocol not registered".to_string())?;
		
		let file_transfer_protocol = file_transfer_handler.as_any()
			.downcast_ref::<crate::infrastructure::networking::protocols::FileTransferProtocolHandler>()
			.ok_or_else(|| "Invalid file transfer protocol handler".to_string())?;

		// Initiate transfer locally (create session)
		let transfer_id = file_transfer_protocol.initiate_transfer(
			self.destination.device_id,
			local_path.to_path_buf(),
			crate::infrastructure::networking::protocols::TransferMode::TrustedCopy,
		).await.map_err(|e| format!("Failed to initiate transfer: {}", e))?;

		ctx.log(format!("📋 Transfer initiated with ID: {}", transfer_id));

		// Send transfer request to remote device
		let chunk_size = 64 * 1024u32;
		let total_chunks = ((file_size + chunk_size as u64 - 1) / chunk_size as u64) as u32;
		
		let transfer_request = crate::infrastructure::networking::protocols::file_transfer::FileTransferMessage::TransferRequest {
			transfer_id,
			file_metadata: file_metadata.clone(),
			transfer_mode: crate::infrastructure::networking::protocols::TransferMode::TrustedCopy,
			chunk_size,
			total_chunks,
			checksum: Some(file_metadata.checksum.unwrap_or([0u8; 32])),
			destination_path: self.destination.path.to_string_lossy().to_string(),
		};

		let request_data = rmp_serde::to_vec(&transfer_request)
			.map_err(|e| format!("Failed to serialize transfer request: {}", e))?;

		// Send transfer request over network
		networking_guard.send_message(
			self.destination.device_id,
			"file_transfer",
			request_data,
		).await.map_err(|e| format!("Failed to send transfer request: {}", e))?;

		ctx.log(format!("📤 Transfer request sent to device {}", self.destination.device_id));

		// For trusted devices, assume acceptance and start streaming immediately
		// TODO: Wait for actual acceptance response in production
		drop(networking_guard);
		drop(registry_guard);

		// Stream file data
		self.stream_file_data(
			local_path,
			transfer_id,
			file_transfer_protocol,
			file_size,
			ctx,
		).await?;

		ctx.log(format!("✅ Cross-device transfer completed: {} bytes", file_size));
		Ok(file_size)
	}

	/// Stream file data in chunks to the remote device
	async fn stream_file_data(
		&self,
		file_path: &std::path::Path,
		transfer_id: uuid::Uuid,
		file_transfer_protocol: &crate::infrastructure::networking::protocols::FileTransferProtocolHandler,
		total_size: u64,
		ctx: &JobContext<'_>,
	) -> Result<(), String> {
		use tokio::io::AsyncReadExt;
		use blake3::Hasher;

		// Get networking service for sending chunks
		let networking = ctx.networking_service()
			.ok_or_else(|| "Networking service not available".to_string())?;

		let mut file = tokio::fs::File::open(file_path).await
			.map_err(|e| format!("Failed to open file: {}", e))?;

		let chunk_size = 64 * 1024; // 64KB chunks
		let total_chunks = (total_size + chunk_size - 1) / chunk_size;
		let mut buffer = vec![0u8; chunk_size as usize];
		let mut chunk_index = 0u32;
		let mut bytes_transferred = 0u64;

		ctx.log(format!("📦 Starting to stream {} chunks to device {}", total_chunks, self.destination.device_id));

		loop {
			ctx.check_interrupt().await
				.map_err(|e| format!("Transfer cancelled: {}", e))?;

			let bytes_read = file.read(&mut buffer).await
				.map_err(|e| format!("Failed to read file: {}", e))?;

			if bytes_read == 0 {
				break; // End of file
			}

			// Calculate chunk checksum
			let chunk_data = &buffer[..bytes_read];
			let chunk_checksum = blake3::hash(chunk_data);

			// Update progress
			bytes_transferred += bytes_read as u64;
			ctx.progress(Progress::structured(CopyProgress {
				current_file: format!("Transferring {}", file_path.display()),
				files_copied: 0, // Will be updated by caller
				total_files: 1,
				bytes_copied: bytes_transferred,
				total_bytes: total_size,
				current_operation: format!("Chunk {}/{}", chunk_index + 1, total_chunks),
				estimated_remaining: None,
			}));

			// Create file chunk message
			let chunk_message = crate::infrastructure::networking::protocols::file_transfer::FileTransferMessage::FileChunk {
				transfer_id,
				chunk_index,
				data: chunk_data.to_vec(),
				chunk_checksum: *chunk_checksum.as_bytes(),
			};

			// Serialize and send chunk over network
			let chunk_data = rmp_serde::to_vec(&chunk_message)
				.map_err(|e| format!("Failed to serialize chunk: {}", e))?;

			let networking_guard = networking.read().await;
			networking_guard.send_message(
				self.destination.device_id,
				"file_transfer",
				chunk_data,
			).await.map_err(|e| format!("Failed to send chunk over network: {}", e))?;

			// Record chunk in local protocol handler for tracking
			file_transfer_protocol.record_chunk_received(
				&transfer_id,
				chunk_index,
				bytes_read as u64,
			).map_err(|e| format!("Failed to record chunk: {}", e))?;

			chunk_index += 1;

			ctx.log(format!("📤 Sent chunk {}/{} ({} bytes)", chunk_index, total_chunks, bytes_read));

			// Small delay to prevent overwhelming the network
			tokio::time::sleep(std::time::Duration::from_millis(10)).await;
		}

		// Send transfer completion message
		let final_checksum = self.calculate_file_checksum(file_path).await?;
		let completion_message = crate::infrastructure::networking::protocols::file_transfer::FileTransferMessage::TransferComplete {
			transfer_id,
			final_checksum,
			total_bytes: bytes_transferred,
		};

		let completion_data = rmp_serde::to_vec(&completion_message)
			.map_err(|e| format!("Failed to serialize completion: {}", e))?;

		let networking_guard = networking.read().await;
		networking_guard.send_message(
			self.destination.device_id,
			"file_transfer",
			completion_data,
		).await.map_err(|e| format!("Failed to send completion over network: {}", e))?;

		// Mark transfer as completed locally
		file_transfer_protocol.update_session_state(
			&transfer_id,
			crate::infrastructure::networking::protocols::file_transfer::TransferState::Completed,
		).map_err(|e| format!("Failed to complete transfer: {}", e))?;

		ctx.log(format!("✅ File streaming completed: {} chunks, {} bytes sent to device {}", 
			chunk_index, bytes_transferred, self.destination.device_id));
		Ok(())
	}

	/// Calculate file checksum for integrity verification
	async fn calculate_file_checksum(&self, path: &std::path::Path) -> Result<[u8; 32], String> {
		use tokio::io::AsyncReadExt;

		let mut file = tokio::fs::File::open(path).await
			.map_err(|e| format!("Failed to open file for checksum: {}", e))?;

		let mut hasher = blake3::Hasher::new();
		let mut buffer = [0u8; 8192];

		loop {
			let bytes_read = file.read(&mut buffer).await
				.map_err(|e| format!("Failed to read file for checksum: {}", e))?;

			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}

		Ok(hasher.finalize().into())
	}

	/// Copy a local file or directory
	async fn copy_local_file(
		&self,
		source: &std::path::Path,
		destination: &std::path::Path,
	) -> Result<u64, std::io::Error> {
		// Create destination directory if needed
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Check if destination exists
		if !self.options.overwrite && fs::try_exists(destination).await? {
			return Err(std::io::Error::new(
				std::io::ErrorKind::AlreadyExists,
				"Destination already exists and overwrite is disabled",
			));
		}

		let metadata = fs::metadata(source).await?;

		if metadata.is_file() {
			let bytes = fs::copy(source, destination).await?;

			// Preserve timestamps if requested
			if self.options.preserve_timestamps {
				if let (Ok(accessed), Ok(modified)) = (metadata.accessed(), metadata.modified()) {
					// Note: Setting timestamps requires platform-specific code
					// This is a simplified version
				}
			}

			Ok(bytes)
		} else if metadata.is_dir() {
			self.copy_directory_recursive(source, destination).await
		} else {
			Ok(0)
		}
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

/// Output from file copy job
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyOutput {
	pub copied_count: usize,
	pub failed_count: usize,
	pub total_bytes: u64,
	pub duration: Duration,
	pub failed_copies: Vec<CopyError>,
}

impl From<FileCopyOutput> for JobOutput {
	fn from(output: FileCopyOutput) -> Self {
		JobOutput::FileCopy {
			copied_count: output.copied_count,
			total_bytes: output.total_bytes,
		}
	}
}

// Job registration is now handled automatically by the derive macro
