//! File transfer protocol for cross-device file operations

use crate::service::network::utils::logging::NetworkLogger;
use crate::service::network::{NetworkingError, Result};
use async_trait::async_trait;
use iroh::NodeId;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	path::PathBuf,
	sync::{Arc, RwLock},
	time::{Duration, SystemTime},
};
use tokio::{fs::File, io::AsyncReadExt};
use uuid::Uuid;

// Encryption imports
use chacha20poly1305::{
	aead::{Aead, AeadCore, KeyInit, OsRng},
	ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

/// Session keys for device-to-device encryption
#[derive(Debug, Clone)]
pub struct SessionKeys {
	pub send_key: Vec<u8>,    // 32-byte HKDF-derived send key
	pub receive_key: Vec<u8>, // 32-byte HKDF-derived receive key
}

/// File transfer protocol handler
pub struct FileTransferProtocolHandler {
	/// Active transfer sessions
	sessions: Arc<RwLock<HashMap<Uuid, TransferSession>>>,
	/// Protocol configuration
	config: TransferConfig,
	/// Device registry for session keys
	device_registry:
		Option<Arc<tokio::sync::RwLock<crate::service::network::device::DeviceRegistry>>>,
	/// Logger for protocol operations
	logger: Arc<dyn NetworkLogger>,
}

/// Configuration for file transfers
#[derive(Debug, Clone)]
pub struct TransferConfig {
	/// Default chunk size for file streaming
	pub chunk_size: u32,
	/// Maximum concurrent transfers
	pub max_concurrent_transfers: u32,
	/// Transfer timeout
	pub transfer_timeout: Duration,
	/// Enable integrity verification
	pub verify_checksums: bool,
}

impl Default for TransferConfig {
	fn default() -> Self {
		Self {
			chunk_size: 64 * 1024, // 64KB chunks
			max_concurrent_transfers: 10,
			transfer_timeout: Duration::from_secs(300), // 5 minutes
			verify_checksums: true,
		}
	}
}

/// Active transfer session
#[derive(Debug, Clone)]
pub struct TransferSession {
	pub id: Uuid,
	pub file_metadata: FileMetadata,
	pub mode: TransferMode,
	pub state: TransferState,
	pub created_at: SystemTime,
	pub bytes_transferred: u64,
	pub chunks_received: Vec<u32>,
	pub source_device: Option<Uuid>,
	pub destination_device: Option<Uuid>,
	pub destination_path: String,
}

/// Transfer state machine
#[derive(Debug, Clone, PartialEq)]
pub enum TransferState {
	/// Waiting for transfer to be accepted
	Pending,
	/// Transfer in progress
	Active,
	/// Transfer completed successfully
	Completed,
	/// Transfer failed
	Failed(String),
	/// Transfer cancelled
	Cancelled,
}

/// Transfer modes for different use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferMode {
	/// Trusted device copy (automatic, uses session keys)
	TrustedCopy,
	/// Ephemeral sharing (requires consent, uses ephemeral keys)
	EphemeralShare {
		ephemeral_pubkey: [u8; 32],
		sender_identity: String,
	},
}

/// File metadata for transfer operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
	pub name: String,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub is_directory: bool,
	pub checksum: Option<String>, // ContentHashGenerator hash
	pub mime_type: Option<String>,
}

/// Universal message types for file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferMessage {
	/// Request to initiate file transfer
	TransferRequest {
		transfer_id: Uuid,
		file_metadata: FileMetadata,
		transfer_mode: TransferMode,
		chunk_size: u32,
		total_chunks: u32,
		destination_path: String,
	},

	/// Response to transfer request
	TransferResponse {
		transfer_id: Uuid,
		accepted: bool,
		reason: Option<String>,
		supported_resume: bool,
	},

	/// File data chunk
	FileChunk {
		transfer_id: Uuid,
		chunk_index: u32,
		data: Vec<u8>,            // Encrypted data
		nonce: [u8; 12],          // ChaCha20-Poly1305 nonce
		chunk_checksum: [u8; 32], // Checksum of original (unencrypted) data
	},

	/// Acknowledge received chunk
	ChunkAck {
		transfer_id: Uuid,
		chunk_index: u32,
		next_expected: u32,
	},

	/// Transfer completion notification
	TransferComplete {
		transfer_id: Uuid,
		final_checksum: String, // ContentHashGenerator hash
		total_bytes: u64,
	},

	/// Transfer error or cancellation
	TransferError {
		transfer_id: Uuid,
		error_type: TransferErrorType,
		message: String,
		recoverable: bool,
	},

	/// Final acknowledgment from receiver after getting TransferComplete
	TransferFinalAck { transfer_id: Uuid },
}

/// Types of transfer errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferErrorType {
	NetworkError,
	FileSystemError,
	PermissionDenied,
	InsufficientSpace,
	ChecksumMismatch,
	Timeout,
	Cancelled,
	ProtocolError,
}

