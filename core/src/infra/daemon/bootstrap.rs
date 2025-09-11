use std::path::PathBuf;
use std::sync::Arc;

use crate::infra::daemon::{
	instance::CoreInstanceManager, rpc::RpcServer, state::SessionStateService,
};

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
	let instances = Arc::new(CoreInstanceManager::new(data_dir.clone()));
	let session = Arc::new(SessionStateService::new(data_dir));
	let server = RpcServer::new(socket_path, instances, session);
	server.start().await
}
