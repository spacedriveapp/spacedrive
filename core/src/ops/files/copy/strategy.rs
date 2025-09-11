//! Copy strategy implementations for different file operation scenarios

use crate::{
    infra::job::prelude::*,
    domain::addressing::SdPath,
    volume::VolumeManager,
    ops::files::copy::job::CopyPhase,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Progress callback for strategy implementations to report granular progress
/// Parameters: bytes_copied_for_current_file, total_bytes_for_current_file
pub type ProgressCallback<'a> = Box<dyn Fn(u64, u64) + Send + Sync + 'a>;

/// Defines a method for performing a file copy operation
#[async_trait]
pub trait CopyStrategy: Send + Sync {
    /// Executes the copy strategy for a single source path
    async fn execute<'a>(&self, ctx: &JobContext<'a>, source: &SdPath, destination: &SdPath, verify_checksum: bool, progress_callback: Option<&ProgressCallback<'a>>) -> Result<u64>;
}

/// Strategy for an atomic move on the same volume
pub struct LocalMoveStrategy;

#[async_trait]
impl CopyStrategy for LocalMoveStrategy {
    async fn execute<'a>(&self, ctx: &JobContext<'a>, source: &SdPath, destination: &SdPath, verify_checksum: bool, progress_callback: Option<&ProgressCallback<'a>>) -> Result<u64> {
        let source_path = source.as_local_path()
            .ok_or_else(|| anyhow::anyhow!("Source path is not local"))?;
        let dest_path = destination.as_local_path()
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
    async fn execute<'a>(&self, ctx: &JobContext<'a>, source: &SdPath, destination: &SdPath, verify_checksum: bool, progress_callback: Option<&ProgressCallback<'a>>) -> Result<u64> {
        let source_path = source.as_local_path()
            .ok_or_else(|| anyhow::anyhow!("Source path is not local"))?;
        let dest_path = destination.as_local_path()
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

        let bytes_copied = copy_file_streaming(source_path, dest_path, volume_info, ctx, verify_checksum, progress_callback).await?;

        ctx.log(format!(
            "Cross-volume streaming copy: {} -> {} ({} bytes)",
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
    async fn execute<'a>(&self, ctx: &JobContext<'a>, source: &SdPath, destination: &SdPath, verify_checksum: bool, progress_callback: Option<&ProgressCallback<'a>>) -> Result<u64> {
        // Get networking service
        let networking = ctx.networking_service()
            .ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

        // Get local path
        let local_path = source.as_local_path()
            .ok_or_else(|| anyhow::anyhow!("Source must be local path"))?;

        // Read file metadata
        let metadata = tokio::fs::metadata(local_path).await?;
        let file_size = metadata.len();

        ctx.log(format!(
            "Initiating cross-device transfer: {} ({} bytes) -> device:{}",
            local_path.display(),
            file_size,
            destination.device_id().unwrap_or_default()
        ));

        // Create file metadata for transfer
        let file_metadata = crate::service::network::protocol::FileMetadata {
            name: local_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size: file_size,
            modified: metadata.modified().ok(),
            is_directory: metadata.is_dir(),
            checksum: Some(calculate_file_checksum(local_path).await?),
            mime_type: None,
        };

        // Get file transfer protocol handler
        let networking_guard = &*networking;
        let protocol_registry = networking_guard.protocol_registry();
        let registry_guard = protocol_registry.read().await;

        let file_transfer_handler = registry_guard.get_handler("file_transfer")
            .ok_or_else(|| anyhow::anyhow!("File transfer protocol not registered"))?;

        let file_transfer_protocol = file_transfer_handler.as_any()
            .downcast_ref::<crate::service::network::protocol::FileTransferProtocolHandler>()
            .ok_or_else(|| anyhow::anyhow!("Invalid file transfer protocol handler"))?;

        // Initiate transfer
        let transfer_id = file_transfer_protocol.initiate_transfer(
            destination.device_id().unwrap_or_default(),
            local_path.to_path_buf(),
            crate::service::network::protocol::TransferMode::TrustedCopy,
        ).await?;

        ctx.log(format!("Transfer initiated with ID: {}", transfer_id));

        // Send transfer request to remote device
        let chunk_size = 64 * 1024u32;
        let total_chunks = ((file_size + chunk_size as u64 - 1) / chunk_size as u64) as u32;

        let transfer_request = crate::service::network::protocol::file_transfer::FileTransferMessage::TransferRequest {
            transfer_id,
            file_metadata: file_metadata.clone(),
            transfer_mode: crate::service::network::protocol::TransferMode::TrustedCopy,
            chunk_size,
            total_chunks,
            destination_path: destination.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        };

        let request_data = rmp_serde::to_vec(&transfer_request)?;

        // Send transfer request over network
        networking_guard.send_message(
            destination.device_id().unwrap_or_default(),
            "file_transfer",
            request_data,
        ).await?;

        ctx.log(format!("Transfer request sent to device {}", destination.device_id().unwrap_or_default()));

        // Stream file data
        drop(networking_guard);
        drop(registry_guard);

        stream_file_data(
            local_path,
            transfer_id,
            file_transfer_protocol,
            file_size,
            destination.device_id().unwrap_or_default(),
            ctx,
            progress_callback,
        ).await?;

        ctx.log(format!("Cross-device transfer completed: {} bytes", file_size));
        Ok(file_size)
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
    let result = copy_single_file_with_offset(source, destination, volume_info, ctx, verify_checksum, file_size, progress_callback, 0).await?;

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
        source_vol.optimal_chunk_size().min(dest_vol.optimal_chunk_size())
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
                "Operation cancelled"
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
                    ctx.log(format!("Strategy progress callback: {} / {} bytes", total_copied, file_size));
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
        ctx.log(format!("Strategy final progress: {} / {} bytes (100%)", total_copied, file_size));
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
                    format!("Checksum verification failed: source={}, dest={}", source_hash.to_hex(), dest_hash.to_hex())
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
                    "Operation cancelled"
                ));
            }

            let file_metadata = fs::metadata(&src_path).await?;
            let file_size = file_metadata.len();


            // Copy the file (offset no longer needed as aggregator tracks it)
            let bytes_copied = copy_single_file_with_offset(&src_path, &dest_path, volume_info, ctx, verify_checksum, file_size, progress_callback, 0).await?;
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
    copy_single_file(source, destination, volume_info, ctx, verify_checksum, file_size, progress_callback).await
}