impl FileTransferProtocolHandler {
	/// Create a new file transfer protocol handler
	pub fn new(config: TransferConfig, logger: Arc<dyn NetworkLogger>) -> Self {
		Self {
			sessions: Arc::new(RwLock::new(HashMap::new())),
			config,
			device_registry: None,
			logger,
		}
	}

	/// Helper function to create a truncated version of FileTransferMessage for logging
	fn truncate_message_for_logging(message: &FileTransferMessage) -> String {
		match message {
			FileTransferMessage::TransferRequest {
				transfer_id,
				file_metadata,
				transfer_mode,
				chunk_size,
				total_chunks,
				destination_path,
			} => {
				format!("TransferRequest {{ transfer_id: {}, file_metadata: FileMetadata {{ name: \"{}\", size: {}, is_directory: {}, checksum: {:?}, .. }}, transfer_mode: {:?}, chunk_size: {}, total_chunks: {}, destination_path: \"{}\" }}",
					transfer_id, file_metadata.name, file_metadata.size, file_metadata.is_directory,
					file_metadata.checksum.as_ref().map(|c| &c[..16]).unwrap_or("None"),
					transfer_mode, chunk_size, total_chunks, destination_path)
			}
			FileTransferMessage::FileChunk {
				transfer_id,
				chunk_index,
				data,
				nonce,
				chunk_checksum,
			} => {
				format!("FileChunk {{ transfer_id: {}, chunk_index: {}, data: [{} bytes], nonce: [{} bytes], chunk_checksum: [{} bytes] }}",
					transfer_id, chunk_index, data.len(), nonce.len(), chunk_checksum.len())
			}
			FileTransferMessage::TransferComplete {
				transfer_id,
				final_checksum,
				total_bytes,
			} => {
				format!("TransferComplete {{ transfer_id: {}, final_checksum: \"{}\", total_bytes: {} }}",
					transfer_id,
					if final_checksum.len() > 16 { format!("{}...", &final_checksum[..16]) } else { final_checksum.clone() },
					total_bytes)
			}
			FileTransferMessage::TransferResponse {
				transfer_id,
				accepted,
				reason,
				supported_resume,
			} => {
				format!("TransferResponse {{ transfer_id: {}, accepted: {}, reason: {:?}, supported_resume: {} }}",
					transfer_id, accepted, reason, supported_resume)
			}
			FileTransferMessage::ChunkAck {
				transfer_id,
				chunk_index,
				next_expected,
			} => {
				format!(
					"ChunkAck {{ transfer_id: {}, chunk_index: {}, next_expected: {} }}",
					transfer_id, chunk_index, next_expected
				)
			}
			FileTransferMessage::TransferError {
				transfer_id,
				error_type,
				message,
				recoverable,
			} => {
				format!("TransferError {{ transfer_id: {}, error_type: {:?}, message: \"{}\", recoverable: {} }}",
					transfer_id, error_type, message, recoverable)
			}
			FileTransferMessage::TransferFinalAck { transfer_id } => {
				format!("TransferFinalAck {{ transfer_id: {} }}", transfer_id)
			}
		}
	}

	/// Set the device registry for session key lookup
	pub fn set_device_registry(
		&mut self,
		device_registry: Arc<tokio::sync::RwLock<crate::service::network::device::DeviceRegistry>>,
	) {
		self.device_registry = Some(device_registry);
	}

	/// Derive chunk encryption key from session keys
	fn derive_chunk_key(
		&self,
		session_send_key: &[u8],
		transfer_id: &Uuid,
		chunk_index: u32,
	) -> Result<[u8; 32]> {
		let hk = Hkdf::<Sha256>::new(None, session_send_key);
		let info = format!("spacedrive-chunk-{}-{}", transfer_id, chunk_index);
		let mut key = [0u8; 32];
		hk.expand(info.as_bytes(), &mut key)
			.map_err(|e| NetworkingError::Protocol(format!("Key derivation failed: {}", e)))?;
		Ok(key)
	}

	/// Encrypt chunk data using ChaCha20-Poly1305
	pub fn encrypt_chunk(
		&self,
		session_send_key: &[u8],
		transfer_id: &Uuid,
		chunk_index: u32,
		data: &[u8],
	) -> Result<(Vec<u8>, [u8; 12])> {
		// Derive chunk-specific key
		let chunk_key = self.derive_chunk_key(session_send_key, transfer_id, chunk_index)?;

		// Create cipher
		let cipher = ChaCha20Poly1305::new_from_slice(&chunk_key)
			.map_err(|e| NetworkingError::Protocol(format!("Cipher creation failed: {}", e)))?;

		// Generate nonce
		let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

		// Encrypt data
		let ciphertext = cipher
			.encrypt(&nonce, data)
			.map_err(|e| NetworkingError::Protocol(format!("Encryption failed: {}", e)))?;

		Ok((ciphertext, nonce.into()))
	}

