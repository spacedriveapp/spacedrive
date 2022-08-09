use std::{
	sync::Arc,
	time::{Duration, Instant},
};

use rspc::{Config, ErrorCode, Type};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
	job::JobManager,
	library::{LibraryContext, LibraryManager},
	node::{NodeConfig, NodeConfigManager},
};

use utils::{InvalidRequests, InvalidateOperationEvent};

pub(crate) type Router = rspc::Router<Ctx>;
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

/// Can wrap a query argument to require it to contain a `library_id` and provide helpers for working with libraries.
#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LibraryArgs<T> {
	// If you want to make these public, your doing it wrong.
	pub library_id: Uuid,
	pub arg: T,
}

impl<T> LibraryArgs<T> {
	pub async fn get_library(self, ctx: &Ctx) -> Result<(T, LibraryContext), rspc::Error> {
		match ctx.library_manager.get_ctx(self.library_id).await {
			Some(library) => Ok((self.arg, library)),
			None => Err(rspc::Error::new(
				ErrorCode::BadRequest,
				"You must specify a valid library to use this operation.".to_string(),
			)),
		}
	}
}

mod files;
mod jobs;
mod libraries;
mod locations;
mod tags;
pub mod utils;
mod volumes;

pub use files::*;
pub use jobs::*;
pub use libraries::*;
pub use tags::*;

#[derive(Serialize, Deserialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: NodeConfig,
	data_path: String,
}

pub(crate) fn mount() -> Arc<Router> {
	let r = <Router>::new()
		.config(
			Config::new()
				// TODO: This messes with Tauri's hot reload so we can't use it until their is a solution upstream. https://github.com/tauri-apps/tauri/issues/4617
				// .export_ts_bindings(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./index.ts")),
				.set_ts_bindings_header("/* eslint-disable */"),
		)
		.query("version", |_, _: ()| env!("CARGO_PKG_VERSION"))
		.query("getNode", |ctx, _: ()| async move {
			Ok(NodeState {
				config: ctx.config.get().await,
				// We are taking the assumption here that this value is only used on the frontend for display purposes
				data_path: ctx.config.data_directory().to_string_lossy().into_owned(),
			})
		})
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		// TODO: Scope the invalidate queries to a specific library (filtered server side)
		.subscription("invalidateQuery", |ctx, _: ()| {
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
		.build()
		.arced();
	InvalidRequests::validate(r.clone()); // This validates all invalidation calls.
	r
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	/// This test will ensure the rspc router and all calls to `invalidate_query` are valid and also export an updated version of the Typescript bindings.
	#[test]
	fn test_and_export_rspc_bindings() {
		let r = super::mount();
		r.export_ts(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./index.ts"))
			.expect("Error exporting rspc Typescript bindings!");
	}
}
