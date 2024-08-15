use crate::{
	library::LibraryId,
	node::{
		config::{is_in_docker, NodeConfig, NodeConfigP2P, NodePreferences},
		get_hardware_model_name, HardwareModel,
	},
	old_job::JobProgressEvent,
	Node,
};

use sd_core_heavy_lifting::media_processor::ThumbKey;
use sd_p2p::RemoteIdentity;
use sd_prisma::prisma::file_path;

use std::sync::Arc;

use rspc::{alpha::Rspc, Config};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

mod backups;
mod ephemeral_files;
mod files;
mod jobs;
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

use libraries::KindStatistic;
use utils::{InvalidRequests, InvalidateOperationEvent};

#[allow(non_upper_case_globals)]
pub(crate) const R: Rspc<Ctx> = Rspc::new();

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail {
		thumb_key: ThumbKey,
	},
	NewIdentifiedObjects {
		file_path_ids: Vec<file_path::id::Type>,
	},
	UpdatedKindStatistic(KindStatistic, LibraryId),
	JobProgress(JobProgressEvent),
	InvalidateOperation(InvalidateOperationEvent),
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
	pub preferences: NodePreferences,
	pub image_labeler_version: Option<String>,
}

impl From<NodeConfig> for SanitisedNodeConfig {
	fn from(value: NodeConfig) -> Self {
		Self {
			id: value.id,
			name: value.name,
			identity: value.identity.to_remote_identity(),
			p2p: value.p2p,
			preferences: value.preferences,
			image_labeler_version: value.image_labeler_version,
		}
	}
}

#[derive(Serialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: SanitisedNodeConfig,
	data_path: String,
	device_model: Option<String>,
	is_in_docker: bool,
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
					is_in_docker: is_in_docker(),
				})
			})
		})
		.merge("api.", web_api::mount())
		.merge("search.", search::mount())
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("labels.", labels::mount())
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