	/// Decrypt chunk data using ChaCha20-Poly1305
	fn decrypt_chunk(
		&self,
		session_receive_key: &[u8],
		transfer_id: &Uuid,
		chunk_index: u32,
		encrypted_data: &[u8],
		nonce: &[u8; 12],
	) -> Result<Vec<u8>> {
		// Derive same chunk-specific key (using receive key)
		let chunk_key = self.derive_chunk_key(session_receive_key, transfer_id, chunk_index)?;

		// Create cipher
		let cipher = ChaCha20Poly1305::new_from_slice(&chunk_key)
			.map_err(|e| NetworkingError::Protocol(format!("Cipher creation failed: {}", e)))?;

		// Decrypt data
		let nonce = Nonce::from_slice(nonce);
		let plaintext = cipher
			.decrypt(nonce, encrypted_data)
			.map_err(|e| NetworkingError::Protocol(format!("Decryption failed: {}", e)))?;

		Ok(plaintext)
	}

	/// Get session keys for a device from the device registry
	pub async fn get_session_keys_for_device(&self, device_id: Uuid) -> Result<SessionKeys> {
		let device_registry = self
			.device_registry
			.as_ref()
			.ok_or_else(|| NetworkingError::Protocol("Device registry not set".to_string()))?;

		let registry_guard = device_registry.read().await;
		let session_keys = registry_guard.get_session_keys(device_id).ok_or_else(|| {
			NetworkingError::Protocol(format!("No session keys found for device {}", device_id))
		})?;

		Ok(SessionKeys {
			send_key: session_keys.send_key,
			receive_key: session_keys.receive_key,
		})
	}

	/// Create with default configuration
	pub fn new_default(logger: Arc<dyn NetworkLogger>) -> Self {
		Self::new(TransferConfig::default(), logger)
	}

	/// Initiate a file transfer to a device
	pub async fn initiate_transfer(
		&self,
		target_device: Uuid,
		file_path: PathBuf,
		transfer_mode: TransferMode,
	) -> Result<Uuid> {
		// Read file metadata
		let metadata = tokio::fs::metadata(&file_path).await.map_err(|e| {
			NetworkingError::file_system_error(format!("Failed to read file metadata: {}", e))
		})?;

		let file_metadata = FileMetadata {
			name: file_path
				.file_name()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string(),
			size: metadata.len(),
			modified: metadata.modified().ok(),
			is_directory: metadata.is_dir(),
			checksum: if self.config.verify_checksums {
				Some(self.calculate_file_checksum(&file_path).await?)
			} else {
				None
			},
			mime_type: None, // TODO: Add MIME type detection
		};

		let transfer_id = Uuid::new_v4();
		let session = TransferSession {
			id: transfer_id,
			file_metadata: file_metadata.clone(),
			mode: transfer_mode.clone(),
			state: TransferState::Pending,
			created_at: SystemTime::now(),
			bytes_transferred: 0,
			chunks_received: Vec::new(),
			source_device: None, // Will be set when we know our device ID
			destination_device: Some(target_device),
			destination_path: "/tmp".to_string(), // Default destination, will be set by caller
		};

		// Store session
		{
			let mut sessions = self.sessions.write().unwrap();
			sessions.insert(transfer_id, session);
		}

		Ok(transfer_id)
	}

	/// Get transfer session by ID
	pub fn get_session(&self, transfer_id: &Uuid) -> Option<TransferSession> {
		let sessions = self.sessions.read().unwrap();
		sessions.get(transfer_id).cloned()
	}

	/// Update transfer session state
	pub fn update_session_state(&self, transfer_id: &Uuid, state: TransferState) -> Result<()> {
		let mut sessions = self.sessions.write().unwrap();
		if let Some(session) = sessions.get_mut(transfer_id) {
			session.state = state;
			Ok(())
		} else {
			Err(NetworkingError::transfer_not_found_error(*transfer_id))
		}
	}

	/// Record chunk received
	pub fn record_chunk_received(
		&self,
		transfer_id: &Uuid,
		chunk_index: u32,
		bytes: u64,
	) -> Result<()> {
		let mut sessions = self.sessions.write().unwrap();
		if let Some(session) = sessions.get_mut(transfer_id) {
			session.chunks_received.push(chunk_index);
			session.bytes_transferred += bytes;
			Ok(())
		} else {
			Err(NetworkingError::transfer_not_found_error(*transfer_id))
		}
	}

	/// Calculate file checksum using ContentHashGenerator
	async fn calculate_file_checksum(&self, path: &PathBuf) -> Result<String> {
		crate::domain::content_identity::ContentHashGenerator::generate_content_hash(path)
			.await
			.map_err(|e| {
				NetworkingError::file_system_error(format!(
					"Failed to generate content hash: {}",
					e
				))
			})
	}

