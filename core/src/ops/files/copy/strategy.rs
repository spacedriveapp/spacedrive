//! Copy strategy implementations for different file operation scenarios
//!
//! This module implements 4 distinct copy strategies, each optimized for specific scenarios.
//! The strategy selection is handled by `CopyStrategyRouter` based on user preferences
//! (`CopyMethod`) and system analysis (device topology, filesystem capabilities).
//!
//! ## Strategy Overview
//!
//! ### User-Facing Methods (3 options in `CopyMethod` enum):
//! - `Auto`: Automatically selects the best strategy based on analysis
//! - `Atomic`: Prefers instant/atomic operations when possible
//! - `Streaming`: Forces streaming with progress tracking and cancellation
//!
//! ### Implementation Strategies (4 strategies in this file):
//!
//! 1. **`LocalMoveStrategy`** - Atomic filesystem rename
//!    - **When**: Moving files on the same volume/filesystem
//!    - **How**: Uses `fs::rename()` syscall - instant metadata update
//!    - **Performance**: Microseconds, regardless of file size
//!    - **Example**: Moving `/home/user/file.txt` → `/home/user/Documents/file.txt`
//!
//! 2. **`FastCopyStrategy`** - Copy-on-Write (CoW) optimized copying
//!    - **When**: Copying on CoW filesystems (APFS, Btrfs, ZFS, ReFS)
//!    - **How**: Uses `std::fs::copy()` which leverages APFS clones, Btrfs reflinks, etc.
//!    - **Performance**: Near-instant for CoW filesystems, falls back to normal copy otherwise
//!    - **Example**: Copying large files on macOS APFS or Linux Btrfs
//!
//! 3. **`LocalStreamCopyStrategy`** - Cross-volume streaming with progress
//!    - **When**: Copying between different local volumes or when user wants progress
//!    - **How**: Chunked streaming with volume-aware buffer sizes, checksum verification
//!    - **Performance**: Depends on storage speeds, provides real-time progress
//!    - **Example**: Copying from internal SSD to external USB drive
//!
//! 4. **`RemoteTransferStrategy`** - Encrypted network transfer
//!    - **When**: Copying to another device (automatically detected by different device IDs)
//!    - **How**: Encrypted chunked streaming over network protocols
//!    - **Performance**: Network-dependent, fault-tolerant with retry logic
//!    - **Example**: Syncing files between laptop and desktop over WiFi
//!
//! ## Strategy Selection Logic
//!
//! ```rust
//! match (copy_method, cross_device, same_storage, is_move) {
//!     (_, true, _, _) => RemoteTransferStrategy,           // Cross-device always uses network
//!     (Atomic, _, _, true) => LocalMoveStrategy,           // Atomic move preference
//!     (Atomic, _, _, false) => FastCopyStrategy,           // Atomic copy preference
//!     (Streaming, _, _, _) => LocalStreamCopyStrategy,     // Streaming preference
//!     (Auto, _, true, true) => LocalMoveStrategy,          // Auto: same storage move
//!     (Auto, _, true, false) => FastCopyStrategy,          // Auto: same storage copy
//!     (Auto, _, false, _) => LocalStreamCopyStrategy,      // Auto: cross storage
//! }
//! ```

use crate::{
	domain::addressing::SdPath, infra::job::prelude::*, ops::files::copy::job::CopyPhase,
	volume::VolumeManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, debug, error};

/// Progress callback for strategy implementations to report granular progress
/// Parameters: bytes_copied_for_current_file, total_bytes_for_current_file
pub type ProgressCallback<'a> = Box<dyn Fn(u64, u64) + Send + Sync + 'a>;

/// Defines a method for performing a file copy operation
#[async_trait]
pub trait CopyStrategy: Send + Sync {
	/// Executes the copy strategy for a single source path
	async fn execute<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64>;
}

/// Strategy for an atomic move on the same volume
pub struct LocalMoveStrategy;

#[async_trait]
impl CopyStrategy for LocalMoveStrategy {
	async fn execute<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		let source_path = source
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Source path is not local"))?;
		let dest_path = destination
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Destination path is not local"))?;

		// Get file size before moving
		let metadata = fs::metadata(source_path).await?;
		let size = if metadata.is_file() {
			metadata.len()
		} else {
			get_path_size(source_path).await?
		};

		// Report progress at current offset before starting
		if let Some(callback) = progress_callback {
			callback(0, size);
		}

