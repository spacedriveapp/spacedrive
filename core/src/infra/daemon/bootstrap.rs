use std::path::PathBuf;
use std::sync::Arc;

use crate::infra::daemon::{
	instance::CoreInstanceManager, rpc::RpcServer, state::SessionStateService,
};

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
	enable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let session = Arc::new(SessionStateService::new(data_dir.clone()));
	let instances = Arc::new(CoreInstanceManager::new(
		data_dir.clone(),
		enable_networking,
		session.clone(),
	));
	let mut server = RpcServer::new(socket_path, instances, session);
	server.start().await
}
