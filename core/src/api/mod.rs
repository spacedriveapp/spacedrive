use std::{
	sync::Arc,
	time::{Duration, Instant},
};

use rspc::{Config, Type};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{
	job::JobManager,
	library::LibraryManager,
	node::{NodeConfig, NodeConfigManager},
};

use utils::{InvalidRequests, InvalidateOperationEvent};

pub type Router = rspc::Router<Ctx>;
pub(crate) type RouterBuilder = rspc::RouterBuilder<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { cas_id: String },
	InvalidateOperation(InvalidateOperationEvent),
	InvalidateOperationDebounced(InvalidateOperationEvent),
}

/// Is provided when executing the router from the request.
pub struct Ctx {
	pub library_manager: Arc<LibraryManager>,
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
	pub event_bus: broadcast::Sender<CoreEvent>,
}

mod files;
mod jobs;
mod libraries;
mod locations;
mod normi;
mod tags;
pub mod utils;
pub mod volumes;

#[derive(Serialize, Deserialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: NodeConfig,
	data_path: String,
}

pub(crate) fn mount() -> Arc<Router> {
	let config = Config::new().set_ts_bindings_header("/* eslint-disable */");

	#[cfg(all(debug_assertions, not(feature = "mobile")))]
	let config = config.export_ts_bindings(
		std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packages/client/src/core.ts"),
	);

	let r = <Router>::new()
		.config(config)
		.query("version", |t| t(|_, _: ()| env!("CARGO_PKG_VERSION")))
		.query("getNode", |t| {
			t(|ctx, _: ()| async move {
				Ok(NodeState {
					config: ctx.config.get().await,
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: ctx.config.data_directory().to_string_lossy().into_owned(),
				})
			})
		})
		.merge("normi.", normi::mount())
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		// TODO: Scope the invalidate queries to a specific library (filtered server side)
		.subscription("invalidateQuery", |t| {
			t(|ctx, _: ()| {
				let mut event_bus_rx = ctx.event_bus.subscribe();
				let mut last = Instant::now();
				async_stream::stream! {
					while let Ok(event) = event_bus_rx.recv().await {
						match event {
							CoreEvent::InvalidateOperation(op) => yield op,
							CoreEvent::InvalidateOperationDebounced(op) => {
								let current = Instant::now();
								if current.duration_since(last) > Duration::from_millis(1000 / 60) {
									last = current;
									yield op;
								}
							},
							_ => {}
						}
					}
				}
			})
		})
		.build()
		.arced();
	InvalidRequests::validate(r.clone()); // This validates all invalidation calls.

	r
}

#[cfg(test)]
mod tests {
	/// This test will ensure the rspc router and all calls to `invalidate_query` are valid and also export an updated version of the Typescript bindings.
	#[test]
	fn test_and_export_rspc_bindings() {
		super::mount();
	}
}