		// Create destination directory if needed
		if let Some(parent) = dest_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Use atomic rename for same-volume moves
		fs::rename(source_path, dest_path).await?;

		// Report progress at 100% after completion
		if let Some(callback) = progress_callback {
			callback(size, size);
		}

		ctx.log(format!(
			"Atomic move: {} -> {}",
			source_path.display(),
			dest_path.display()
		));

		Ok(size)
	}
}

/// Strategy for streaming a copy between different local volumes
pub struct LocalStreamCopyStrategy;

#[async_trait]
impl CopyStrategy for LocalStreamCopyStrategy {
	async fn execute<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		let source_path = source
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Source path is not local"))?;
		let dest_path = destination
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Destination path is not local"))?;

		// Get volume information for optimization
		let (source_vol, dest_vol) = if let Some(volume_manager) = ctx.volume_manager() {
			let source_vol = volume_manager.volume_for_path(source_path).await;
			let dest_vol = volume_manager.volume_for_path(dest_path).await;
			(source_vol, dest_vol)
		} else {
			(None, None)
		};

		let volume_info = match (&source_vol, &dest_vol) {
			(Some(s), Some(d)) => Some((s, d)),
			_ => None,
		};

		let bytes_copied = copy_file_streaming(
			source_path,
			dest_path,
			volume_info,
			ctx,
			verify_checksum,
			progress_callback,
		)
		.await?;

		ctx.log(format!(
			"Streaming copy: {} -> {} ({} bytes)",
			source_path.display(),
			dest_path.display(),
			bytes_copied
		));

		Ok(bytes_copied)
	}
}

/// Strategy for fast local copy operations on CoW filesystems
/// Uses std::fs::copy which automatically handles APFS clones, Btrfs reflinks, ZFS clones, and ReFS block clones
pub struct FastCopyStrategy;

#[async_trait]
impl CopyStrategy for FastCopyStrategy {
	async fn execute<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		_progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		let source_path = source
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Source path is not local"))?;
		let dest_path = destination
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Destination path is not local"))?;

		// Create destination directory if needed
		if let Some(parent) = dest_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Use std::fs::copy which automatically handles filesystem optimizations
		let bytes_copied = tokio::task::spawn_blocking({
			let source_path = source_path.to_path_buf();
			let dest_path = dest_path.to_path_buf();
			move || -> Result<u64, std::io::Error> { std::fs::copy(&source_path, &dest_path) }
		})
		.await??;

		// Verify checksum if requested
		if verify_checksum {
			let source_checksum = calculate_file_checksum(source_path).await?;
			let dest_checksum = calculate_file_checksum(dest_path).await?;
			if source_checksum != dest_checksum {
				return Err(anyhow::anyhow!("Checksum verification failed"));
			}
		}

		ctx.log(format!(
			"Fast copy: {} -> {} ({} bytes)",
			source_path.display(),
			dest_path.display(),
			bytes_copied
		));

		Ok(bytes_copied)
	}
}

/// Strategy for transferring a file to another device
pub struct RemoteTransferStrategy;

#[async_trait]
impl CopyStrategy for RemoteTransferStrategy {
	async fn execute<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		debug!("RemoteTransferStrategy: {} -> device:{}",
			source,
			destination.device_id().unwrap_or_default());

		// Get networking service
		let networking = ctx.networking_service()
			.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

		// Get local path
		let local_path = source.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Source must be local path"))?;

		// Read file metadata
		let metadata = tokio::fs::metadata(local_path).await?;
		let file_size = metadata.len();

		let checksum = calculate_file_checksum(local_path).await
			.map(Some)
			.map_err(|e| anyhow::anyhow!("Failed to calculate checksum: {}", e))?;

		info!("Initiating cross-device transfer: {} ({} bytes) -> device:{}",
			local_path.display(),
			file_size,
			destination.device_id().unwrap_or_default());

		ctx.log(format!(
			"Initiating cross-device transfer: {} ({} bytes) -> device:{}",
			local_path.display(),
			file_size,
			destination.device_id().unwrap_or_default()
		));

		// Create file metadata for transfer
		let file_metadata = crate::service::network::protocol::FileMetadata {
			name: local_path
				.file_name()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string(),
			size: file_size,
			modified: metadata.modified().ok(),
			is_directory: metadata.is_dir(),
			checksum,
			mime_type: None,
		};

		// Get file transfer protocol handler
		let networking_guard = &*networking;
		let protocol_registry = networking_guard.protocol_registry();
		let registry_guard = protocol_registry.read().await;

