//! Transport registration for ops: wires actions/queries to the daemon dispatch

use std::sync::Arc;

use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use futures::future::BoxFuture;

use crate::infra::action::builder::ActionBuilder;
use crate::infra::daemon::dispatch::{ActionHandler, DispatchRegistry, QueryHandler};
use crate::infra::daemon::state::SessionState;
use crate::infra::daemon::types::type_ids;

/// Register all ops handlers with the daemon dispatch registry
pub async fn register_handlers(registry: Arc<DispatchRegistry>) {
	// FileCopyInput
	let copy_handler: ActionHandler = Arc::new(
		|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
			Box::pin(async move {
				let mut input: crate::ops::files::copy::input::FileCopyInput =
					decode_from_slice(&payload, standard())
						.map_err(|e| format!("deserialize: {}", e))?
						.0;
				if input.library_id.is_none() {
					input.library_id = session.current_library_id;
				}
				let action =
					crate::ops::files::copy::action::FileCopyActionBuilder::from_input(input)
						.build()
						.map_err(|e| e.to_string())?;
				core.execute_library_action(action)
					.await
					.map_err(|e| e.to_string())?;
				Ok(Vec::new())
			})
		},
	);
	registry
		.register_action(type_ids::FILE_COPY_INPUT, copy_handler)
		.await;

	// IndexInput
	let index_handler: ActionHandler = Arc::new(
		|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
			Box::pin(async move {
				let mut input: crate::ops::indexing::IndexInput =
					decode_from_slice(&payload, standard())
						.map_err(|e| format!("deserialize: {}", e))?
						.0;
				if input.library_id.is_nil() {
					if let Some(id) = session.current_library_id {
						input.library_id = id;
					}
				}
				let action = crate::ops::indexing::IndexingAction::new(input);
				core.execute_library_action(action)
					.await
					.map_err(|e| e.to_string())?;
				Ok(Vec::new())
			})
		},
	);
	registry
		.register_action(type_ids::INDEX_INPUT, index_handler)
		.await;

	// LocationRescanAction
	let rescan_handler: ActionHandler = Arc::new(
		|payload, core, session: SessionState| -> BoxFuture<'static, Result<Vec<u8>, String>> {
			Box::pin(async move {
				let mut action: crate::ops::locations::rescan::action::LocationRescanAction =
					decode_from_slice(&payload, standard())
						.map_err(|e| format!("deserialize: {}", e))?
						.0;
				if action.library_id.is_nil() {
					if let Some(id) = session.current_library_id {
						action.library_id = id;
					}
				}
				core.execute_library_action(action)
					.await
					.map_err(|e| e.to_string())?;
				Ok(Vec::new())
			})
		},
	);
	registry
		.register_action(type_ids::LOCATION_RESCAN_ACTION, rescan_handler)
		.await;

	// Queries
	let list_libs_q: QueryHandler = Arc::new(|payload, core, _session| {
		Box::pin(async move {
			let q: crate::ops::libraries::list::query::ListLibrariesQuery =
				decode_from_slice(&payload, standard())
					.map_err(|e| e.to_string())?
					.0;
			let out: Vec<crate::ops::libraries::list::output::LibraryInfo> =
				core.execute_query(q).await.map_err(|e| e.to_string())?;
			encode_to_vec(&out, standard()).map_err(|e| e.to_string())
		})
	});
	registry
		.register_query(type_ids::LIST_LIBRARIES_QUERY, list_libs_q)
		.await;

	let core_status_q: QueryHandler = Arc::new(|payload, core, _session| {
		Box::pin(async move {
			let _q: crate::ops::core::status::query::CoreStatusQuery =
				decode_from_slice(&payload, standard())
					.map_err(|e| e.to_string())?
					.0;
			let out: crate::ops::core::status::output::CoreStatus =
				core.execute_query(_q).await.map_err(|e| e.to_string())?;
			encode_to_vec(&out, standard()).map_err(|e| e.to_string())
		})
	});
	registry
		.register_query(type_ids::CORE_STATUS_QUERY, core_status_q)
		.await;
}
