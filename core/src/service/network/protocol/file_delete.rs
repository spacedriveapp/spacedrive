//! File delete protocol for cross-device deletion operations

use crate::{
	ops::files::delete::{
		strategy::{DeleteStrategy, FileDeleteMessage, LocalDeleteStrategy},
		DeleteMode,
	},
	service::network::{NetworkingError, Result},
};
use async_trait::async_trait;
use iroh::NodeId;
use std::sync::Arc;
use uuid::Uuid;

/// File delete protocol handler
pub struct FileDeleteProtocolHandler {
	/// Optional context for accessing job system
	context: Option<Arc<crate::context::CoreContext>>,
}

impl FileDeleteProtocolHandler {
	/// Create a new file delete protocol handler
	pub fn new() -> Self {
		Self { context: None }
	}

	/// Create handler with context
	pub fn with_context(context: Arc<crate::context::CoreContext>) -> Self {
		Self {
			context: Some(context),
		}
	}

	/// Set context after creation
	pub fn set_context(&mut self, context: Arc<crate::context::CoreContext>) {
		self.context = Some(context);
	}

	/// Handle delete request from remote device
	async fn handle_delete_request(
		&self,
		_from_device: Uuid,
		request: FileDeleteMessage,
	) -> Result<FileDeleteMessage> {
		if let FileDeleteMessage::Request {
			paths,
			mode,
			request_id,
		} = request
		{
			// Get context to create a temporary job context
			let context = self.context.as_ref().ok_or_else(|| {
				NetworkingError::Protocol("Context not available for deletion".to_string())
			})?;

			// Create local delete strategy
			let strategy = LocalDeleteStrategy;

			// Execute deletion using the strategy
			// Note: We're creating a minimal job context for this operation
			// In a full implementation, this might integrate with the job system
			let results =
				match Self::execute_deletion_with_strategy(&strategy, &paths, mode.clone()).await {
					Ok(results) => results,
					Err(e) => {
						return Ok(FileDeleteMessage::Response {
							request_id,
							results: paths
								.iter()
								.map(|path| crate::ops::files::delete::strategy::DeleteResult {
									path: path.clone(),
									success: false,
									bytes_freed: 0,
									error: Some(format!("Strategy execution failed: {}", e)),
								})
								.collect(),
						});
					}
				};

			Ok(FileDeleteMessage::Response {
				request_id,
				results,
			})
		} else {
			Err(NetworkingError::Protocol(
				"Invalid delete request message".to_string(),
			))
		}
	}

	/// Execute deletion with strategy (simplified without full job context)
	async fn execute_deletion_with_strategy(
		strategy: &LocalDeleteStrategy,
		paths: &[crate::domain::addressing::SdPath],
		mode: DeleteMode,
	) -> anyhow::Result<Vec<crate::ops::files::delete::strategy::DeleteResult>> {
		// Create a minimal execution context
		// Note: In production, this should properly integrate with the job system
		// For now, we're executing directly

		let mut results = Vec::new();

		for path in paths {
			let local_path = path
				.as_local_path()
				.ok_or_else(|| anyhow::anyhow!("Path is not local"))?;

			// Get file size before deletion
			let size = tokio::fs::metadata(local_path)
				.await
				.map(|m| {
					if m.is_file() {
						m.len()
					} else {
						0 // Directory size calculation would be more complex
					}
				})
				.unwrap_or(0);

			// Perform deletion based on mode
			let result = match mode {
				DeleteMode::Trash => LocalDeleteStrategy.move_to_trash(local_path).await,
				DeleteMode::Permanent => LocalDeleteStrategy.permanent_delete(local_path).await,
				DeleteMode::Secure => LocalDeleteStrategy.secure_delete(local_path).await,
			};

			results.push(crate::ops::files::delete::strategy::DeleteResult {
				path: path.clone(),
				success: result.is_ok(),
				bytes_freed: if result.is_ok() { size } else { 0 },
				error: result.err().map(|e| e.to_string()),
			});
		}

		Ok(results)
	}
}

impl Default for FileDeleteProtocolHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl super::ProtocolHandler for FileDeleteProtocolHandler {
	fn protocol_name(&self) -> &str {
		"file_delete"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		_remote_node_id: NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		// Simple request-response over streams
		loop {
			// Read message length (4 bytes)
			let mut len_buf = [0u8; 4];
			match recv.read_exact(&mut len_buf).await {
				Ok(_) => {}
				Err(_) => break, // Connection closed
			}
			let msg_len = u32::from_be_bytes(len_buf) as usize;

			// Read message
			let mut msg_buf = vec![0u8; msg_len];
			if let Err(e) = recv.read_exact(&mut msg_buf).await {
				eprintln!("Failed to read delete message: {}", e);
				break;
			}

			// Deserialize and handle
			match rmp_serde::from_slice::<FileDeleteMessage>(&msg_buf) {
				Ok(message) => {
					// Get device ID (simplified - in production would resolve from node_id)
					let device_id = Uuid::nil();

					// Handle the delete request
					let response = match self.handle_delete_request(device_id, message).await {
						Ok(resp) => resp,
						Err(e) => {
							eprintln!("Failed to handle delete request: {}", e);
							continue;
						}
					};

					// Serialize response
					match rmp_serde::to_vec(&response) {
						Ok(response_data) => {
							// Send response
							let len = response_data.len() as u32;
							if send.write_all(&len.to_be_bytes()).await.is_err() {
								break;
							}
							if send.write_all(&response_data).await.is_err() {
								break;
							}
							let _ = send.flush().await;
						}
						Err(e) => {
							eprintln!("Failed to serialize delete response: {}", e);
							break;
						}
					}
				}
				Err(e) => {
					eprintln!("Failed to deserialize delete message: {}", e);
					break;
				}
			}
		}
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		let message: FileDeleteMessage = rmp_serde::from_slice(&request_data)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to deserialize: {}", e)))?;

		let response = self.handle_delete_request(from_device, message).await?;

		rmp_serde::to_vec(&response)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to serialize: {}", e)))
	}

	async fn handle_response(
		&self,
		_from_device: Uuid,
		_from_node: NodeId,
		_response_data: Vec<u8>,
	) -> Result<()> {
		// File delete responses are handled by RemoteDeleteStrategy
		Ok(())
	}

	async fn handle_event(&self, _event: super::ProtocolEvent) -> Result<()> {
		// File delete doesn't need special event handling
		Ok(())
	}
}