		let file_transfer_handler = registry_guard
			.get_handler("file_transfer")
			.ok_or_else(|| anyhow::anyhow!("File transfer protocol not registered"))?;

		let file_transfer_protocol = file_transfer_handler
			.as_any()
			.downcast_ref::<crate::service::network::protocol::FileTransferProtocolHandler>()
			.ok_or_else(|| anyhow::anyhow!("Invalid file transfer protocol handler"))?;

		// Initiate transfer
		let transfer_id = file_transfer_protocol
			.initiate_transfer(
				destination.device_id().unwrap_or_default(),
				local_path.to_path_buf(),
				crate::service::network::protocol::TransferMode::TrustedCopy,
			)
			.await?;

		debug!("Transfer initiated with ID: {}", transfer_id);
		ctx.log(format!("Transfer initiated with ID: {}", transfer_id));

		let result = stream_file_data(
			local_path,
			transfer_id,
			file_transfer_protocol,
			file_size,
			destination.device_id().unwrap_or_default(),
			destination.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
			file_metadata,
			ctx,
			progress_callback,
		)
		.await;

		match result {
			Ok(()) => {
				info!("Cross-device transfer completed: {} bytes", file_size);
				ctx.log(format!(
					"Cross-device transfer completed successfully: {} bytes",
					file_size
				));
				Ok(file_size)
			}
			Err(e) => {
				error!("Cross-device transfer failed: {}", e);
				ctx.log(format!("Cross-device transfer FAILED: {}", e));
				Err(e)
			}
		}
	}
}

/// Helper function to get size of a path (file or directory)
async fn get_path_size(path: &Path) -> Result<u64, std::io::Error> {
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

/// Copy a single file with streaming and progress tracking
async fn copy_single_file<'a>(
	source: &Path,
	destination: &Path,
	volume_info: Option<(&crate::volume::Volume, &crate::volume::Volume)>,
	ctx: &JobContext<'a>,
	verify_checksum: bool,
	file_size: u64,
	progress_callback: Option<&ProgressCallback<'a>>,
) -> Result<u64, std::io::Error> {
	let result = copy_single_file_with_offset(
		source,
		destination,
		volume_info,
		ctx,
		verify_checksum,
		file_size,
		progress_callback,
		0,
	)
	.await?;

	// For single file copies, send completion signal
	if let Some(callback) = progress_callback {
		callback(result, u64::MAX);
	}

	Ok(result)
}

