use rspc::{alpha::Rspc, Config};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

use crate::{node::NodeConfig, Node};

use utils::{InvalidRequests, InvalidateOperationEvent};

#[allow(non_upper_case_globals)]
pub(self) const R: Rspc<Ctx> = Rspc::new();

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { cas_id: String },
	InvalidateOperation(InvalidateOperationEvent),
}

mod categories;
mod files;
mod jobs;
mod keys;
mod libraries;
mod locations;
mod nodes;
mod p2p;
mod search;
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
	let r = R
		.router()
		.procedure("buildInfo", {
			#[derive(Serialize, Type)]
			pub struct BuildInfo {
				version: &'static str,
				commit: &'static str,
			}

			R.query(|_, _: ()| BuildInfo {
				version: env!("CARGO_PKG_VERSION"),
				commit: env!("GIT_HASH"),
			})
		})
		.procedure("nodeState", {
			R.query(|ctx, _: ()| async move {
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
		.merge("search.", search::mount())
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("categories.", categories::mount())
		.merge("keys.", keys::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		.merge("p2p.", p2p::mount())
		.merge("nodes.", nodes::mount())
		.merge("sync.", sync::mount())
		.merge("invalidation.", utils::mount_invalidate())
		.build(
			#[allow(clippy::let_and_return)]
			{
				let config = Config::new().set_ts_bindings_header("/* eslint-disable */");

				#[cfg(all(debug_assertions, not(feature = "mobile")))]
				let config = config.export_ts_bindings(
					std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
						.join("../packages/client/src/core.ts"),
				);

				config
			},
		)
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