	/// Calculate file checksum as bytes for compatibility
	async fn calculate_file_checksum_bytes(&self, path: &PathBuf) -> Result<[u8; 32]> {
		// Generate the content hash and then hash it again for 32-byte output
		let content_hash = self.calculate_file_checksum(path).await?;
		let mut hasher = blake3::Hasher::new();
		hasher.update(content_hash.as_bytes());
		Ok(hasher.finalize().into())
	}

	/// Handle transfer request message
	async fn handle_transfer_request(
		&self,
		from_device: Uuid,
		request: FileTransferMessage,
	) -> Result<FileTransferMessage> {
		if let FileTransferMessage::TransferRequest {
			transfer_id,
			file_metadata,
			transfer_mode,
			destination_path,
			..
		} = request
		{
			// For trusted devices, auto-accept transfers
			let accepted = match transfer_mode {
				TransferMode::TrustedCopy => true,
				TransferMode::EphemeralShare { .. } => {
					// For ephemeral shares, would need user consent
					// For now, auto-accept but this should trigger UI prompt
					true
				}
			};

			if accepted {
				// Create session for incoming transfer
				let session = TransferSession {
					id: transfer_id,
					file_metadata,
					mode: transfer_mode,
					state: TransferState::Active,
					created_at: SystemTime::now(),
					bytes_transferred: 0,
					chunks_received: Vec::new(),
					source_device: Some(from_device),
					destination_device: None, // We are the destination
					destination_path,
				};

				let mut sessions = self.sessions.write().unwrap();
				sessions.insert(transfer_id, session);
			}

			Ok(FileTransferMessage::TransferResponse {
				transfer_id,
				accepted,
				reason: if accepted {
					None
				} else {
					Some("User declined".to_string())
				},
				supported_resume: true,
			})
		} else {
			Err(NetworkingError::Protocol(
				"Invalid transfer request message".to_string(),
			))
		}
	}

	/// Handle file chunk message
	async fn handle_file_chunk(
		&self,
		from_device: Uuid,
		chunk: FileTransferMessage,
	) -> Result<FileTransferMessage> {
		if let FileTransferMessage::FileChunk {
			transfer_id,
			chunk_index,
			data,
			nonce,
			chunk_checksum,
		} = chunk
		{
			// Get session keys for decryption
			let session_keys = self.get_session_keys_for_device(from_device).await?;

			// Decrypt chunk data
			let decrypted_data = self.decrypt_chunk(
				&session_keys.receive_key,
				&transfer_id,
				chunk_index,
				&data,
				&nonce,
			)?;

			// Verify chunk checksum (of decrypted data)
			if self.config.verify_checksums {
				let calculated_checksum = blake3::hash(&decrypted_data);
				if calculated_checksum.as_bytes() != &chunk_checksum {
					return Ok(FileTransferMessage::TransferError {
						transfer_id,
						error_type: TransferErrorType::ChecksumMismatch,
						message: format!("Chunk {} checksum mismatch", chunk_index),
						recoverable: true,
					});
				}
			}

			// Record chunk received (using decrypted size)
			self.record_chunk_received(&transfer_id, chunk_index, decrypted_data.len() as u64)?;

			// Write decrypted chunk to file
			self.write_chunk_to_file(&transfer_id, chunk_index, &decrypted_data)
				.await
				.map_err(|e| {
					NetworkingError::Protocol(format!("Failed to write chunk to file: {}", e))
				})?;

			// Calculate next expected chunk
			let next_expected = {
				let sessions = self.sessions.read().unwrap();
				if let Some(session) = sessions.get(&transfer_id) {
					let mut received_chunks = session.chunks_received.clone();
					received_chunks.sort();

					// Find the first missing chunk
					let mut next = 0;
					for &chunk in &received_chunks {
						if chunk == next {
							next += 1;
						} else {
							break;
						}
					}
					next
				} else {
					return Err(NetworkingError::transfer_not_found_error(transfer_id));
				}
			};

			Ok(FileTransferMessage::ChunkAck {
				transfer_id,
				chunk_index,
				next_expected,
			})
		} else {
			Err(NetworkingError::Protocol(
				"Invalid file chunk message".to_string(),
			))
		}
	}