/// Copy a single file with streaming and progress tracking, with byte offset for cumulative progress
async fn copy_single_file_with_offset<'a>(
	source: &Path,
	destination: &Path,
	volume_info: Option<(&crate::volume::Volume, &crate::volume::Volume)>,
	ctx: &JobContext<'a>,
	verify_checksum: bool,
	file_size: u64,
	progress_callback: Option<&ProgressCallback<'a>>,
	byte_offset: u64,
) -> Result<u64, std::io::Error> {
	// Create destination directory if needed
	if let Some(parent) = destination.parent() {
		fs::create_dir_all(parent).await?;
	}

	let mut source_file = fs::File::open(source).await?;
	let mut dest_file = fs::File::create(destination).await?;

	// Determine optimal chunk size based on volume characteristics
	let chunk_size = if let Some((source_vol, dest_vol)) = volume_info {
		source_vol
			.optimal_chunk_size()
			.min(dest_vol.optimal_chunk_size())
	} else {
		64 * 1024 // Default 64KB chunks
	};

	let mut buffer = vec![0u8; chunk_size];
	let mut total_copied = 0u64;
	let mut last_progress_update = std::time::Instant::now();

	// Initialize checksums if verification is enabled
	let mut source_hasher = if verify_checksum {
		Some(blake3::Hasher::new())
	} else {
		None
	};

	let mut dest_hasher = if verify_checksum {
		Some(blake3::Hasher::new())
	} else {
		None
	};

	loop {
		// Check for cancellation
		if let Err(_) = ctx.check_interrupt().await {
			// Clean up partial file on cancellation
			let _ = fs::remove_file(destination).await;
			return Err(std::io::Error::new(
				std::io::ErrorKind::Interrupted,
				"Operation cancelled",
			));
		}

		let bytes_read = source_file.read(&mut buffer).await?;
		if bytes_read == 0 {
			break; // EOF
		}

		let chunk = &buffer[..bytes_read];
		dest_file.write_all(chunk).await?;
		total_copied += bytes_read as u64;

		// Update checksums if verification is enabled
		if let Some(hasher) = &mut source_hasher {
			hasher.update(chunk);
		}
		if let Some(hasher) = &mut dest_hasher {
			hasher.update(chunk);
		}

		// Update progress every 50ms for smoother updates
		if last_progress_update.elapsed() >= std::time::Duration::from_millis(50) {
			if let Some(callback) = progress_callback {
				// Send bytes copied within current file
				// The aggregator will add this to the bytes_completed_before_current
				callback(total_copied, file_size);

				// Debug log every 100MB
				if total_copied % (100 * 1024 * 1024) < bytes_read as u64 {
					ctx.log(format!(
						"Strategy progress callback: {} / {} bytes",
						total_copied, file_size
					));
				}
			}
			last_progress_update = std::time::Instant::now();

			// Explicitly yield to the scheduler to allow other tasks (like progress reporting) to run
			tokio::task::yield_now().await;
		}
	}

	// Ensure all data is written to disk
	dest_file.flush().await?;
	dest_file.sync_all().await?;

	// Final progress update to ensure we show 100%
	if let Some(callback) = progress_callback {
		callback(total_copied, file_size);
		ctx.log(format!(
			"Strategy final progress: {} / {} bytes (100%)",
			total_copied, file_size
		));
	}

	// Verify checksums if enabled
	if verify_checksum {
		if let (Some(source_hasher), Some(dest_hasher)) = (source_hasher, dest_hasher) {
			let source_hash = source_hasher.finalize();
			let dest_hash = dest_hasher.finalize();

			if source_hash != dest_hash {
				// Clean up corrupted file
				let _ = fs::remove_file(destination).await;
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!(
						"Checksum verification failed: source={}, dest={}",
						source_hash.to_hex(),
						dest_hash.to_hex()
					),
				));
			}

			ctx.log(format!(
				"Checksum verification passed for {}: {}",
				destination.display(),
				source_hash.to_hex()
			));
		}
	}

	// Copy file permissions and timestamps if requested
	let source_metadata = fs::metadata(source).await?;
	let dest_file = fs::File::open(destination).await?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let permissions = std::fs::Permissions::from_mode(source_metadata.permissions().mode());
		dest_file.set_permissions(permissions).await?;
	}

	Ok(total_copied)
}

