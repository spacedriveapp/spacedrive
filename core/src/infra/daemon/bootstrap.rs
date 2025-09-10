use std::path::PathBuf;
use std::sync::Arc;

use crate::infra::daemon::{
	dispatch::DispatchRegistry, instance::CoreInstanceManager, rpc::RpcServer,
	state::SessionStateService,
};

/// Handlers are registered here to keep the core daemon transport generic.
pub async fn register_default_handlers(registry: Arc<DispatchRegistry>) {
	crate::ops::transport::register_handlers(registry).await;
}

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
	let instances = Arc::new(CoreInstanceManager::new(data_dir.clone()));
	let session = Arc::new(SessionStateService::new(data_dir));
	let registry = DispatchRegistry::new();
	register_default_handlers(registry.clone()).await;
	let server = RpcServer::new(socket_path, instances, session, registry);
	server.start().await
}
