use crate::{invalidate_query, job::JobProgressEvent, node::config::NodeConfig, Node};
use itertools::Itertools;
use rspc::{alpha::Rspc, Config, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{atomic::Ordering, Arc};
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

/// All of the feature flags provided by the core itself. The frontend has it's own set of feature flags!
///
/// If you want a variant of this to show up on the frontend it must be added to `backendFeatures` in `useFeatureFlag.tsx`
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BackendFeature {
	SyncEmitMessages,
	FilesOverP2P,
}

impl BackendFeature {
	pub fn restore(&self, node: &Node) {
		match self {
			BackendFeature::SyncEmitMessages => {
				node.libraries
					.emit_messages_flag
					.store(true, Ordering::Relaxed);
			}
			BackendFeature::FilesOverP2P => {
				node.files_over_p2p_flag.store(true, Ordering::Relaxed);
			}
		}
	}
}

mod api;
mod auth;
mod backups;
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
pub(crate) mod search;
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
	pub features: Vec<BackendFeature>,
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
			features: value.features,
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
		.procedure("toggleFeatureFlag", {
			R.mutation(|node, feature: BackendFeature| async move {
				let config = node.config.get().await;

				let enabled = if config.features.iter().contains(&feature) {
					node.config
						.write(|mut cfg| {
							cfg.features.retain(|f| *f != feature);
						})
						.await
						.map(|_| false)
				} else {
					node.config
						.write(|mut cfg| {
							cfg.features.push(feature.clone());
						})
						.await
						.map(|_| true)
				}
				.map_err(|err| rspc::Error::new(ErrorCode::InternalServerError, err.to_string()))?;

				match feature {
					BackendFeature::SyncEmitMessages => {
						node.libraries
							.emit_messages_flag
							.store(enabled, Ordering::Relaxed);
					}
					BackendFeature::FilesOverP2P => {
						node.files_over_p2p_flag.store(enabled, Ordering::Relaxed);
					}
				}

				invalidate_query!(node; node, "nodeState");

				Ok(())
			})
		})
		.merge("api.", api::mount())
		.merge("auth.", auth::mount())
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
		.merge("backups.", backups::mount())
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
