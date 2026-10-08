//! List loaded extensions, the jobs they registered, the file kinds and
//! viewers they declare, and the file extensions two of them both claimed.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	domain::ContentKind,
	filetype::{KindConflict, PreviewSpec},
	infra::{
		api::SessionContext,
		query::{CoreQuery, QueryResult},
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListExtensionsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionJobInfo {
	/// Name the extension registered the job under
	pub name: String,
	/// Full name a run request uses: `<extension id>:<name>`
	pub full_name: String,
	pub resumable: bool,
}

/// A file kind an extension declares, as the client resolves previews and
/// icons against it. The kinds of every loaded extension are the client's
/// only source of extension kinds, which is what lets a stored kind name
/// fall back to its parent once the extension is gone.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionKindInfo {
	/// `<extension id>:<name>`, the value `File.content_kind_name` carries
	pub id: String,
	pub name: String,
	pub display_name: String,
	pub parent: ContentKind,
	/// Every extension the manifest claims, contested ones included
	pub extensions: Vec<String>,
	pub preview: Option<PreviewSpec>,
}

/// A viewer `ui_manifest.json` declares. The client mounts the bundle from
/// `/extension/<extension id>/<bundle>` for a kind whose `preview.viewer`
/// names the id.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionViewerInfo {
	pub id: String,
	/// Path inside the extension directory to one ES module
	pub bundle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionInfo {
	pub id: String,
	pub name: String,
	pub version: String,
	pub jobs: Vec<ExtensionJobInfo>,
	pub kinds: Vec<ExtensionKindInfo>,
	pub viewers: Vec<ExtensionViewerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListExtensionsOutput {
	/// Whether this build can load extensions at all
	pub supported: bool,
	pub extensions: Vec<ExtensionInfo>,
	/// File extensions two loaded extensions both claimed, resolved by load
	/// order: the kind loaded first holds each one.
	pub conflicts: Vec<KindConflict>,
}

pub struct ListExtensionsQuery;

impl CoreQuery for ListExtensionsQuery {
	type Input = ListExtensionsInput;
	type Output = ListExtensionsOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: SessionContext,
	) -> QueryResult<Self::Output> {
		#[cfg(feature = "wasm")]
		{
			let Some(plugin_manager) = context.get_plugin_manager().await else {
				return Ok(ListExtensionsOutput {
					supported: true,
					extensions: Vec::new(),
					conflicts: Vec::new(),
				});
			};
			let pm = plugin_manager.read().await;
			let registry = pm.job_registry();
			let mut extensions = Vec::new();
			for id in pm.list_plugins().await {
				let Some(manifest) = pm.get_manifest(&id).await else {
					continue;
				};
				let mut jobs: Vec<ExtensionJobInfo> = registry
					.list_jobs_for_extension(&id)
					.into_iter()
					.map(|job| ExtensionJobInfo {
						name: job.job_name,
						full_name: job.full_name,
						resumable: job.resumable,
					})
					.collect();
				jobs.sort_by(|a, b| a.name.cmp(&b.name));
				let kinds = manifest
					.kinds
					.iter()
					.map(|kind| ExtensionKindInfo {
						id: kind.id(&id),
						name: kind.name.clone(),
						display_name: kind
							.display_name
							.clone()
							.unwrap_or_else(|| kind.name.clone()),
						parent: kind.parent,
						extensions: kind.extensions.clone(),
						preview: kind.preview.clone(),
					})
					.collect();
				let viewers = pm
					.ui_manifest(&id)
					.await
					.map(|ui| {
						ui.file_viewers
							.iter()
							.map(|viewer| ExtensionViewerInfo {
								id: viewer.id.clone(),
								bundle: viewer.bundle.clone(),
							})
							.collect()
					})
					.unwrap_or_default();
				extensions.push(ExtensionInfo {
					id,
					name: manifest.name.clone(),
					version: manifest.version.clone(),
					jobs,
					kinds,
					viewers,
				});
			}
			extensions.sort_by(|a, b| a.id.cmp(&b.id));
			Ok(ListExtensionsOutput {
				supported: true,
				extensions,
				conflicts: crate::filetype::FileTypeRegistry::current()
					.conflicts()
					.to_vec(),
			})
		}
		#[cfg(not(feature = "wasm"))]
		{
			let _ = context;
			Ok(ListExtensionsOutput {
				supported: false,
				extensions: Vec::new(),
				conflicts: Vec::new(),
			})
		}
	}
}

crate::register_core_query!(ListExtensionsQuery, "extensions.list");
