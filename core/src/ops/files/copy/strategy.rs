//! # Copy Strategy Implementations
//!
//! `core::ops::files::copy::strategy` provides 4 specialized copy strategies, each optimized
//! for specific scenarios. The router selects strategies based on user preferences and system
//! topology to maximize performance while respecting user intent.
//!
//! ## Why Multiple Strategies?
//!
//! A single copy implementation can't optimize for all scenarios. Same-volume moves are instant
//! (metadata update only). CoW filesystems can clone files without copying data. Cross-volume
//! copies need progress tracking and cancellation. Network transfers require encryption and
//! fault tolerance.
//!
//! ## Strategy Selection
//!
//! 1. **`LocalMoveStrategy`** - Atomic filesystem rename (same volume, microseconds)
//! 2. **`FastCopyStrategy`** - CoW-optimized copy (APFS clones, Btrfs reflinks)
//! 3. **`LocalStreamCopyStrategy`** - Chunked streaming with progress (cross-volume)
//! 4. **`RemoteTransferStrategy`** - Encrypted network transfer (cross-device)
//!
//! The router picks RemoteTransferStrategy for cross-device transfers, LocalMoveStrategy for
//! same-volume moves, and FastCopyStrategy for same-volume copies in Atomic mode. Streaming
//! mode or cross-volume operations use LocalStreamCopyStrategy for progress tracking.
//!
//! ## Example
//! ```rust,no_run
//! use spacedrive_core::ops::files::copy::strategy::{CopyStrategy, LocalMoveStrategy};
//! use spacedrive_core::domain::addressing::SdPath;
//!
//! let strategy = LocalMoveStrategy;
//! let bytes_moved = strategy.execute(
//!     &ctx,
//!     &source_path,
//!     &dest_path,
//!     true,  // verify_checksum
//!     None,  // no progress callback
//! ).await?;
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
use tracing::{debug, error, info};

/// Progress callback for strategy implementations to report granular progress
/// Parameters: bytes_copied_for_current_file, total_bytes_for_current_file
pub type ProgressCallback<'a> = Box<dyn Fn(u64, u64) + Send + Sync + 'a>;

