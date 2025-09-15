use std::path::PathBuf;
use std::sync::Arc;

use crate::infra::daemon::{
	instance::CoreInstanceManager, rpc::RpcServer,
};

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
	enable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let instances = Arc::new(CoreInstanceManager::new(
		data_dir.clone(),
		enable_networking,
	));
	let mut server = RpcServer::new(socket_path, instances);
	server.start().await
}
