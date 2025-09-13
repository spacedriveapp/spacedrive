use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::infra::daemon::instance::CoreInstanceManager;
use crate::infra::daemon::state::SessionStateService;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse};

/// Minimal JSON-over-UDS RPC server
pub struct RpcServer {
	socket_path: PathBuf,
	instances: Arc<CoreInstanceManager>,
	session: Arc<SessionStateService>,
	shutdown_tx: mpsc::Sender<()>,
	shutdown_rx: mpsc::Receiver<()>,
}

impl RpcServer {
	pub fn new(
		socket_path: PathBuf,
		instances: Arc<CoreInstanceManager>,
		session: Arc<SessionStateService>,
	) -> Self {
		let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
		Self {
			socket_path,
			instances,
			session,
			shutdown_tx,
			shutdown_rx,
		}
	}

	pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		if std::fs::remove_file(&self.socket_path).is_ok() {}
		if let Some(parent) = self.socket_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let listener = UnixListener::bind(&self.socket_path)?;

		loop {
			tokio::select! {
				// Handle new connections
				result = listener.accept() => {
					match result {
						Ok((stream, _addr)) => {
							let instances = self.instances.clone();
							let session = self.session.clone();
							let shutdown_tx = self.shutdown_tx.clone();

							// Spawn task for concurrent request handling
							tokio::spawn(async move {
								// Convert errors to strings to ensure Send
								if let Err(e) = Self::handle_connection(stream, instances, session, shutdown_tx).await {
									eprintln!("Connection error: {}", e);
								}
							});
						}
						Err(e) => {
							eprintln!("Accept error: {}", e);
							continue;
						}
					}
				}

				// Handle shutdown signal
				_ = self.shutdown_rx.recv() => {
					eprintln!("Shutdown signal received, stopping RPC server");
					break;
				}
			}
		}

		Ok(())
	}

	/// Handle individual client connection concurrently
	async fn handle_connection(
		mut stream: tokio::net::UnixStream,
		instances: Arc<CoreInstanceManager>,
		session: Arc<SessionStateService>,
		shutdown_tx: mpsc::Sender<()>,
	) -> Result<(), String> {
		// Request size limit (10MB)
		const MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;

		let mut buf = Vec::new();
		let mut total_read = 0;
		let mut chunk = [0u8; 4096];

		// Read request with size limit
		loop {
			let n = stream
				.read(&mut chunk)
				.await
				.map_err(|e| DaemonError::ReadError(e.to_string()).to_string())?;
			if n == 0 {
				// EOF - client closed connection
				return Ok(());
			}

			if total_read + n > MAX_REQUEST_SIZE {
				let resp = DaemonResponse::Error(DaemonError::RequestTooLarge(format!(
					"Request size {} exceeds maximum {}",
					total_read + n,
					MAX_REQUEST_SIZE
				)));
				let _ = stream
					.write_all(
						serde_json::to_string(&resp)
							.map_err(|e| {
								DaemonError::SerializationError(e.to_string()).to_string()
							})?
							.as_bytes(),
					)
					.await;
				return Ok(());
			}

			buf.extend_from_slice(&chunk[..n]);
			total_read += n;

			// Try to parse JSON - if successful, we have the complete request
			if let Ok(req) = serde_json::from_slice::<DaemonRequest>(&buf) {
				let resp = Self::process_request(req, &instances, &session, &shutdown_tx).await;

				// Send response
				let response_bytes = serde_json::to_string(&resp)
					.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;
				let _ = stream.write_all(response_bytes.as_bytes()).await;

				// Close stream
				let _ = stream.shutdown().await;
				return Ok(());
			}
		}
	}

	/// Process a parsed daemon request
	async fn process_request(
		request: DaemonRequest,
		instances: &Arc<CoreInstanceManager>,
		session: &Arc<SessionStateService>,
		shutdown_tx: &mpsc::Sender<()>,
	) -> DaemonResponse {
		match request {
			DaemonRequest::Ping => DaemonResponse::Pong,

			DaemonRequest::Action { method, payload } => match instances.get_default().await {
				Ok(core) => {
					let session_snapshot = session.get().await;
					match core
						.execute_action_by_method(&method, payload, session_snapshot)
						.await
					{
						Ok(out) => DaemonResponse::Ok(out),
						Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
					}
				}
				Err(e) => DaemonResponse::Error(DaemonError::CoreUnavailable(e)),
			},

			DaemonRequest::Query { method, payload } => match instances.get_default().await {
				Ok(core) => match core.execute_query_by_method(&method, payload).await {
					Ok(out) => DaemonResponse::Ok(out),
					Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
				},
				Err(e) => DaemonResponse::Error(DaemonError::CoreUnavailable(e)),
			},

			DaemonRequest::Shutdown => {
				// Signal shutdown to main loop
				let _ = shutdown_tx.send(()).await;
				DaemonResponse::Ok(Vec::new())
			}
		}
	}
}
