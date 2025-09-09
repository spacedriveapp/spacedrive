use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};
use crate::infra::daemon::instance::CoreInstanceManager;
use crate::infra::daemon::state::SessionStateService;
use crate::infra::daemon::dispatch::DispatchRegistry;

/// Minimal JSON-over-UDS RPC server
pub struct RpcServer {
	socket_path: PathBuf,
	instances: Arc<CoreInstanceManager>,
	session: Arc<SessionStateService>,
	registry: Arc<DispatchRegistry>,
}

impl RpcServer {
	pub fn new(socket_path: PathBuf, instances: Arc<CoreInstanceManager>, session: Arc<SessionStateService>, registry: Arc<DispatchRegistry>) -> Self {
		Self { socket_path, instances, session, registry }
	}

	pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
		if std::fs::remove_file(&self.socket_path).is_ok() {}
		if let Some(parent) = self.socket_path.parent() { std::fs::create_dir_all(parent)?; }
		let listener = UnixListener::bind(&self.socket_path)?;
		loop {
			let (mut stream, _addr) = listener.accept().await?;
			let instances = self.instances.clone();
			let session = self.session.clone();
			let registry = self.registry.clone();
			tokio::spawn(async move {
				let mut buf = Vec::new();
				if stream.read_to_end(&mut buf).await.is_err() { return; }
				let req: Result<DaemonRequest, _> = serde_json::from_slice(&buf);
				let resp = match req {
					Ok(DaemonRequest::Ping) => DaemonResponse::Pong,
					Ok(DaemonRequest::Action { type_id, payload }) => {
						let core = match instances.get_default().await { Ok(c) => c, Err(_) => return };
						let session_snapshot = session.get().await;
						match registry.dispatch_action(&type_id, payload, core, session_snapshot).await {
							Ok(out) => DaemonResponse::Ok(out),
							Err(e) => DaemonResponse::Error(e),
						}
					}
					Ok(DaemonRequest::Query { type_id, payload }) => {
						let core = match instances.get_default().await { Ok(c) => c, Err(_) => return };
						let session_snapshot = session.get().await;
						match registry.dispatch_query(&type_id, payload, core, session_snapshot).await {
							Ok(out) => DaemonResponse::Ok(out),
							Err(e) => DaemonResponse::Error(e),
						}
					}
					Err(e) => DaemonResponse::Error(format!("Invalid request: {}", e)),
				};
				let _ = stream.write_all(serde_json::to_string(&resp).unwrap().as_bytes()).await;
			});
		}
	}
}


