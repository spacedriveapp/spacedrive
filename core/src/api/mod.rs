use crate::{
	invalidate_query,
	job::{JobManagerError, JobProgressEvent},
	library::LibraryManagerError,
	location::{
		indexer::rules::IndexerRuleError, non_indexed::NonIndexedLocationError, LocationError,
	},
	node::config::NodeConfig,
	util::{db::MissingFieldError, error::FileIOError},
	Node,
};
use itertools::Itertools;
use rspc::{ErrorCode, Rspc};
use serde::{Deserialize, Serialize};
use specta::{ts, Type};
use std::sync::{atomic::Ordering, Arc};
use thiserror::Error;
use uuid::Uuid;

use utils::InvalidateOperationEvent;

#[derive(Debug, Error, Serialize, Type)]
pub enum SdError {
	#[error("Internal database error: {0:?}")]
	Prisma(
		#[from]
		#[serde(skip)]
		prisma_client_rust::QueryError,
	),

	// TODO: Remove this variant
	#[error("rspc error: {0:?}")]
	Rspc(#[from] rspc::Error),
	#[error("location error: {0:?}")]
	LocationError(
		#[from]
		#[serde(skip)]
		LocationError,
	),
	#[error("non indexed location error: {0:?}")]
	NonIndexedLocationError(
		#[from]
		#[serde(skip)]
		NonIndexedLocationError,
	),
	#[error("library manager error: {0:?}")]
	LibraryManagerError(
		#[from]
		#[serde(skip)]
		LibraryManagerError,
	),
	#[error("job manager error: {0:?}")]
	JobManagerError(
		#[from]
		#[serde(skip)]
		JobManagerError,
	),
	#[error("job manager error: {0:?}")]
	IndexerRuleError(
		#[from]
		#[serde(skip)]
		IndexerRuleError,
	),
	#[error("file io error: {0:?}")]
	FileIOError(
		#[from]
		#[serde(skip)]
		FileIOError,
	),
	#[error("missing field error: {0:?}")]
	MissingFieldError(
		#[from]
		#[serde(skip)]
		MissingFieldError,
	),
	#[error("images error: {0:?}")]
	SdImagesError(
		#[from]
		#[serde(skip)]
		sd_images::Error,
	),
	#[error("rspc infallible")]
	Infallible(#[from] rspc::Infallible),
}

#[allow(non_upper_case_globals)]
pub(crate) const R: Rspc<Ctx, SdError> = Rspc::new();

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;
pub type RouterBuilder = rspc::RouterBuilder<Ctx>;

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

mod auth;
mod backups;
mod categories;
mod ephemeral_files;
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
mod web_api;

// A version of [NodeConfig] that is safe to share with the frontend
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct SanitisedNodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	pub p2p_enabled: bool,
	pub p2p_port: Option<u16>,
	pub features: Vec<BackendFeature>,
}

impl From<NodeConfig> for SanitisedNodeConfig {
	fn from(value: NodeConfig) -> Self {
		Self {
			id: value.id,
			name: value.name,
			p2p_enabled: value.p2p.enabled,
			p2p_port: value.p2p.port,
			features: value.features,
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

			R.query(|_, _: ()| {
				Ok(BuildInfo {
					version: env!("CARGO_PKG_VERSION"),
					commit: env!("GIT_HASH"),
				})
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
		.merge("api", web_api::mount())
		.merge("auth", auth::mount())
		.merge("search", search::mount())
		.merge("library", libraries::mount())
		.merge("volumes", volumes::mount())
		.merge("tags", tags::mount())
		.merge("categories", categories::mount())
		// .merge("keys.", keys::mount())
		.merge("locations", locations::mount())
		.merge("ephemeralFiles", ephemeral_files::mount())
		.merge("files", files::mount())
		.merge("jobs", jobs::mount())
		.merge("p2p", p2p::mount())
		.merge("nodes", nodes::mount())
		.merge("sync", sync::mount())
		.merge("preferences", preferences::mount())
		.merge("notifications", notifications::mount())
		.merge("backups", backups::mount())
		.merge("invalidation", utils::mount_invalidate())
		.build()
		.unwrap()
		.arced();

	#[cfg(all(debug_assertions, not(feature = "mobile")))]
	r.export_ts(
		rspc::ExportConfig::new(
			std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
				.join("../packages/client/src/core.ts"),
		)
		// TODO: We include some aliases for types that don't exist right now because Prisma Client Rust & Specta aren't playing nice.
		.header(
			"/* eslint-disable */\nexport type Object = NonNullable<FilePathWithObject['object']>;",
		)
		.formatter(ts::prettier),
	)
	.unwrap();

	// InvalidRequests::validate(r.clone()); // This validates all invalidation calls.

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