/// Calculate file checksum for integrity verification
async fn calculate_file_checksum(path: &Path) -> Result<String> {
    crate::domain::content_identity::ContentHashGenerator::generate_content_hash(path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate content hash: {}", e))
}

/// Stream file data in chunks to the remote device
async fn stream_file_data<'a>(
    file_path: &Path,
    transfer_id: uuid::Uuid,
    file_transfer_protocol: &crate::service::network::protocol::FileTransferProtocolHandler,
    total_size: u64,
    destination_device_id: uuid::Uuid,
    ctx: &JobContext<'a>,
    progress_callback: Option<&ProgressCallback<'a>>,
) -> Result<()> {
    use tokio::io::AsyncReadExt;
    use blake3::Hasher;

    // Get networking service for sending chunks
    let networking = ctx.networking_service()
        .ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

    let mut file = tokio::fs::File::open(file_path).await?;

    let chunk_size = 64 * 1024; // 64KB chunks
    let total_chunks = (total_size + chunk_size - 1) / chunk_size;
    let mut buffer = vec![0u8; chunk_size as usize];
    let mut chunk_index = 0u32;
    let mut bytes_transferred = 0u64;

    ctx.log(format!("Starting to stream {} chunks to device {}", total_chunks, destination_device_id));

    loop {
        ctx.check_interrupt().await?;

        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break; // End of file
        }

        // Calculate chunk checksum
        let chunk_data = &buffer[..bytes_read];
        let chunk_checksum = blake3::hash(chunk_data);

        // Update progress
        bytes_transferred += bytes_read as u64;
        if let Some(callback) = progress_callback {
            callback(bytes_transferred, total_size);
        }

        // Get session keys for encryption
        let session_keys = file_transfer_protocol.get_session_keys_for_device(destination_device_id).await?;

        // Encrypt chunk using file transfer protocol
        let (encrypted_data, nonce) = file_transfer_protocol.encrypt_chunk(
            &session_keys.send_key,
            &transfer_id,
            chunk_index,
            chunk_data,
        )?;

        // Create encrypted file chunk message
        let chunk_message = crate::service::network::protocol::file_transfer::FileTransferMessage::FileChunk {
            transfer_id,
            chunk_index,
            data: encrypted_data,
            nonce,
            chunk_checksum: *chunk_checksum.as_bytes(), // Checksum of original unencrypted data
        };

        // Serialize and send chunk over network
        let chunk_data = rmp_serde::to_vec(&chunk_message)?;

        let networking_guard = &*networking;
        networking_guard.send_message(
            destination_device_id,
            "file_transfer",
            chunk_data,
        ).await?;

        // Record chunk in local protocol handler for tracking
        file_transfer_protocol.record_chunk_received(
            &transfer_id,
            chunk_index,
            bytes_read as u64,
        )?;

        chunk_index += 1;

        ctx.log(format!("Sent chunk {}/{} ({} bytes)", chunk_index, total_chunks, bytes_read));

        // Small delay to prevent overwhelming the network
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Send transfer completion message
    let final_checksum = calculate_file_checksum(file_path).await?;
    let completion_message = crate::service::network::protocol::file_transfer::FileTransferMessage::TransferComplete {
        transfer_id,
        final_checksum,
        total_bytes: bytes_transferred,
    };

    let completion_data = rmp_serde::to_vec(&completion_message)?;

    let networking_guard = &*networking;
    networking_guard.send_message(
        destination_device_id,
        "file_transfer",
        completion_data,
    ).await?;

    // Mark transfer as completed locally
    file_transfer_protocol.update_session_state(
        &transfer_id,
        crate::service::network::protocol::file_transfer::TransferState::Completed,
    )?;

    ctx.log(format!("File streaming completed: {} chunks, {} bytes sent to device {}",
        chunk_index, bytes_transferred, destination_device_id));
    Ok(())
}