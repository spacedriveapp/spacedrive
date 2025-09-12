use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use crate::infra::daemon::instance::CoreInstanceManager;
use crate::infra::daemon::state::SessionStateService;
use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};

/// Minimal JSON-over-UDS RPC server
pub struct RpcServer {
	socket_path: PathBuf,
	instances: Arc<CoreInstanceManager>,
	session: Arc<SessionStateService>,
}

impl RpcServer {
	pub fn new(
		socket_path: PathBuf,
		instances: Arc<CoreInstanceManager>,
		session: Arc<SessionStateService>,
	) -> Self {
		Self {
			socket_path,
			instances,
			session,
		}
	}

	pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
		if std::fs::remove_file(&self.socket_path).is_ok() {}
		if let Some(parent) = self.socket_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let listener = UnixListener::bind(&self.socket_path)?;
		loop {
			let (mut stream, _addr) = listener.accept().await?;
			let instances = self.instances.clone();
			let session = self.session.clone();
			// Handle connection without spawning to avoid Send requirements
			{
				let mut buf = Vec::new();
				if stream.read_to_end(&mut buf).await.is_err() {
					continue;
				}
				let req: Result<DaemonRequest, _> = serde_json::from_slice(&buf);
				let resp = match req {
					Ok(DaemonRequest::Ping) => DaemonResponse::Pong,
					Ok(DaemonRequest::Action { ref method, ref payload }) => {
						match instances.get_default().await {
							Ok(core) => {
								let session_snapshot = session.get().await;
								match core
									.execute_action_by_method(
										method,
										payload.clone(),
										session_snapshot.clone(),
									)
									.await
								{
									Ok(out) => DaemonResponse::Ok(out),
									Err(e) => DaemonResponse::Error(e),
								}
							}
							Err(_) => {
								DaemonResponse::Error("Failed to get core instance".to_string())
							}
						}
					}
					Ok(DaemonRequest::Query { ref method, ref payload }) => {
						match instances.get_default().await {
							Ok(core) => {
								// Pass-through for queries using opaque method string
								match core.execute_query_by_method(method, payload.clone()).await {
									Ok(out) => DaemonResponse::Ok(out),
									Err(e) => DaemonResponse::Error(e),
								}
							}
							Err(_) => {
								DaemonResponse::Error("Failed to get core instance".to_string())
							}
						}
					}
                    Ok(DaemonRequest::Shutdown) => {
                        // Handle shutdown request
                        DaemonResponse::Ok(Vec::new()) // Send an OK response
                    }
					Err(ref e) => DaemonResponse::Error(format!("Invalid request: {}", e)), // Use ref e here
				};
				let _ = stream
					.write_all(serde_json::to_string(&resp).unwrap().as_bytes())
					.await;
                if let Ok(ref req_inner) = req { // Use ref req_inner here
                    if let DaemonRequest::Shutdown = req_inner {
                        break Ok(()); // Break the loop to shut down the server
                    }
                }
			}
		}
	}
}
