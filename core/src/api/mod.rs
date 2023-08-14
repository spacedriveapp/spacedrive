use crate::{job::JobProgressEvent, node::config::NodeConfig, Node};
use rspc::{alpha::Rspc, Config};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

use utils::{InvalidRequests, InvalidateOperationEvent};

#[allow(non_upper_case_globals)]
pub(crate) const R: Rspc<Ctx> = Rspc::new();

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { thumb_key: Vec<String> },
	JobProgress(JobProgressEvent),
	InvalidateOperation(InvalidateOperationEvent),
}

mod categories;
mod files;
mod jobs;
mod keys;
mod libraries;
pub mod locations;
mod nodes;
pub mod notifications;
mod p2p;
mod preferences;
mod search;
mod sync;
mod tags;
pub mod utils;
pub mod volumes;

// A version of [NodeConfig] that is safe to share with the frontend
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct SanitisedNodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
	// TODO: These will probs be replaced by your Spacedrive account in the near future.
	pub p2p_email: Option<String>,
	pub p2p_img_url: Option<String>,
}

impl From<NodeConfig> for SanitisedNodeConfig {
	fn from(value: NodeConfig) -> Self {
		Self {
			id: value.id,
			name: value.name,
			p2p_port: value.p2p_port,
			p2p_email: value.p2p_email,
			p2p_img_url: value.p2p_img_url,
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: SanitisedNodeConfig,
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
			R.query(|node, _: ()| async move {
				Ok(NodeState {
					config: node.config.get().await.into(),
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: node
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
		// .merge("keys.", keys::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		.merge("p2p.", p2p::mount())
		.merge("nodes.", nodes::mount())
		.merge("sync.", sync::mount())
		.merge("preferences.", preferences::mount())
		.merge("notifications.", notifications::mount())
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