	/// Handle transfer completion
	async fn handle_transfer_complete(
		&self,
		from_device: Uuid,
		completion: FileTransferMessage,
	) -> Result<FileTransferMessage> {
		if let FileTransferMessage::TransferComplete {
			transfer_id,
			final_checksum,
			total_bytes,
		} = completion
		{
			// Verify final checksum if configured
			if self.config.verify_checksums {
				// Get the received file path
				let received_file_path = {
					let sessions = self.sessions.read().unwrap();
					if let Some(session) = sessions.get(&transfer_id) {
						let destination_path = PathBuf::from(&session.destination_path);
						destination_path.join(&session.file_metadata.name)
					} else {
						return Err(NetworkingError::transfer_not_found_error(transfer_id));
					}
				};

				// Calculate checksum of received file
				let received_checksum = self.calculate_file_checksum(&received_file_path).await?;

				// Compare with sender's checksum
				if received_checksum != final_checksum {
					self.update_session_state(
						&transfer_id,
						TransferState::Failed(format!(
							"Final checksum mismatch: expected {}, got {}",
							final_checksum, received_checksum
						)),
					)?;

					return Ok(FileTransferMessage::TransferError {
						transfer_id,
						error_type: TransferErrorType::ChecksumMismatch,
						message: "Final file checksum verification failed".to_string(),
						recoverable: false,
					});
				}

				println!("File checksum verified: {}", received_checksum);
			}

			// Mark transfer as completed
			self.update_session_state(&transfer_id, TransferState::Completed)?;

			println!(
				"File transfer {} completed: {} bytes",
				transfer_id, total_bytes
			);

			// Return final acknowledgment
			Ok(FileTransferMessage::TransferFinalAck { transfer_id })
		} else {
			Err(NetworkingError::Protocol(
				"Invalid transfer complete message".to_string(),
			))
		}
	}

	/// Get active transfers
	pub fn get_active_transfers(&self) -> Vec<TransferSession> {
		let sessions = self.sessions.read().unwrap();
		sessions
			.values()
			.filter(|session| {
				matches!(
					session.state,
					TransferState::Active | TransferState::Pending
				)
			})
			.cloned()
			.collect()
	}

	/// Cancel a transfer
	pub fn cancel_transfer(&self, transfer_id: &Uuid) -> Result<()> {
		self.update_session_state(transfer_id, TransferState::Cancelled)
	}

	/// Clean up completed/failed transfers older than specified duration
	pub fn cleanup_old_transfers(&self, max_age: Duration) {
		let mut sessions = self.sessions.write().unwrap();
		let cutoff = SystemTime::now() - max_age;

		sessions.retain(|_, session| match session.state {
			TransferState::Active | TransferState::Pending => true,
			_ => session.created_at > cutoff,
		});
	}