/// Copy file with streaming and progress tracking for cross-volume operations
async fn copy_file_streaming<'a>(
	source: &Path,
	destination: &Path,
	volume_info: Option<(&crate::volume::Volume, &crate::volume::Volume)>,
	ctx: &JobContext<'a>,
	verify_checksum: bool,
	progress_callback: Option<&ProgressCallback<'a>>,
) -> Result<u64, std::io::Error> {
	// Create destination directory if needed
	if let Some(parent) = destination.parent() {
		fs::create_dir_all(parent).await?;
	}

	let metadata = fs::metadata(source).await?;
	if metadata.is_dir() {
		// For directories, we need to accumulate progress across multiple files
		fs::create_dir_all(destination).await?;
		let mut total_size = 0u64;

		// First, collect all files to copy
		let mut files_to_copy = Vec::new();
		let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];

		while let Some((src_path, dest_path)) = stack.pop() {
			if src_path.is_file() {
				files_to_copy.push((src_path, dest_path));
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

		// Now copy all files, tracking cumulative progress
		let mut cumulative_bytes = 0u64;
		for (src_path, dest_path) in files_to_copy {
			// Check for cancellation
			if let Err(_) = ctx.check_interrupt().await {
				return Err(std::io::Error::new(
					std::io::ErrorKind::Interrupted,
					"Operation cancelled",
				));
			}

			let file_metadata = fs::metadata(&src_path).await?;
			let file_size = file_metadata.len();

			// Copy the file (offset no longer needed as aggregator tracks it)
			let bytes_copied = copy_single_file_with_offset(
				&src_path,
				&dest_path,
				volume_info,
				ctx,
				verify_checksum,
				file_size,
				progress_callback,
				0,
			)
			.await?;
			cumulative_bytes += bytes_copied;
			total_size += bytes_copied;

			// Signal completion of this file, passing its total size
			if let Some(callback) = progress_callback {
				callback(bytes_copied, u64::MAX); // Send file size and MAX signal
			}
		}

		return Ok(total_size);
	}

	let file_size = metadata.len();
	// Use the copy_single_file helper function
	copy_single_file(
		source,
		destination,
		volume_info,
		ctx,
		verify_checksum,
		file_size,
		progress_callback,
	)
	.await
}

/// Calculate file checksum for integrity verification
async fn calculate_file_checksum(path: &Path) -> Result<String> {
	crate::domain::content_identity::ContentHashGenerator::generate_content_hash(path)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to generate content hash: {}", e))
}

/// Stream file data in chunks to the remote device using a persistent connection
async fn stream_file_data<'a>(
	file_path: &Path,
	transfer_id: uuid::Uuid,
	file_transfer_protocol: &crate::service::network::protocol::FileTransferProtocolHandler,
	total_size: u64,
	destination_device_id: uuid::Uuid,
	destination_path: String,
	file_metadata: crate::service::network::protocol::FileMetadata,
	ctx: &JobContext<'a>,
	progress_callback: Option<&ProgressCallback<'a>>,
) -> Result<()> {
	use blake3::Hasher;
	use tokio::io::{AsyncReadExt, AsyncWriteExt};

	debug!("Streaming {} bytes to device {}", total_size, destination_device_id);

	// Get networking service
	let networking = ctx
		.networking_service()
		.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

	let networking_guard = &*networking;

	// Get device registry to lookup node_id
	let device_registry = networking_guard.device_registry();
	let registry = device_registry.read().await;
	let node_id = registry
		.get_node_by_device(destination_device_id)
		.ok_or_else(|| {
			anyhow::anyhow!(
				"Could not find node_id for device {}",
				destination_device_id
			)
		})?;
	drop(registry);

	// Get endpoint for creating connection
	let endpoint = networking_guard.endpoint().ok_or_else(|| {
		anyhow::anyhow!("Networking endpoint not available")
	})?;

	ctx.log(format!(
		"Opening persistent connection to node {} (device {}) for file transfer",
		node_id, destination_device_id
	));

	// Connect to the target device using file_transfer ALPN
	let node_addr = iroh::NodeAddr::new(node_id);
	let connection = endpoint
		.connect(node_addr, b"spacedrive/filetransfer/1")
		.await
		.map_err(|e| anyhow::anyhow!("Failed to connect to device: {}", e))?;

	// Open bidirectional stream for the transfer (so we can receive acknowledgment)
	let (mut send_stream, mut recv_stream) = connection
		.open_bi()
		.await
		.map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

	// First, send the TransferRequest message
	let chunk_size = 64 * 1024u32;
	let total_chunks = ((total_size + chunk_size as u64 - 1) / chunk_size as u64) as u32;

	let transfer_request = crate::service::network::protocol::file_transfer::FileTransferMessage::TransferRequest {
		transfer_id,
		file_metadata,
		transfer_mode: crate::service::network::protocol::TransferMode::TrustedCopy,
		chunk_size,
		total_chunks,
		destination_path,
	};

	let request_data = rmp_serde::to_vec(&transfer_request)?;

	ctx.log(format!(
		"Sending TransferRequest for {} bytes ({} chunks)",
		total_size, total_chunks
	));

	// Send transfer request: type (0) + length + data
	send_stream.write_u8(0).await?;
	send_stream.write_all(&(request_data.len() as u32).to_be_bytes()).await?;
	send_stream.write_all(&request_data).await?;
	send_stream.flush().await?;

	ctx.log("TransferRequest sent, now sending file chunks".to_string());

	// Open file for reading
	let mut file = tokio::fs::File::open(file_path).await?;

	let chunk_size = 64 * 1024u64; // 64KB chunks
	let total_chunks = (total_size + chunk_size - 1) / chunk_size;
	let mut buffer = vec![0u8; chunk_size as usize];
	let mut chunk_index = 0u32;
	let mut bytes_transferred = 0u64;

	ctx.log(format!(
		"Starting to stream {} chunks ({} bytes) to device {}",
		total_chunks, total_size, destination_device_id
	));

	// Send all chunks over the stream
	loop {
		ctx.check_interrupt().await?;

		let bytes_read = file.read(&mut buffer).await?;
		if bytes_read == 0 {
			break; // End of file
		}

		// Calculate chunk checksum (of original data)
		let chunk_data = &buffer[..bytes_read];
		let chunk_checksum = blake3::hash(chunk_data);

		// Get session keys for encryption
		let session_keys = file_transfer_protocol
			.get_session_keys_for_device(destination_device_id)
			.await?;

		// Encrypt chunk using file transfer protocol
		let (encrypted_data, nonce) = file_transfer_protocol.encrypt_chunk(
			&session_keys.send_key,
			&transfer_id,
			chunk_index,
			chunk_data,
		)?;

		// Create encrypted file chunk message
		let chunk_message =
			crate::service::network::protocol::file_transfer::FileTransferMessage::FileChunk {
				transfer_id,
				chunk_index,
				data: encrypted_data,
				nonce,
				chunk_checksum: *chunk_checksum.as_bytes(),
			};

		// Serialize message
		let message_data = rmp_serde::to_vec(&chunk_message)?;

		// Send transfer type (0 for messages) + message length + message data
		send_stream.write_u8(0).await?; // Type 0 = message-based transfer
		send_stream
			.write_all(&(message_data.len() as u32).to_be_bytes())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to write message length: {}", e))?;
		send_stream
			.write_all(&message_data)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to write chunk data: {}", e))?;
		send_stream
			.flush()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to flush stream: {}", e))?;

		// Record chunk sent
		file_transfer_protocol.record_chunk_received(
			&transfer_id,
			chunk_index,
			bytes_read as u64,
		)?;

		// Update progress
		bytes_transferred += bytes_read as u64;
		if let Some(callback) = progress_callback {
			callback(bytes_transferred, total_size);
		}

		chunk_index += 1;

		// Log every 100 chunks or on first/last chunk
		if chunk_index == 1 || chunk_index % 100 == 0 || chunk_index == total_chunks as u32 {
			ctx.log(format!(
				"Sent chunk {}/{} ({} bytes total)",
				chunk_index, total_chunks, bytes_transferred
			));
		}

		// Yield to allow other tasks to run
		tokio::task::yield_now().await;
	}

	ctx.log(format!(
		"All {} chunks sent, sending completion message",
		chunk_index
	));

	// Send transfer completion message
	let final_checksum = calculate_file_checksum(file_path).await?;
	let completion_message =
		crate::service::network::protocol::file_transfer::FileTransferMessage::TransferComplete {
			transfer_id,
			final_checksum,
			total_bytes: bytes_transferred,
		};

	let completion_data = rmp_serde::to_vec(&completion_message)?;

	// Send completion message
	send_stream.write_u8(0).await?;
	send_stream.write_all(&(completion_data.len() as u32).to_be_bytes()).await?;
	send_stream.write_all(&completion_data).await?;
	send_stream.flush().await?;

	ctx.log(format!(
		"Completion message sent, waiting for final acknowledgment from receiver"
	));

	// Close the send side to signal we're done sending
	send_stream
		.finish()
		.map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

	// Wait for TransferFinalAck response from receiver
	ctx.log("Waiting for TransferFinalAck from receiver...".to_string());

	// Read the response message
	let mut msg_type = [0u8; 1];
	recv_stream.read_exact(&mut msg_type).await?;

	if msg_type[0] != 0 {
		return Err(anyhow::anyhow!("Unexpected response type: {}", msg_type[0]));
	}

	let mut len_buf = [0u8; 4];
	recv_stream.read_exact(&mut len_buf).await?;
	let msg_len = u32::from_be_bytes(len_buf) as usize;

	let mut msg_buf = vec![0u8; msg_len];
	recv_stream.read_exact(&mut msg_buf).await?;

	let ack_message: crate::service::network::protocol::file_transfer::FileTransferMessage =
		rmp_serde::from_slice(&msg_buf)?;

	// Verify it's a TransferFinalAck for our transfer
	match ack_message {
		crate::service::network::protocol::file_transfer::FileTransferMessage::TransferFinalAck { transfer_id: ack_id } => {
			if ack_id != transfer_id {
				return Err(anyhow::anyhow!("Received ack for wrong transfer: expected {}, got {}", transfer_id, ack_id));
			}
			ctx.log("Received TransferFinalAck from receiver - transfer confirmed!".to_string());
		}
		_ => {
			return Err(anyhow::anyhow!("Expected TransferFinalAck, got different message type"));
		}
	}

	// Now mark transfer as completed locally (only after receiving confirmation)
	file_transfer_protocol.update_session_state(
		&transfer_id,
		crate::service::network::protocol::file_transfer::TransferState::Completed,
	)?;

	ctx.log(format!(
		"File streaming completed and acknowledged: {} chunks, {} bytes sent to device {}",
		chunk_index, bytes_transferred, destination_device_id
	));

	Ok(())
}