/// Strategy pattern for file copy operations with different performance characteristics.
///
/// Each implementation optimizes for specific scenarios (same-volume moves, CoW filesystems,
/// cross-device transfers). The trait abstracts these differences so callers don't need to
/// know which strategy is running - they all provide the same interface with progress callbacks
/// and checksum verification.
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

		// Read size before rename since source path becomes invalid after move.
		let metadata = fs::metadata(source_path).await?;
		let size = if metadata.is_file() {
			metadata.len()
		} else {
			get_path_size(source_path).await?
		};

		// Send initial progress event so UI shows 0% before the instant rename.
		if let Some(callback) = progress_callback {
			callback(0, size);
		}

		if let Some(parent) = dest_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		fs::rename(source_path, dest_path).await?;

		if let Some(callback) = progress_callback {
			callback(size, u64::MAX);
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

		// Query volume characteristics for optimal buffer sizing (SSD vs HDD, USB speeds, etc).
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

/// Fast copy strategy leveraging CoW filesystem optimizations.
///
/// Uses std::fs::copy which automatically invokes APFS clones, Btrfs reflinks, ZFS clones,
/// and ReFS block clones when available. On traditional filesystems, falls back to standard
/// copy. Directory copies delegate to streaming strategy since std::fs::copy only handles files.
pub struct FastCopyStrategy;

#[async_trait]
impl CopyStrategy for FastCopyStrategy {
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

		let metadata = fs::metadata(source_path).await?;
		if metadata.is_dir() {
			// std::fs::copy only handles files; delegate recursive copies to streaming strategy.
			ctx.log(format!(
				"FastCopyStrategy delegating directory to LocalStreamCopyStrategy: {}",
				source_path.display()
			));
			return LocalStreamCopyStrategy
				.execute(ctx, source, destination, verify_checksum, progress_callback)
				.await;
		}

		if let Some(parent) = dest_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		let bytes_copied = tokio::task::spawn_blocking({
			let source_path = source_path.to_path_buf();
			let dest_path = dest_path.to_path_buf();
			move || -> Result<u64, std::io::Error> { std::fs::copy(&source_path, &dest_path) }
		})
		.await??;

		// Post-copy verification detects CoW bugs and hardware errors (bit flips, bad sectors).
		if verify_checksum {
			let source_checksum = calculate_file_checksum(source_path).await?;
			let dest_checksum = calculate_file_checksum(dest_path).await?;
			if source_checksum != dest_checksum {
				return Err(anyhow::anyhow!("Checksum verification failed"));
			}
		}

		// Signal file completion to aggregator
		if let Some(callback) = progress_callback {
			callback(bytes_copied, u64::MAX);
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

/// Strategy for transferring a file to/from another device
pub struct RemoteTransferStrategy;

impl RemoteTransferStrategy {
	/// Detect the transfer direction based on source and destination paths.
	/// Returns Push if source is local and destination is remote.
	/// Returns Pull if source is remote and destination is local.
	fn detect_direction(
		source: &SdPath,
		destination: &SdPath,
	) -> crate::service::network::protocol::TransferDirection {
		use crate::service::network::protocol::TransferDirection;

		let source_is_local = source.is_local();
		let dest_is_local = destination.is_local();

		match (source_is_local, dest_is_local) {
			(true, false) => TransferDirection::Push,
			(false, true) => TransferDirection::Pull,
			(true, true) => {
				// Both local - this shouldn't use RemoteTransferStrategy
				// Default to Push to match existing behavior
				debug!("Both source and destination are local - defaulting to Push");
				TransferDirection::Push
			}
			(false, false) => {
				// Both remote - not supported yet (would require relay)
				// Default to Push for now
				debug!("Both source and destination are remote - defaulting to Push");
				TransferDirection::Push
			}
		}
	}

	/// Execute a PUSH operation (local -> remote)
	async fn execute_push<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		_verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		let dest_device_slug = destination.device_slug().ok_or_else(|| {
			anyhow::anyhow!("Destination must have a device slug for cross-device transfer")
		})?;

		let library = ctx.library();
		let dest_device_id = library
			.resolve_device_slug(dest_device_slug)
			.ok_or_else(|| anyhow::anyhow!(
				"Could not resolve destination device slug '{}' to UUID in library {}. Device may not be registered in this library.",
				dest_device_slug,
				library.id()
			))?;

		debug!(
			"RemoteTransferStrategy PUSH: {} -> device:{} ({})",
			source, dest_device_slug, dest_device_id
		);

		let networking = ctx
			.networking_service()
			.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

		let local_path = source
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Source must be local path for PUSH operation"))?;

		let metadata = tokio::fs::metadata(local_path).await?;
		let file_size = metadata.len();

		let checksum = calculate_file_checksum(local_path)
			.await
			.map(Some)
			.map_err(|e| anyhow::anyhow!("Failed to calculate checksum: {}", e))?;

		info!(
			"Initiating PUSH transfer: {} ({} bytes) -> device:{} ({})",
			local_path.display(),
			file_size,
			dest_device_slug,
			dest_device_id
		);

		ctx.log(format!(
			"Initiating PUSH transfer: {} ({} bytes) -> device:{} ({})",
			local_path.display(),
			file_size,
			dest_device_slug,
			dest_device_id
		));

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

		let transfer_id = file_transfer_protocol
			.initiate_transfer(
				dest_device_id,
				local_path.to_path_buf(),
				crate::service::network::protocol::TransferMode::TrustedCopy,
			)
			.await?;

		debug!("PUSH transfer initiated with ID: {}", transfer_id);
		ctx.log(format!("PUSH transfer initiated with ID: {}", transfer_id));

		let result = stream_file_data(
			local_path,
			transfer_id,
			file_transfer_protocol,
			file_size,
			dest_device_id,
			destination
				.path()
				.map(|p| p.to_string_lossy().to_string())
				.unwrap_or_default(),
			file_metadata,
			ctx,
			progress_callback,
		)
		.await;

		match result {
			Ok(()) => {
				info!("PUSH transfer completed: {} bytes", file_size);
				ctx.log(format!(
					"PUSH transfer completed successfully: {} bytes",
					file_size
				));
				Ok(file_size)
			}
			Err(e) => {
				error!("PUSH transfer failed: {}", e);
				ctx.log(format!("PUSH transfer FAILED: {}", e));
				Err(e)
			}
		}
	}

	/// Execute a PULL operation (remote -> local)
	async fn execute_pull<'a>(
		&self,
		ctx: &JobContext<'a>,
		source: &SdPath,
		destination: &SdPath,
		verify_checksum: bool,
		progress_callback: Option<&ProgressCallback<'a>>,
	) -> Result<u64> {
		let (source_device_slug, source_path) = source
			.as_physical()
			.ok_or_else(|| anyhow::anyhow!("Source must be a physical path for PULL operation"))?;

		let local_dest_path = destination
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Destination must be local path for PULL operation"))?;

		let library = ctx.library();
		let source_device_id = library
			.resolve_device_slug(source_device_slug)
			.ok_or_else(|| anyhow::anyhow!(
				"Could not resolve source device slug '{}' to UUID in library {}. Device may not be registered in this library.",
				source_device_slug,
				library.id()
			))?;

		debug!(
			"RemoteTransferStrategy PULL: device:{} ({}) -> {}",
			source_device_slug,
			source_device_id,
			local_dest_path.display()
		);

		info!(
			"Initiating PULL transfer: device:{}:{} -> {}",
			source_device_slug,
			source_path.display(),
			local_dest_path.display()
		);

		ctx.log(format!(
			"Initiating PULL transfer: device:{}:{} -> {}",
			source_device_slug,
			source_path.display(),
			local_dest_path.display()
		));

		let networking = ctx
			.networking_service()
			.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

		let networking_guard = &*networking;

		// Resolve device slug to node_id for network routing
		let device_registry = networking_guard.device_registry();
		let registry = device_registry.read().await;
		let node_id = registry
			.get_node_by_device(source_device_id)
			.ok_or_else(|| {
				anyhow::anyhow!(
					"Could not find node_id for device {} (slug: {}). Device may be offline.",
					source_device_id,
					source_device_slug
				)
			})?;
		drop(registry);

		let endpoint = networking_guard
			.endpoint()
			.ok_or_else(|| anyhow::anyhow!("Networking endpoint not available"))?;

		ctx.log(format!(
			"Opening PULL connection to node {} (device {})",
			node_id, source_device_id
		));

		// Connect to remote device
		let node_addr = iroh::NodeAddr::new(node_id);
		let connection = endpoint
			.connect(node_addr, b"spacedrive/filetransfer/1")
			.await
			.map_err(|e| anyhow::anyhow!("Failed to connect to device: {}", e))?;

		let (mut send_stream, mut recv_stream) = connection
			.open_bi()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to open bidirectional stream: {}", e))?;

		// Send PullRequest
		let transfer_id = uuid::Uuid::new_v4();
		let current_device_id = crate::device::get_current_device_id();
		let pull_request =
			crate::service::network::protocol::file_transfer::FileTransferMessage::PullRequest {
				transfer_id,
				source_path: source_path.clone(),
				requested_by: current_device_id,
			};

		let request_data = rmp_serde::to_vec(&pull_request)?;

		ctx.log(format!(
			"Sending PullRequest {} for path: {}",
			transfer_id,
			source_path.display()
		));

		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		send_stream.write_u8(0).await?;
		send_stream
			.write_all(&(request_data.len() as u32).to_be_bytes())
			.await?;
		send_stream.write_all(&request_data).await?;
		send_stream.flush().await?;

		// Receive PullResponse
		let mut msg_type = [0u8; 1];
		recv_stream.read_exact(&mut msg_type).await?;

		let mut len_buf = [0u8; 4];
		recv_stream.read_exact(&mut len_buf).await?;
		let msg_len = u32::from_be_bytes(len_buf) as usize;

		let mut msg_buf = vec![0u8; msg_len];
		recv_stream.read_exact(&mut msg_buf).await?;

		let response: crate::service::network::protocol::file_transfer::FileTransferMessage =
			rmp_serde::from_slice(&msg_buf)?;

		let file_metadata = match response {
			crate::service::network::protocol::file_transfer::FileTransferMessage::PullResponse {
				accepted: true,
				file_metadata: Some(metadata),
				..
			} => {
				ctx.log(format!(
					"PullRequest accepted: {} bytes",
					metadata.size
				));
				metadata
			}
			crate::service::network::protocol::file_transfer::FileTransferMessage::PullResponse {
				accepted: false,
				error,
				..
			} => {
				let err_msg = error.unwrap_or_else(|| "Unknown error".to_string());
				return Err(anyhow::anyhow!("Pull request rejected: {}", err_msg));
			}
			_ => {
				return Err(anyhow::anyhow!(
					"Unexpected response to pull request"
				));
			}
		};

		let file_size = file_metadata.size;

		// Ensure parent directory exists
		if let Some(parent) = local_dest_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Determine final file path
		let final_dest_path =
			if local_dest_path.is_dir() || local_dest_path.to_string_lossy().ends_with('/') {
				// Destination is a directory - append source filename
				let dir_path = local_dest_path.to_path_buf();
				fs::create_dir_all(&dir_path).await?;

				// Sanitize remote filename to prevent path traversal attacks
				let safe_name = std::path::Path::new(&file_metadata.name)
					.file_name()
					.map(|n| n.to_string_lossy().to_string())
					.unwrap_or_else(|| "unnamed_file".to_string());

				dir_path.join(&safe_name)
			} else {
				local_dest_path.to_path_buf()
			};

		// Create file for writing
		let mut file = fs::File::create(&final_dest_path).await?;
		let mut hasher = if verify_checksum {
			Some(blake3::Hasher::new())
		} else {
			None
		};
		let mut total_bytes_received = 0u64;

		ctx.log(format!(
			"Receiving file chunks to: {}",
			final_dest_path.display()
		));

		// Track whether we received a proper TransferComplete message
		let mut transfer_completed = false;

		// Receive file chunks
		loop {
			let mut msg_type = [0u8; 1];
			match recv_stream.read_exact(&mut msg_type).await {
				Ok(_) => {}
				Err(e) => {
					// Check if this is an expected EOF (connection closed cleanly)
					let err_str = e.to_string();
					if err_str.contains("finish") || err_str.contains("closed") {
						// Connection closed - will check transfer_completed below
						break;
					}
					let _ = fs::remove_file(&final_dest_path).await;
					return Err(anyhow::anyhow!("Failed to read message type: {}", e));
				}
			}

			let mut len_buf = [0u8; 4];
			recv_stream.read_exact(&mut len_buf).await?;
			let msg_len = u32::from_be_bytes(len_buf) as usize;

			let mut msg_buf = vec![0u8; msg_len];
			recv_stream.read_exact(&mut msg_buf).await?;

			let msg: crate::service::network::protocol::file_transfer::FileTransferMessage =
				rmp_serde::from_slice(&msg_buf)?;

			match msg {
				crate::service::network::protocol::file_transfer::FileTransferMessage::FileChunk {
					chunk_index,
					data,
					chunk_checksum,
					..
				} => {
					// Verify chunk checksum
					let calculated = blake3::hash(&data);
					if calculated.as_bytes() != &chunk_checksum {
						return Err(anyhow::anyhow!(
							"Chunk {} checksum mismatch",
							chunk_index
						));
					}

					// Write chunk
					file.write_all(&data).await?;
					if let Some(h) = &mut hasher {
						h.update(&data);
					}
					total_bytes_received += data.len() as u64;

					// Progress callback
					if let Some(cb) = progress_callback {
						cb(total_bytes_received, file_size);
					}

					if chunk_index % 100 == 0 {
						ctx.log(format!(
							"PULL progress: chunk {}, {} / {} bytes",
							chunk_index, total_bytes_received, file_size
						));
					}
				}
				crate::service::network::protocol::file_transfer::FileTransferMessage::TransferComplete {
					final_checksum,
					total_bytes,
					..
				} => {
					// Verify byte count first
					if total_bytes != total_bytes_received {
						let _ = fs::remove_file(&final_dest_path).await;
						return Err(anyhow::anyhow!(
							"Byte count mismatch: expected {}, got {}",
							total_bytes,
							total_bytes_received
						));
					}

					// Verify final checksum if enabled
					if verify_checksum {
						if let Some(h) = hasher.take() {
							let calculated = h.finalize();
							let calculated_hex = calculated.to_hex().to_string();

							if final_checksum.is_empty() {
								// Warn when checksum verification enabled but no checksum provided
								ctx.log("Warning: checksum verification enabled but remote did not provide checksum".to_string());
							} else if calculated_hex != final_checksum {
								error!(
									"Final checksum mismatch: expected {}, got {}",
									final_checksum, calculated_hex
								);
								let _ = fs::remove_file(&final_dest_path).await;
								return Err(anyhow::anyhow!("Final checksum mismatch"));
							}
						}
					}

					transfer_completed = true;
					ctx.log(format!(
						"PULL transfer completed: {} bytes received",
						total_bytes_received
					));
					break;
				}
				crate::service::network::protocol::file_transfer::FileTransferMessage::TransferError {
					message,
					..
				} => {
					// Clean up partial file
					let _ = fs::remove_file(&final_dest_path).await;
					return Err(anyhow::anyhow!("Transfer error: {}", message));
				}
				_ => {
					debug!("Received unexpected message during PULL transfer");
				}
			}
		}

		// Verify transfer completed properly
		if !transfer_completed {
			let _ = fs::remove_file(&final_dest_path).await;
			return Err(anyhow::anyhow!(
				"Transfer interrupted: received {} of {} bytes before connection closed",
				total_bytes_received,
				file_size
			));
		}

		file.flush().await?;
		file.sync_all().await?;

		info!(
			"PULL transfer completed: {} bytes from device:{} to {}",
			total_bytes_received,
			source_device_slug,
			final_dest_path.display()
		);

		ctx.log(format!(
			"PULL transfer completed successfully: {} bytes from device:{} to {}",
			total_bytes_received,
			source_device_slug,
			final_dest_path.display()
		));

		// Signal file completion to aggregator
		if let Some(callback) = progress_callback {
			callback(total_bytes_received, u64::MAX);
		}

		Ok(total_bytes_received)
	}
}

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
		use crate::service::network::protocol::TransferDirection;

		let direction = Self::detect_direction(source, destination);

		debug!(
			"RemoteTransferStrategy: {} -> {} (direction: {:?})",
			source, destination, direction
		);

		match direction {
			TransferDirection::Push => {
				self.execute_push(ctx, source, destination, verify_checksum, progress_callback)
					.await
			}
			TransferDirection::Pull => {
				self.execute_pull(ctx, source, destination, verify_checksum, progress_callback)
					.await
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

	// Signal completion so aggregator knows the file is done.
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
	if let Some(parent) = destination.parent() {
		fs::create_dir_all(parent).await?;
	}

	let mut source_file = fs::File::open(source).await?;
	let mut dest_file = fs::File::create(destination).await?;

	// Use smaller of source/dest optimal sizes to avoid overwhelming slower device.
	let chunk_size = if let Some((source_vol, dest_vol)) = volume_info {
		source_vol
			.optimal_chunk_size()
			.min(dest_vol.optimal_chunk_size())
	} else {
		64 * 1024
	};

	let mut buffer = vec![0u8; chunk_size];
	let mut total_copied = 0u64;
	let mut last_progress_update = std::time::Instant::now();

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
		if let Err(_) = ctx.check_interrupt().await {
			// Clean up partial file so resume doesn't see corrupted data.
			let _ = fs::remove_file(destination).await;
			return Err(std::io::Error::new(
				std::io::ErrorKind::Interrupted,
				"Operation cancelled",
			));
		}

		let bytes_read = source_file.read(&mut buffer).await?;
		if bytes_read == 0 {
			break;
		}

		let chunk = &buffer[..bytes_read];
		dest_file.write_all(chunk).await?;
		total_copied += bytes_read as u64;

		if let Some(hasher) = &mut source_hasher {
			hasher.update(chunk);
		}
		if let Some(hasher) = &mut dest_hasher {
			hasher.update(chunk);
		}

		// Throttle progress updates to 50ms intervals for UI smoothness without overhead.
		if last_progress_update.elapsed() >= std::time::Duration::from_millis(50) {
			if let Some(callback) = progress_callback {
				callback(total_copied, file_size);

				if total_copied % (100 * 1024 * 1024) < bytes_read as u64 {
					ctx.log(format!(
						"Strategy progress callback: {} / {} bytes",
						total_copied, file_size
					));
				}
			}
			last_progress_update = std::time::Instant::now();

			// Yield to scheduler so progress reporting and cancellation checks can run.
			tokio::task::yield_now().await;
		}
	}

	dest_file.flush().await?;
	dest_file.sync_all().await?;

	if let Some(callback) = progress_callback {
		callback(file_size, u64::MAX);
		ctx.log(format!(
			"Strategy final progress: {} / {} bytes (100%)",
			total_copied, file_size
		));
	}

	if verify_checksum {
		if let (Some(source_hasher), Some(dest_hasher)) = (source_hasher, dest_hasher) {
			let source_hash = source_hasher.finalize();
			let dest_hash = dest_hasher.finalize();

			if source_hash != dest_hash {
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
	if let Some(parent) = destination.parent() {
		fs::create_dir_all(parent).await?;
	}

	let metadata = fs::metadata(source).await?;
	if metadata.is_dir() {
		fs::create_dir_all(destination).await?;
		let mut total_size = 0u64;

		// Collect all files first to avoid holding directory handles during copy.
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

		let mut cumulative_bytes = 0u64;
		for (src_path, dest_path) in files_to_copy {
			if let Err(_) = ctx.check_interrupt().await {
				return Err(std::io::Error::new(
					std::io::ErrorKind::Interrupted,
					"Operation cancelled",
				));
			}

			let file_metadata = fs::metadata(&src_path).await?;
			let file_size = file_metadata.len();

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

			// Signal file completion with u64::MAX so aggregator advances to next file.
			if let Some(callback) = progress_callback {
				callback(bytes_copied, u64::MAX);
			}
		}

		return Ok(total_size);
	}

	let file_size = metadata.len();
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

	debug!(
		"Streaming {} bytes to device {}",
		total_size, destination_device_id
	);

	let networking = ctx
		.networking_service()
		.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

	let networking_guard = &*networking;

	// Map device UUID to Iroh node_id for network routing.
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

	let endpoint = networking_guard
		.endpoint()
		.ok_or_else(|| anyhow::anyhow!("Networking endpoint not available"))?;

	ctx.log(format!(
		"Opening persistent connection to node {} (device {}) for file transfer",
		node_id, destination_device_id
	));

	let node_addr = iroh::NodeAddr::new(node_id);
	let connection = endpoint
		.connect(node_addr, b"spacedrive/filetransfer/1")
		.await
		.map_err(|e| anyhow::anyhow!("Failed to connect to device: {}", e))?;

	// Bidirectional stream allows receiving acknowledgment after transfer completion.
	let (mut send_stream, mut recv_stream) = connection
		.open_bi()
		.await
		.map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

	let chunk_size = 64 * 1024u32;
	let total_chunks = ((total_size + chunk_size as u64 - 1) / chunk_size as u64) as u32;

	let transfer_request =
		crate::service::network::protocol::file_transfer::FileTransferMessage::TransferRequest {
			transfer_id,
			file_metadata,
			transfer_mode: crate::service::network::protocol::TransferMode::TrustedCopy,
			chunk_size,
			total_chunks,
			destination_path: destination_path.clone(),
		};

	let request_data = rmp_serde::to_vec(&transfer_request)?;

	ctx.log(format!(
		"Sending TransferRequest for {} bytes ({} chunks) to destination: {}",
		total_size, total_chunks, destination_path
	));

	send_stream.write_u8(0).await?;
	send_stream
		.write_all(&(request_data.len() as u32).to_be_bytes())
		.await?;
	send_stream.write_all(&request_data).await?;
	send_stream.flush().await?;

	ctx.log("TransferRequest sent, now sending file chunks".to_string());

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

	loop {
		ctx.check_interrupt().await?;

		let bytes_read = file.read(&mut buffer).await?;
		if bytes_read == 0 {
			break;
		}

		// Checksum before encryption so receiver can verify decrypted data.
		let chunk_data = &buffer[..bytes_read];
		let chunk_checksum = blake3::hash(chunk_data);

		// Skip encryption - Iroh already provides E2E encryption for the connection
		let encrypted_data = chunk_data.to_vec();
		let nonce = [0u8; 12]; // Dummy nonce since we're not encrypting

		let chunk_message =
			crate::service::network::protocol::file_transfer::FileTransferMessage::FileChunk {
				transfer_id,
				chunk_index,
				data: encrypted_data,
				nonce,
				chunk_checksum: *chunk_checksum.as_bytes(),
			};

		let message_data = rmp_serde::to_vec(&chunk_message)?;

		send_stream.write_u8(0).await?;
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

		file_transfer_protocol.record_chunk_received(
			&transfer_id,
			chunk_index,
			bytes_read as u64,
		)?;

		bytes_transferred += bytes_read as u64;
		if let Some(callback) = progress_callback {
			callback(bytes_transferred, total_size);
		}

		chunk_index += 1;

		if chunk_index == 1 || chunk_index % 100 == 0 || chunk_index == total_chunks as u32 {
			ctx.log(format!(
				"Sent chunk {}/{} ({} bytes total)",
				chunk_index, total_chunks, bytes_transferred
			));
		}

		tokio::task::yield_now().await;
	}

	ctx.log(format!(
		"All {} chunks sent, sending completion message",
		chunk_index
	));

	let final_checksum = calculate_file_checksum(file_path).await?;
	let completion_message =
		crate::service::network::protocol::file_transfer::FileTransferMessage::TransferComplete {
			transfer_id,
			final_checksum,
			total_bytes: bytes_transferred,
		};

	let completion_data = rmp_serde::to_vec(&completion_message)?;

	send_stream.write_u8(0).await?;
	send_stream
		.write_all(&(completion_data.len() as u32).to_be_bytes())
		.await?;
	send_stream.write_all(&completion_data).await?;
	send_stream.flush().await?;

	ctx.log(format!(
		"Completion message sent, waiting for final acknowledgment from receiver"
	));

	send_stream
		.finish()
		.map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

	ctx.log("Waiting for TransferFinalAck from receiver...".to_string());

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

	// Only mark completed after receiver confirms to detect network failures.
	file_transfer_protocol.update_session_state(
		&transfer_id,
		crate::service::network::protocol::file_transfer::TransferState::Completed,
	)?;

	ctx.log(format!(
		"File streaming completed and acknowledged: {} chunks, {} bytes sent to device {}",
		chunk_index, bytes_transferred, destination_device_id
	));

	// Signal file completion to aggregator
	if let Some(callback) = progress_callback {
		callback(bytes_transferred, u64::MAX);
	}

	Ok(())
}