	/// Write a file chunk to the destination file
	async fn write_chunk_to_file(
		&self,
		transfer_id: &Uuid,
		chunk_index: u32,
		data: &[u8],
	) -> std::result::Result<(), String> {
		use tokio::io::{AsyncSeekExt, AsyncWriteExt};

		// Get session info to determine file path and chunk size
		let (file_path, chunk_size) = {
			let sessions = self.sessions.read().unwrap();
			let session = sessions
				.get(transfer_id)
				.ok_or_else(|| "Transfer session not found".to_string())?;

			// Use the destination path from the transfer request (already includes filename)
			let file_path = PathBuf::from(&session.destination_path);

			(file_path, 64 * 1024u32) // 64KB chunk size
		};

		// Ensure parent directory exists
		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent)
				.await
				.map_err(|e| format!("Failed to create parent directory: {}", e))?;
		}

		// Open file for writing (create if doesn't exist)
		let mut file = tokio::fs::OpenOptions::new()
			.create(true)
			.write(true)
			.open(&file_path)
			.await
			.map_err(|e| format!("Failed to open file for writing: {}", e))?;

		// Calculate file offset for this chunk
		let offset = chunk_index as u64 * chunk_size as u64;

		// Seek to the correct position and write the chunk
		file.seek(std::io::SeekFrom::Start(offset))
			.await
			.map_err(|e| format!("Failed to seek in file: {}", e))?;
		file.write_all(data)
			.await
			.map_err(|e| format!("Failed to write chunk data: {}", e))?;
		file.flush()
			.await
			.map_err(|e| format!("Failed to flush file: {}", e))?;

		// Note: Using println for chunk writing as this is detailed debug info
		// that might be too verbose for standard logging

		Ok(())
	}

	/// Handle incoming transfer request
	async fn handle_incoming_transfer_request(
		&self,
		device_id: Uuid,
		transfer_id: Uuid,
		file_metadata: FileMetadata,
		destination_path: String,
	) -> Result<()> {
		self.logger
			.info(&format!(
				"Handling transfer request for file: {} ({} bytes) -> {}",
				file_metadata.name, file_metadata.size, destination_path
			))
			.await;

		// Create new transfer session
		let session = TransferSession {
			id: transfer_id,
			file_metadata: file_metadata.clone(),
			mode: TransferMode::TrustedCopy,
			state: TransferState::Pending,
			created_at: SystemTime::now(),
			bytes_transferred: 0,
			chunks_received: Vec::new(),
			source_device: Some(device_id),
			destination_device: None,
			destination_path: destination_path.clone(),
		};

		// Store session
		{
			let mut sessions = self.sessions.write().unwrap();
			sessions.insert(transfer_id, session);
		}

		// Accept the transfer (for trusted devices, auto-accept)
		self.update_session_state(&transfer_id, TransferState::Active)?;
		self.logger
			.info(&format!(
				"Auto-accepted transfer {} from trusted device {}",
				transfer_id, device_id
			))
			.await;

		Ok(())
	}

	/// Handle incoming file chunk
	async fn handle_incoming_file_chunk(
		&self,
		transfer_id: Uuid,
		chunk_index: u32,
		encrypted_data: Vec<u8>,
		nonce: [u8; 12],
		chunk_checksum: [u8; 32],
	) -> Result<()> {
		self.logger
			.debug(&format!(
				"Handling file chunk {} for transfer {}",
				chunk_index, transfer_id
			))
			.await;

		// Get the source device ID from the session
		let source_device_id = {
			let sessions = self.sessions.read().unwrap();
			if let Some(session) = sessions.get(&transfer_id) {
				session.source_device.ok_or_else(|| {
					NetworkingError::Protocol("No source device for transfer".to_string())
				})?
			} else {
				return Err(NetworkingError::Protocol(
					"Transfer session not found".to_string(),
				));
			}
		};

		// Get session keys for decryption
		let session_keys = if let Some(device_registry) = &self.device_registry {
			let registry = device_registry.read().await;
			registry.get_session_keys(source_device_id).ok_or_else(|| {
				NetworkingError::Protocol(format!(
					"No session keys for device {}",
					source_device_id
				))
			})?
		} else {
			return Err(NetworkingError::Protocol(
				"Device registry not available".to_string(),
			));
		};

		// Decrypt chunk data
		let chunk_data = self.decrypt_chunk(
			&session_keys.receive_key,
			&transfer_id,
			chunk_index,
			&encrypted_data,
			&nonce,
		)?;

		self.logger
			.debug(&format!(
				"Decrypted chunk {} ({} bytes -> {} bytes)",
				chunk_index,
				encrypted_data.len(),
				chunk_data.len()
			))
			.await;

		// Verify chunk checksum (of decrypted data)
		let calculated_checksum = blake3::hash(&chunk_data);
		if calculated_checksum.as_bytes() != &chunk_checksum {
			self.logger
				.error(&format!(
					"Chunk {} checksum mismatch after decryption",
					chunk_index
				))
				.await;
			return Err(NetworkingError::Protocol(format!(
				"Chunk {} checksum mismatch after decryption",
				chunk_index
			)));
		}

		self.logger
			.debug(&format!("Checksum verified for chunk {}", chunk_index))
			.await;

		// Write chunk to file
		if let Err(e) = self
			.write_chunk_to_file(&transfer_id, chunk_index, &chunk_data)
			.await
		{
			return Err(NetworkingError::Protocol(format!(
				"Failed to write chunk {}: {}",
				chunk_index, e
			)));
		}

		// Update session progress
		{
			let mut sessions = self.sessions.write().unwrap();
			if let Some(session) = sessions.get_mut(&transfer_id) {
				session.bytes_transferred += chunk_data.len() as u64;
				session.chunks_received.push(chunk_index);
				session.chunks_received.sort();
			}
		}

		self.logger
			.debug(&format!(
				"Successfully processed chunk {} for transfer {}",
				chunk_index, transfer_id
			))
			.await;
		Ok(())
	}

	/// Handle incoming transfer completion
	async fn handle_incoming_transfer_complete(
		&self,
		transfer_id: Uuid,
		final_checksum: String,
		total_bytes: u64,
	) -> Result<()> {
		let truncated_checksum = if final_checksum.len() > 16 {
			format!("{}...", &final_checksum[..16])
		} else {
			final_checksum.clone()
		};
		self.logger
			.info(&format!(
				"Handling transfer completion for transfer {} ({} bytes, checksum: {})",
				transfer_id, total_bytes, truncated_checksum
			))
			.await;

		// Mark transfer as completed
		self.update_session_state(&transfer_id, TransferState::Completed)?;

		// TODO: Verify final file checksum
		self.logger
			.info(&format!("Transfer {} completed successfully", transfer_id))
			.await;
		Ok(())
	}
}

