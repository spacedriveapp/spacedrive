//! List loaded extensions and the jobs they registered

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionInfo {
	pub id: String,
	pub name: String,
	pub version: String,
	pub jobs: Vec<ExtensionJobInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListExtensionsOutput {
	/// Whether this build can load extensions at all
	pub supported: bool,
	pub extensions: Vec<ExtensionInfo>,
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
				extensions.push(ExtensionInfo {
					id,
					name: manifest.name,
					version: manifest.version,
					jobs,
				});
			}
			extensions.sort_by(|a, b| a.id.cmp(&b.id));
			Ok(ListExtensionsOutput {
				supported: true,
				extensions,
			})
		}
		#[cfg(not(feature = "wasm"))]
		{
			let _ = context;
			Ok(ListExtensionsOutput {
				supported: false,
				extensions: Vec::new(),
			})
		}
	}
}

crate::register_core_query!(ListExtensionsQuery, "extensions.list");
