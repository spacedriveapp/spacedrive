use std::path::PathBuf;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::infra::daemon::{
    dispatch::{ActionHandler, DispatchRegistry},
    instance::CoreInstanceManager,
    rpc::RpcServer,
    state::{SessionState, SessionStateService},
};
use crate::infra::daemon::types::type_ids;
use crate::infra::daemon::dispatch::QueryHandler;

// Handlers are registered here to keep the core daemon transport generic.
pub async fn register_default_handlers(registry: Arc<DispatchRegistry>) {
	// FileCopyInput
	let copy_handler: ActionHandler = Arc::new(|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
		Box::pin(async move {
			let mut input: crate::ops::files::copy::input::FileCopyInput = bincode::deserialize(&payload)
				.map_err(|e| format!("deserialize: {}", e))?;
			if input.library_id.is_none() { input.library_id = session.current_library_id; }
			let action = crate::ops::files::copy::action::FileCopyActionBuilder::from_input(input)
				.build()
				.map_err(|e| e.to_string())?;
			core.execute_library_action(action).await.map_err(|e| e.to_string())?;
			Ok(Vec::new())
		})
	});
	registry.register_action(super::types::type_ids::FILE_COPY_INPUT, copy_handler).await;

	// IndexInput
	let index_handler: ActionHandler = Arc::new(|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
		Box::pin(async move {
			let mut input: crate::ops::indexing::IndexInput = bincode::deserialize(&payload)
				.map_err(|e| format!("deserialize: {}", e))?;
			if input.library_id.is_nil() { if let Some(id) = session.current_library_id { input.library_id = id; } }
			let action = crate::ops::indexing::IndexingAction::new(input);
			core.execute_library_action(action).await.map_err(|e| e.to_string())?;
			Ok(Vec::new())
		})
	});
	registry.register_action(super::types::type_ids::INDEX_INPUT, index_handler).await;

	// LocationRescanAction
	let rescan_handler: ActionHandler = Arc::new(|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
		Box::pin(async move {
			let mut action: crate::ops::locations::rescan::action::LocationRescanAction = bincode::deserialize(&payload)
				.map_err(|e| format!("deserialize: {}", e))?;
			if action.library_id.is_nil() { if let Some(id) = session.current_library_id { action.library_id = id; } }
			core.execute_library_action(action).await.map_err(|e| e.to_string())?;
			Ok(Vec::new())
		})
	});
	registry.register_action(super::types::type_ids::LOCATION_RESCAN_ACTION, rescan_handler).await;

	// Queries
	let list_libs_q: QueryHandler = Arc::new(|payload, core, _session| Box::pin(async move {
		let q: crate::ops::libraries::list::query::ListLibrariesQuery = bincode::deserialize(&payload).map_err(|e| e.to_string())?;
		let out: Vec<crate::ops::libraries::list::output::LibraryInfo> = core.execute_query(q).await.map_err(|e| e.to_string())?;
		bincode::serialize(&out).map_err(|e| e.to_string())
	}));
	registry.register_query(type_ids::LIST_LIBRARIES_QUERY, list_libs_q).await;

	let core_status_q: QueryHandler = Arc::new(|payload, core, _session| Box::pin(async move {
		let _q: crate::ops::core::status::query::CoreStatusQuery = bincode::deserialize(&payload).map_err(|e| e.to_string())?;
		let out: crate::ops::core::status::output::CoreStatus = core.execute_query(_q).await.map_err(|e| e.to_string())?;
		bincode::serialize(&out).map_err(|e| e.to_string())
	}));
	registry.register_query(type_ids::CORE_STATUS_QUERY, core_status_q).await;
}

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(socket_path: PathBuf, data_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let instances = Arc::new(CoreInstanceManager::new(data_dir.clone()));
	let session = Arc::new(SessionStateService::new(data_dir));
	let registry = DispatchRegistry::new();
	register_default_handlers(registry.clone()).await;
	let server = RpcServer::new(socket_path, instances, session, registry);
	server.start().await
}