#[async_trait]
impl super::ProtocolHandler for FileTransferProtocolHandler {
	fn protocol_name(&self) -> &str {
		"file_transfer"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		self.logger
			.debug(&format!(
				"FILE_TRANSFER: handle_stream called from node {}",
				remote_node_id
			))
			.await;

		// Read transfer type (1 byte)
		let mut transfer_type = [0u8; 1];
		if let Err(e) = recv.read_exact(&mut transfer_type).await {
			self.logger
				.error(&format!("Failed to read transfer type: {}", e))
				.await;
			return;
		}

		self.logger
			.debug(&format!(
				"FILE_TRANSFER: Received transfer type: {}",
				transfer_type[0]
			))
			.await;

		match transfer_type[0] {
			0 => {
				// File metadata request - this is now a message stream
				// Keep reading messages until stream closes or TransferComplete received
				// Note: The first type byte (0) was already read above
				let mut first_message = true;

				loop {
					// For messages after the first, read the type byte
					if !first_message {
						let mut msg_type = [0u8; 1];
						match recv.read_exact(&mut msg_type).await {
							Ok(_) => {
								if msg_type[0] != 0 {
									self.logger
										.error(&format!(
											"Unexpected message type in stream: {}",
											msg_type[0]
										))
										.await;
									break;
								}
							}
							Err(e) => {
								self.logger
									.debug(&format!("Stream ended or error reading type: {}", e))
									.await;
								break;
							}
						}
					}
					first_message = false;

					// Read message length
					let mut len_buf = [0u8; 4];
					match recv.read_exact(&mut len_buf).await {
						Ok(_) => {}
						Err(e) => {
							self.logger
								.error(&format!("Failed to read message length: {}", e))
								.await;
							break;
						}
					}
					let msg_len = u32::from_be_bytes(len_buf) as usize;

					// Read message
					let mut msg_buf = vec![0u8; msg_len];
					if let Err(e) = recv.read_exact(&mut msg_buf).await {
						self.logger
							.error(&format!("Failed to read message: {}", e))
							.await;
						break;
					}

					// Deserialize and handle
					if let Ok(message) = rmp_serde::from_slice::<FileTransferMessage>(&msg_buf) {
						self.logger
							.debug(&format!(
								"Received file transfer message: {}",
								Self::truncate_message_for_logging(&message)
							))
							.await;

						// Get device ID from node ID using device registry
						let device_id =
							if let Some(device_registry) = &self.device_registry {
								let registry = device_registry.read().await;
								registry
							.get_device_by_node(remote_node_id)
							.unwrap_or_else(|| {
								// Note: Can't use await in closure, this should be refactored
								eprintln!("Warning: Could not find device ID for node {}, using random ID", remote_node_id);
								uuid::Uuid::new_v4()
							})
							} else {
								// Note: Need to await this call properly
								eprintln!("Warning: Device registry not available, using random device ID");
								uuid::Uuid::new_v4()
							};

						// Process the message based on type
						match message {
							FileTransferMessage::TransferRequest {
								transfer_id,
								file_metadata,
								destination_path,
								..
							} => {
								// Handle transfer request
								if let Err(e) = self
									.handle_incoming_transfer_request(
										device_id,
										transfer_id,
										file_metadata,
										destination_path,
									)
									.await
								{
									self.logger
										.error(&format!("Failed to handle transfer request: {}", e))
										.await;
								}
							}
							FileTransferMessage::FileChunk {
								transfer_id,
								chunk_index,
								data,
								nonce,
								chunk_checksum,
							} => {
								// Handle file chunk
								if let Err(e) = self
									.handle_incoming_file_chunk(
										transfer_id,
										chunk_index,
										data,
										nonce,
										chunk_checksum,
									)
									.await
								{
									self.logger
										.error(&format!("Failed to handle file chunk: {}", e))
										.await;
								}
							}
							FileTransferMessage::TransferComplete {
								transfer_id,
								final_checksum,
								total_bytes,
							} => {
								// Handle transfer completion
								if let Err(e) = self
									.handle_incoming_transfer_complete(
										transfer_id,
										final_checksum.clone(),
										total_bytes,
									)
									.await
								{
									self.logger
										.error(&format!(
											"Failed to handle transfer completion: {}",
											e
										))
										.await;
								} else {
									// Send TransferFinalAck response back to sender
									self.logger
										.info(&format!(
											"Sending TransferFinalAck for transfer {}",
											transfer_id
										))
										.await;

									let ack_message =
										FileTransferMessage::TransferFinalAck { transfer_id };
									if let Ok(ack_data) = rmp_serde::to_vec(&ack_message) {
										// Send type (0) + length + data
										let _ = send.write_u8(0).await;
										let _ = send
											.write_all(&(ack_data.len() as u32).to_be_bytes())
											.await;
										let _ = send.write_all(&ack_data).await;
										let _ = send.flush().await;

										self.logger
											.info(&format!(
												"TransferFinalAck sent for transfer {}",
												transfer_id
											))
											.await;
									}
								}
							}
							_ => {
								self.logger
									.warn("Received unexpected file transfer message type")
									.await;
							}
						}
					} // Close the if let Ok(message)
				} // Close the loop
			}
			1 => {
				// File data stream
				// This would be a raw file transfer
				// For now, just read and discard
				let mut buffer = vec![0u8; 8192];
				while let Ok(n) = recv.read(&mut buffer).await {
					if n == 0 {
						break;
					}
					// Process file data chunk
				}
			}
			_ => {
				self.logger
					.error(&format!("Unknown transfer type: {}", transfer_type[0]))
					.await;
			}
		}
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		// Deserialize the request
		let request: FileTransferMessage = rmp_serde::from_slice(&request_data).map_err(|e| {
			NetworkingError::Protocol(format!("Failed to deserialize request: {}", e))
		})?;

		let response = match request {
			FileTransferMessage::TransferRequest { .. } => {
				self.handle_transfer_request(from_device, request).await?
			}
			FileTransferMessage::FileChunk { .. } => {
				self.handle_file_chunk(from_device, request).await?
			}
			FileTransferMessage::TransferComplete { .. } => {
				self.handle_transfer_complete(from_device, request).await?
			}
			_ => {
				return Err(NetworkingError::Protocol(
					"Unsupported request message type".to_string(),
				));
			}
		};

		// Serialize the response
		rmp_serde::to_vec(&response)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to serialize response: {}", e)))
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		_from_node: NodeId,
		response_data: Vec<u8>,
	) -> Result<()> {
		// Deserialize the response
		let response: FileTransferMessage = rmp_serde::from_slice(&response_data).map_err(|e| {
			NetworkingError::Protocol(format!("Failed to deserialize response: {}", e))
		})?;

		match response {
			FileTransferMessage::TransferResponse {
				transfer_id,
				accepted,
				reason,
				..
			} => {
				if accepted {
					self.update_session_state(&transfer_id, TransferState::Active)?;
					self.logger
						.info(&format!(
							"Transfer {} accepted by device {}",
							transfer_id, from_device
						))
						.await;
				} else {
					let reason = reason.unwrap_or_else(|| "No reason given".to_string());
					self.update_session_state(&transfer_id, TransferState::Failed(reason.clone()))?;
					self.logger
						.warn(&format!(
							"Transfer {} rejected by device {}: {}",
							transfer_id, from_device, reason
						))
						.await;
				}
			}
			FileTransferMessage::ChunkAck {
				transfer_id,
				chunk_index,
				next_expected,
			} => {
				self.logger
					.debug(&format!(
						"Chunk {} acknowledged for transfer {}, next expected: {}",
						chunk_index, transfer_id, next_expected
					))
					.await;
				// TODO: Continue sending next chunks
			}
			FileTransferMessage::TransferError {
				transfer_id,
				error_type,
				message,
				..
			} => {
				self.update_session_state(&transfer_id, TransferState::Failed(message.clone()))?;
				self.logger
					.error(&format!(
						"Transfer {} error: {:?} - {}",
						transfer_id, error_type, message
					))
					.await;
			}
			FileTransferMessage::TransferFinalAck { transfer_id } => {
				self.logger
					.info(&format!(
						"Transfer {} fully acknowledged by receiver",
						transfer_id
					))
					.await;
				// The sender can now consider the transfer fully and cleanly closed
			}
			_ => {
				return Err(NetworkingError::Protocol(
					"Unsupported response message type".to_string(),
				));
			}
		}

		Ok(())
	}

