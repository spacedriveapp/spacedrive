use crate::{
	invalidate_query,
	node::{
		config::{DeletePreferences, DeletePromptOptions, NodeConfig, NodeConfigP2P, NodePreferences},
		get_hardware_model_name, HardwareModel,
	},
	old_job::JobProgressEvent,
	Node,
};

use sd_p2p::RemoteIdentity;
use sd_prisma::prisma::file_path;

use std::sync::{atomic::Ordering, Arc};

use itertools::Itertools;
use rspc::{alpha::Rspc, Config, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

mod auth;
mod backups;
mod cloud;
// mod categories;
mod ephemeral_files;
mod files;
mod jobs;
mod keys;
mod labels;
mod libraries;
pub mod locations;
mod models;
mod nodes;
pub mod notifications;
mod p2p;
mod preferences;
pub(crate) mod search;
mod sync;
mod tags;
pub mod utils;
pub mod volumes;
mod web_api;

use utils::{InvalidRequests, InvalidateOperationEvent};

#[allow(non_upper_case_globals)]
pub(crate) const R: Rspc<Ctx> = Rspc::new();

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail {
		thumb_key: Vec<String>,
	},
	NewIdentifiedObjects {
		file_path_ids: Vec<file_path::id::Type>,
	},
	JobProgress(JobProgressEvent),
	InvalidateOperation(InvalidateOperationEvent),
}

/// All of the feature flags provided by the core itself. The frontend has it's own set of feature flags!
///
/// If you want a variant of this to show up on the frontend it must be added to `backendFeatures` in `useFeatureFlag.tsx`
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BackendFeature {
	CloudSync,
}

impl BackendFeature {
	pub fn restore(&self, node: &Node) {
		match self {
			BackendFeature::CloudSync => {
				node.cloud_sync_flag.store(true, Ordering::Relaxed);
			}
		}
	}
}

// A version of [NodeConfig] that is safe to share with the frontend
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct SanitisedNodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	pub identity: RemoteIdentity,
	pub p2p: NodeConfigP2P,
	pub features: Vec<BackendFeature>,
	pub preferences: NodePreferences,
	pub image_labeler_version: Option<String>,
	pub delete_prompt: DeletePreferences,
}

impl From<NodeConfig> for SanitisedNodeConfig {
	fn from(value: NodeConfig) -> Self {
		Self {
			id: value.id,
			name: value.name,
			identity: value.identity.to_remote_identity(),
			p2p: value.p2p,
			features: value.features,
			preferences: value.preferences,
			image_labeler_version: value.image_labeler_version,
			delete_prompt: value.delete_prompt,
		}
	}
}

#[derive(Serialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: SanitisedNodeConfig,
	data_path: String,
	device_model: Option<String>,
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

			R.query(|_, _: ()| {
				Ok(BuildInfo {
					version: env!("CARGO_PKG_VERSION"),
					commit: env!("GIT_HASH"),
				})
			})
		})
		.procedure("nodeState", {
			R.query(|node, _: ()| async move {
				let device_model = get_hardware_model_name()
					.unwrap_or(HardwareModel::Other)
					.to_string();

				Ok(NodeState {
					config: node.config.get().await.into(),
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: node
						.config
						.data_directory()
						.to_str()
						.expect("Found non-UTF-8 path")
						.to_string(),
					device_model: Some(device_model),
				})
			})
		})
		.procedure("toggleFeatureFlag", {
			R.mutation(|node, feature: BackendFeature| async move {
				let config = node.config.get().await;

				let enabled = if config.features.iter().contains(&feature) {
					node.config
						.write(|cfg| {
							cfg.features.retain(|f| *f != feature);
						})
						.await
						.map(|_| false)
				} else {
					node.config
						.write(|cfg| {
							cfg.features.push(feature.clone());
						})
						.await
						.map(|_| true)
				}
				.map_err(|err| rspc::Error::new(ErrorCode::InternalServerError, err.to_string()))?;

				match feature {
					BackendFeature::CloudSync => {
						node.cloud_sync_flag.store(enabled, Ordering::Relaxed);
					}
				}

				invalidate_query!(node; node, "nodeState");

				Ok(())
			})
		})
		.merge("api.", web_api::mount())
		.merge("auth.", auth::mount())
		.merge("cloud.", cloud::mount())
		.merge("search.", search::mount())
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("labels.", labels::mount())
		// .merge("categories.", categories::mount())
		// .merge("keys.", keys::mount())
		.merge("locations.", locations::mount())
		.merge("ephemeralFiles.", ephemeral_files::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		.merge("p2p.", p2p::mount())
		.merge("models.", models::mount())
		.merge("nodes.", nodes::mount())
		.merge("sync.", sync::mount())
		.merge("preferences.", preferences::mount())
		.merge("notifications.", notifications::mount())
		.merge("backups.", backups::mount())
		.merge("invalidation.", utils::mount_invalidate())
		.sd_patch_types_dangerously(|type_map| {
			let def =
				<sd_prisma::prisma::object::Data as specta::NamedType>::definition_named_data_type(
					type_map,
				);
			type_map.insert(
				<sd_prisma::prisma::object::Data as specta::NamedType>::sid(),
				def,
			);
		});

	let r = r
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
