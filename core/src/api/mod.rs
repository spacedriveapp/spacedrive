use rspc::{Config, Type};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{node::NodeConfig, Node};

use utils::{InvalidRequests, InvalidateOperationEvent};

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;
pub(crate) type RouterBuilder = rspc::RouterBuilder<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { cas_id: String },
	InvalidateOperation(InvalidateOperationEvent),
}

mod files;
mod jobs;
mod keys;
mod libraries;
mod locations;
mod nodes;
mod p2p;
mod sync;
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
		.query("buildInfo", |t| {
			#[derive(Serialize, Type)]
			pub struct BuildInfo {
				version: &'static str,
				commit: &'static str,
			}

			t(|_, _: ()| BuildInfo {
				version: env!("CARGO_PKG_VERSION"),
				commit: env!("GIT_HASH"),
			})
		})
		.query("nodeState", |t| {
			t(|ctx, _: ()| async move {
				Ok(NodeState {
					config: ctx.config.get().await,
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: ctx
						.config
						.data_directory()
						.to_str()
						.expect("Found non-UTF-8 path")
						.to_string(),
				})
			})
		})
		.yolo_merge("library.", libraries::mount())
		.yolo_merge("volumes.", volumes::mount())
		.yolo_merge("tags.", tags::mount())
		.yolo_merge("keys.", keys::mount())
		.yolo_merge("locations.", locations::mount())
		.yolo_merge("files.", files::mount())
		.yolo_merge("jobs.", jobs::mount())
		.yolo_merge("p2p.", p2p::mount())
		.yolo_merge("sync.", sync::mount())
		.yolo_merge("invalidation.", utils::mount_invalidate())
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