	async fn handle_event(&self, event: super::ProtocolEvent) -> Result<()> {
		match event {
			super::ProtocolEvent::DeviceConnected { device_id } => {
				self.logger
					.info(&format!(
						"Device {} connected - file transfer available",
						device_id
					))
					.await;
			}
			super::ProtocolEvent::DeviceDisconnected { device_id } => {
				self.logger
					.info(&format!(
						"Device {} disconnected - pausing active transfers",
						device_id
					))
					.await;
				// TODO: Pause transfers to this device
			}
			super::ProtocolEvent::ConnectionFailed { device_id, reason } => {
				self.logger
					.warn(&format!(
						"Connection to device {} failed: {} - cancelling transfers",
						device_id, reason
					))
					.await;
				// TODO: Cancel transfers to this device
			}
			_ => {}
		}
		Ok(())
	}
}

/// Error extensions for file transfer
impl NetworkingError {
	pub fn transfer_not_found(transfer_id: Uuid) -> Self {
		Self::Protocol(format!("Transfer not found: {}", transfer_id))
	}

	pub fn file_system(message: String) -> Self {
		Self::Protocol(format!("File system error: {}", message))
	}
}

// Custom error variants for file transfer
impl NetworkingError {
	pub fn transfer_not_found_error(transfer_id: Uuid) -> Self {
		Self::Protocol(format!("Transfer not found: {}", transfer_id))
	}

	pub fn file_system_error(message: String) -> Self {
		Self::Protocol(format!("File system error: {}", message))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::network::protocol::ProtocolHandler;
	use crate::service::network::utils::logging::SilentLogger;

	#[tokio::test]
	async fn test_file_transfer_handler_creation() {
		let logger = Arc::new(SilentLogger);
		let handler = FileTransferProtocolHandler::new_default(logger);
		assert_eq!(handler.protocol_name(), "file_transfer");
		assert!(handler.get_active_transfers().is_empty());
	}

	#[tokio::test]
	async fn test_transfer_session_lifecycle() {
		let logger = Arc::new(SilentLogger);
		let handler = FileTransferProtocolHandler::new_default(logger);
		let transfer_id = Uuid::new_v4();

		// Initially no session
		assert!(handler.get_session(&transfer_id).is_none());

		// Update state should fail for non-existent session
		assert!(handler
			.update_session_state(&transfer_id, TransferState::Active)
			.is_err());
	}
}
