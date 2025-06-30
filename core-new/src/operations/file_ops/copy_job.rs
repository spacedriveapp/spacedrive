//! File copy job implementation (with move support)

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
	const DESCRIPTION: Option<&'static str> = Some("Copy or move files to a destination");
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
				// Same device - volume-aware local copy
				ctx.log(format!(
					"Processing {} files on same device with volume-aware routing",
					device_paths.len()
				));
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
				// Cross-device copy (uses networking)
				println!("🔍 FILECOPY_DEBUG: Processing cross-device copies for device {}", device_id);
				ctx.log(format!(
					"Processing {} files for cross-device transfer to device {}",
					device_paths.len(),
					device_id
				));
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

	/// Process copies within the same device (with volume-aware routing)
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
				let dest_path = match self.options.move_mode {
					Some(MoveMode::Rename) => self.destination.path.clone(),
					_ => self.destination.path.join(local_source.file_name().unwrap_or_default()),
				};

				// Check if this is truly same volume or cross-volume on same device
				let is_same_volume = if let Some(volume_manager) = ctx.volume_manager() {
					volume_manager.same_volume(local_source, &dest_path).await
				} else {
					// Fallback: assume same volume if no volume manager
					true
				};

				let operation_name = if self.options.delete_after_copy {
					"Moving"
				} else {
					"Copying"
				};

				let detailed_operation = if is_same_volume {
					if self.options.delete_after_copy {
						"Atomic move"
					} else {
						"Same-volume copy"
					}
				} else {
					if self.options.delete_after_copy {
						"Cross-volume move"
					} else {
						"Cross-volume streaming copy"
					}
				};

				ctx.progress(Progress::structured(CopyProgress {
					current_file: local_source.display().to_string(),
					files_copied: *copied_count,
					total_files,
					bytes_copied: *total_bytes,
					total_bytes: estimated_total_bytes,
					current_operation: detailed_operation.to_string(),
					estimated_remaining: None,
				}));

				// Route to appropriate method based on volume detection
				let result = if is_same_volume && self.options.delete_after_copy {
					// Same volume move: use atomic rename
					self.move_local_file(local_source, &dest_path).await
				} else if is_same_volume {
					// Same volume copy: use regular copy
					self.copy_local_file(local_source, &dest_path).await
				} else {
					// Cross-volume: use streaming copy with progress
					let (source_vol, dest_vol) = if let Some(volume_manager) = ctx.volume_manager() {
						let source_vol = volume_manager.volume_for_path(local_source).await;
						let dest_vol = volume_manager.volume_for_path(&dest_path).await;
						(source_vol, dest_vol)
					} else {
						(None, None)
					};
					
					let volume_info = match (&source_vol, &dest_vol) {
						(Some(s), Some(d)) => Some((s, d)),
						_ => None,
					};

					let copy_result = self.copy_file_streaming(local_source, &dest_path, volume_info, ctx).await;
					
					// If this is a move operation, delete source after successful copy
					if copy_result.is_ok() && self.options.delete_after_copy {
						if let Err(e) = self.delete_source_file(local_source).await {
							return Err(JobError::execution(format!(
								"Copy succeeded but failed to delete source: {}", e
							)));
						}
					}
					
					copy_result
				};

				match result {
					Ok(bytes) => {
						*copied_count += 1;
						*total_bytes += bytes;
						ctx.log(format!(
							"{} ({}): {} -> {}",
							operation_name,
							detailed_operation.to_lowercase(),
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
							"Failed to {} {}: {}",
							operation_name.to_lowercase(),
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

			let operation_name = if self.options.delete_after_copy {
				"Cross-device move"
			} else {
				"Cross-device transfer"
			};

			ctx.progress(Progress::structured(CopyProgress {
				current_file: source.display(),
				files_copied: *copied_count,
				total_files,
				bytes_copied: *total_bytes,
				total_bytes: estimated_total_bytes,
				current_operation: operation_name.to_string(),
				estimated_remaining: None,
			}));

			// For cross-device operations, always use copy-then-delete for moves
			match self.transfer_file_to_device(source, ctx).await {
				Ok(bytes_transferred) => {
					// If this is a move operation, delete the source after successful transfer
					if self.options.delete_after_copy {
						if let Some(local_source) = source.as_local_path() {
							if let Err(e) = self.delete_source_file(local_source).await {
								failed_copies.push(CopyError {
									source: source.path.clone(),
									destination: self.destination.path.clone(),
									error: format!("Copy succeeded but failed to delete source: {}", e),
								});
								ctx.add_non_critical_error(format!(
									"Failed to delete source after transfer {}: {}",
									source.display(),
									e
								));
								return Ok(()); // Don't count as success if delete failed
							}
						}
					}

					*copied_count += 1;
					*total_bytes += bytes_transferred;
					ctx.log(format!(
						"{} completed: {} -> device:{}",
						operation_name,
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
		let file_metadata = crate::services::networking::protocols::FileMetadata {
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
		let networking_guard = &*networking;
		let protocol_registry = networking_guard.protocol_registry();
		let registry_guard = protocol_registry.read().await;

		let file_transfer_handler = registry_guard.get_handler("file_transfer")
			.ok_or_else(|| "File transfer protocol not registered".to_string())?;

		let file_transfer_protocol = file_transfer_handler.as_any()
			.downcast_ref::<crate::services::networking::protocols::FileTransferProtocolHandler>()
			.ok_or_else(|| "Invalid file transfer protocol handler".to_string())?;

		// Initiate transfer locally (create session)
		let transfer_id = file_transfer_protocol.initiate_transfer(
			self.destination.device_id,
			local_path.to_path_buf(),
			crate::services::networking::protocols::TransferMode::TrustedCopy,
		).await.map_err(|e| format!("Failed to initiate transfer: {}", e))?;

		ctx.log(format!("📋 Transfer initiated with ID: {}", transfer_id));

		// Send transfer request to remote device
		let chunk_size = 64 * 1024u32;
		let total_chunks = ((file_size + chunk_size as u64 - 1) / chunk_size as u64) as u32;

		let transfer_request = crate::services::networking::protocols::file_transfer::FileTransferMessage::TransferRequest {
			transfer_id,
			file_metadata: file_metadata.clone(),
			transfer_mode: crate::services::networking::protocols::TransferMode::TrustedCopy,
			chunk_size,
			total_chunks,
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
		file_transfer_protocol: &crate::services::networking::protocols::FileTransferProtocolHandler,
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

			// Get session keys for encryption
			let session_keys = file_transfer_protocol.get_session_keys_for_device(self.destination.device_id).await
				.map_err(|e| format!("Failed to get session keys: {}", e))?;

			// Encrypt chunk using file transfer protocol
			let (encrypted_data, nonce) = file_transfer_protocol.encrypt_chunk(
				&session_keys.send_key,
				&transfer_id,
				chunk_index,
				chunk_data,
			).map_err(|e| format!("Failed to encrypt chunk: {}", e))?;

			// Create encrypted file chunk message
			let chunk_message = crate::services::networking::protocols::file_transfer::FileTransferMessage::FileChunk {
				transfer_id,
				chunk_index,
				data: encrypted_data,
				nonce,
				chunk_checksum: *chunk_checksum.as_bytes(), // Checksum of original unencrypted data
			};

			// Serialize and send chunk over network
			let chunk_data = rmp_serde::to_vec(&chunk_message)
				.map_err(|e| format!("Failed to serialize chunk: {}", e))?;

			let networking_guard = &*networking;
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
		let completion_message = crate::services::networking::protocols::file_transfer::FileTransferMessage::TransferComplete {
			transfer_id,
			final_checksum,
			total_bytes: bytes_transferred,
		};

		let completion_data = rmp_serde::to_vec(&completion_message)
			.map_err(|e| format!("Failed to serialize completion: {}", e))?;

		let networking_guard = &*networking;
		networking_guard.send_message(
			self.destination.device_id,
			"file_transfer",
			completion_data,
		).await.map_err(|e| format!("Failed to send completion over network: {}", e))?;

		// Mark transfer as completed locally
		file_transfer_protocol.update_session_state(
			&transfer_id,
			crate::services::networking::protocols::file_transfer::TransferState::Completed,
		).map_err(|e| format!("Failed to complete transfer: {}", e))?;

		ctx.log(format!("✅ File streaming completed: {} chunks, {} bytes sent to device {}",
			chunk_index, bytes_transferred, self.destination.device_id));
		Ok(())
	}

	/// Calculate file checksum for integrity verification
	async fn calculate_file_checksum(&self, path: &std::path::Path) -> Result<String, String> {
		crate::domain::content_identity::ContentHashGenerator::generate_content_hash(path)
			.await
			.map_err(|e| format!("Failed to generate content hash: {}", e))
	}

	/// Move a file on the same device using atomic rename
	async fn move_local_file(
		&self,
		source: &std::path::Path,
		destination: &std::path::Path,
	) -> Result<u64, std::io::Error> {
		// Get file size before moving
		let metadata = fs::metadata(source).await?;
		let size = if metadata.is_file() {
			metadata.len()
		} else {
			self.get_path_size(source).await?
		};

		// Check if destination exists
		if !self.options.overwrite && fs::try_exists(destination).await? {
			return Err(std::io::Error::new(
				std::io::ErrorKind::AlreadyExists,
				"Destination already exists and overwrite is disabled",
			));
		}

		// Create destination directory if needed
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Use atomic rename for same-device moves
		fs::rename(source, destination).await?;

		Ok(size)
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

	/// Delete source file after successful cross-device copy
	async fn delete_source_file(&self, source: &std::path::Path) -> Result<(), std::io::Error> {
		let metadata = fs::metadata(source).await?;

		if metadata.is_file() {
			fs::remove_file(source).await
		} else if metadata.is_dir() {
			fs::remove_dir_all(source).await
		} else {
			Ok(())
		}
	}

	/// Copy file with streaming and progress tracking for cross-volume operations
	async fn copy_file_streaming(
		&self,
		source: &std::path::Path,
		destination: &std::path::Path,
		volume_info: Option<(&crate::volume::Volume, &crate::volume::Volume)>,
		ctx: &JobContext<'_>,
	) -> Result<u64, std::io::Error> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
		if metadata.is_dir() {
			return self.copy_directory_streaming(source, destination, volume_info, ctx).await;
		}

		let file_size = metadata.len();
		let mut source_file = fs::File::open(source).await?;
		let mut dest_file = fs::File::create(destination).await?;

		// Determine optimal chunk size based on volume characteristics
		let chunk_size = if let Some((source_vol, dest_vol)) = volume_info {
			// Use the minimum optimal chunk size between source and destination volumes
			source_vol.optimal_chunk_size().min(dest_vol.optimal_chunk_size())
		} else {
			64 * 1024 // Default 64KB chunks
		};

		let mut buffer = vec![0u8; chunk_size];
		let mut total_copied = 0u64;
		let mut last_progress_update = std::time::Instant::now();

		loop {
			// Check for cancellation
			if let Err(_) = ctx.check_interrupt().await {
				// Clean up partial file on cancellation
				let _ = fs::remove_file(destination).await;
				return Err(std::io::Error::new(
					std::io::ErrorKind::Interrupted,
					"Operation cancelled"
				));
			}

			let bytes_read = source_file.read(&mut buffer).await?;
			if bytes_read == 0 {
				break; // EOF
			}

			dest_file.write_all(&buffer[..bytes_read]).await?;
			total_copied += bytes_read as u64;

			// Update progress every 100ms to avoid overwhelming the UI
			if last_progress_update.elapsed() >= std::time::Duration::from_millis(100) {
				ctx.progress(Progress::structured(CopyProgress {
					current_file: format!("Streaming {}", source.display()),
					files_copied: 0, // Will be updated by caller
					total_files: 1,
					bytes_copied: total_copied,
					total_bytes: file_size,
					current_operation: "Cross-volume streaming".to_string(),
					estimated_remaining: Some({
						let elapsed = last_progress_update.elapsed();
						let rate = total_copied as f64 / elapsed.as_secs_f64();
						let remaining_bytes = file_size.saturating_sub(total_copied);
						if rate > 0.0 {
							std::time::Duration::from_secs_f64(remaining_bytes as f64 / rate)
						} else {
							std::time::Duration::from_secs(0)
						}
					}),
				}));
				last_progress_update = std::time::Instant::now();
			}
		}

		// Ensure all data is written to disk
		dest_file.flush().await?;
		dest_file.sync_all().await?;

		// Preserve timestamps if requested
		if self.options.preserve_timestamps {
			if let (Ok(accessed), Ok(modified)) = (metadata.accessed(), metadata.modified()) {
				// Note: Setting timestamps requires platform-specific code
				// This is a simplified version that would need proper implementation
			}
		}

		Ok(total_copied)
	}

	/// Copy directory with streaming for cross-volume operations
	fn copy_directory_streaming<'a>(
		&'a self,
		source: &'a std::path::Path,
		destination: &'a std::path::Path,
		volume_info: Option<(&'a crate::volume::Volume, &'a crate::volume::Volume)>,
		ctx: &'a JobContext<'_>,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, std::io::Error>> + Send + 'a>> {
		Box::pin(async move {
		fs::create_dir_all(destination).await?;
		let mut total_size = 0u64;
		let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];

		while let Some((src_path, dest_path)) = stack.pop() {
			// Check for cancellation
			if let Err(_) = ctx.check_interrupt().await {
				return Err(std::io::Error::new(
					std::io::ErrorKind::Interrupted,
					"Operation cancelled"
				));
			}

			if src_path.is_file() {
				total_size += self.copy_file_streaming(&src_path, &dest_path, volume_info, ctx).await?;
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
		})
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

	/// Get volume-optimized copy method for the given paths
	async fn get_optimized_copy_method(
		&self,
		source_path: &std::path::Path,
		dest_path: &std::path::Path,
		volume_manager: Option<&crate::volume::VolumeManager>,
	) -> CopyMethod {
		if let Some(vm) = volume_manager {
			// Check if paths are on same volume
			if vm.same_volume(source_path, dest_path).await {
				// Same volume - check for filesystem optimizations
				if let Some(source_vol) = vm.volume_for_path(source_path).await {
					if source_vol.supports_fast_copy() {
						return CopyMethod::FastCopy { supports_reflink: source_vol.file_system.supports_reflink() };
					}
				}
				return CopyMethod::SameVolume;
			} else {
				// Cross-volume - get optimization hints
				let source_vol = vm.volume_for_path(source_path).await;
				let dest_vol = vm.volume_for_path(dest_path).await;
				
				if let (Some(s_vol), Some(d_vol)) = (source_vol, dest_vol) {
					let estimated_speed = s_vol.estimate_copy_speed(&d_vol);
					return CopyMethod::CrossVolume {
						source_volume: s_vol,
						dest_volume: d_vol,
						estimated_speed_mbps: estimated_speed,
					};
				}
			}
		}
		
		// Fallback when no volume manager available
		CopyMethod::Standard
	}
}

/// Copy method determined by volume analysis
#[derive(Debug, Clone)]
enum CopyMethod {
	/// Standard copy operation (fallback)
	Standard,
	/// Same volume copy (potentially faster)
	SameVolume,
	/// Fast copy with filesystem support (CoW, reflinks)
	FastCopy { supports_reflink: bool },
	/// Cross-volume copy with volume information
	CrossVolume {
		source_volume: crate::volume::Volume,
		dest_volume: crate::volume::Volume,
		estimated_speed_mbps: Option<u64>,
	},
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

// Job registration is now handled automatically by the derive macro
