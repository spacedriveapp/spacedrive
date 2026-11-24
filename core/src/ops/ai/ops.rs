//! Listing, downloading and removing models, and the cloud caption opt-in.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

use super::{catalog, cloud_key, Category, Locality};
use crate::context::CoreContext;
use crate::infra::action::{context::ActionContext, error::ActionError, LibraryAction};
use crate::infra::job::types::JobPriority;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::library::Library;
use crate::ops::models::download::{ModelDownloadConfig, ModelDownloadJob};

/// A catalog model, with what the user needs to decide on a download.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderModel {
	pub id: String,
	pub name: String,
	pub category: Category,
	pub description: String,
	/// SPDX identifier the user accepts by downloading.
	pub license: String,
	pub license_url: String,
	pub size_bytes: u64,
	pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CloudCaptionStatus {
	pub enabled: bool,
	pub base_url: Option<String>,
	pub model: Option<String>,
	/// Whether a key is stored. The key itself is never returned.
	pub has_key: bool,
}

async fn cloud_status(library: &Library) -> CloudCaptionStatus {
	let settings = library.config().await.settings.ai.cloud_caption;
	CloudCaptionStatus {
		enabled: settings.enabled,
		base_url: settings.base_url,
		model: settings.model,
		has_key: cloud_key::has_key(library).await,
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListProvidersInput {}

/// The provider a category resolves to, picked the way `ai.infer` picks it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveProvider {
	pub category: Category,
	pub locality: Locality,
	/// The catalog model that answers locally. `None` for the cloud
	/// captioner, whose model is in `cloud_caption`. An extension whose
	/// manifest asks for local inference never reaches the cloud one.
	pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListProvidersOutput {
	pub models: Vec<ProviderModel>,
	pub cloud_caption: CloudCaptionStatus,
	/// Categories with a provider. A category missing here has none.
	pub active: Vec<ActiveProvider>,
	/// Whether this build can run local models (the `ai-local` feature).
	/// Without it the catalog is listed but nothing can be downloaded or run
	/// on this device. The cloud captioner needs only `ai-cloud`.
	pub inference_available: bool,
}

pub struct ListProvidersQuery;

impl LibraryQuery for ListProvidersQuery {
	type Input = ListProvidersInput;
	type Output = ListProvidersOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;
		let mut models = Vec::new();
		for spec in catalog::CATALOG {
			models.push(ProviderModel {
				id: spec.id.to_string(),
				name: spec.name.to_string(),
				category: spec.category,
				description: spec.description.to_string(),
				license: spec.license.to_string(),
				license_url: spec.license_url.to_string(),
				size_bytes: spec.size(),
				installed: super::is_installed(&context.data_dir, spec).await,
			});
		}
		let cloud_caption = cloud_status(&library).await;
		let inference_available = cfg!(feature = "ai-local");
		let active = Category::ALL
			.into_iter()
			.filter_map(|category| {
				if let Some(model) = models
					.iter()
					.find(|m| inference_available && m.category == category && m.installed)
				{
					return Some(ActiveProvider {
						category,
						locality: Locality::Local,
						model_id: Some(model.id.clone()),
					});
				}
				(cfg!(feature = "ai-cloud")
					&& category == Category::ImageCaption
					&& cloud_caption.enabled)
					.then_some(ActiveProvider {
						category,
						locality: Locality::Cloud,
						model_id: None,
					})
			})
			.collect();
		Ok(ListProvidersOutput {
			models,
			cloud_caption,
			active,
			inference_available,
		})
	}
}

crate::register_library_query!(ListProvidersQuery, "ai.providers.list");

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadModelInput {
	pub model_id: String,
	/// The model's license identifier, as `ai.providers.list` shows it. A
	/// download without it is refused, so no client can fetch a model the
	/// user was not shown the license of.
	pub accept_license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadModelOutput {
	/// One download job per file still missing.
	pub job_ids: Vec<String>,
	pub size_bytes: u64,
}

pub struct DownloadModelAction {
	input: DownloadModelInput,
}

impl LibraryAction for DownloadModelAction {
	type Input = DownloadModelInput;
	type Output = DownloadModelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let spec = catalog::find(&self.input.model_id).ok_or_else(|| {
			ActionError::InvalidInput(format!("unknown model {}", self.input.model_id))
		})?;
		if !cfg!(feature = "ai-local") {
			return Err(ActionError::InvalidInput(
				"this build of Spacedrive cannot run local inference models".to_string(),
			));
		}
		if self.input.accept_license != spec.license {
			return Err(ActionError::InvalidInput(format!(
				"{} is licensed {}; pass accept_license \"{}\" to download it",
				spec.id, spec.license, spec.license
			)));
		}
		let dir = super::model_dir(&context.data_dir, spec);
		let mut job_ids = Vec::new();
		let mut size_bytes = 0;
		for file in spec.files {
			let on_disk = tokio::fs::metadata(dir.join(file.name)).await;
			if on_disk.is_ok_and(|meta| meta.len() == file.size) {
				continue;
			}
			let job = ModelDownloadJob::new(ModelDownloadConfig {
				model_id: spec.file_id(file),
				data_dir: context.data_dir.clone(),
			});
			// The model id rides on the job so a client can match a running
			// download to its model after a restart.
			let action_context = ActionContext::new(
				"ai.models.download",
				json!({ "model_id": spec.id }),
				json!({ "file": file.name, "size_bytes": file.size }),
			);
			let handle = library
				.jobs()
				.dispatch_with_priority(job, JobPriority::NORMAL, Some(action_context))
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to dispatch download: {e}")))?;
			job_ids.push(handle.id().to_string());
			size_bytes += file.size;
		}
		Ok(DownloadModelOutput {
			job_ids,
			size_bytes,
		})
	}

	fn action_kind(&self) -> &'static str {
		"ai.models.download"
	}
}

crate::register_library_action!(DownloadModelAction, "ai.models.download");

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteModelInput {
	pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteModelOutput {
	pub deleted: bool,
}

pub struct DeleteModelAction {
	input: DeleteModelInput,
}

impl LibraryAction for DeleteModelAction {
	type Input = DeleteModelInput;
	type Output = DeleteModelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let spec = catalog::find(&self.input.model_id).ok_or_else(|| {
			ActionError::InvalidInput(format!("unknown model {}", self.input.model_id))
		})?;
		#[cfg(feature = "ai-cloud")]
		context.ai.unload(spec).await;
		let dir = super::model_dir(&context.data_dir, spec);
		let deleted = match tokio::fs::remove_dir_all(&dir).await {
			Ok(()) => true,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
			Err(e) => return Err(ActionError::Internal(format!("{}: {e}", dir.display()))),
		};
		Ok(DeleteModelOutput { deleted })
	}

	fn action_kind(&self) -> &'static str {
		"ai.models.delete"
	}
}

crate::register_library_action!(DeleteModelAction, "ai.models.delete");

/// Changes the library's cloud caption opt-in. Each named field replaces the
/// stored one; `api_key` is written encrypted and never echoed back.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ConfigureCloudCaptionInput {
	#[serde(default)]
	pub enabled: Option<bool>,
	#[serde(default)]
	pub base_url: Option<String>,
	#[serde(default)]
	pub model: Option<String>,
	#[serde(default)]
	pub api_key: Option<String>,
	/// Removes the stored key, which also turns the opt-in off.
	#[serde(default)]
	pub clear_key: bool,
}

pub struct ConfigureCloudCaptionAction {
	input: ConfigureCloudCaptionInput,
}

impl LibraryAction for ConfigureCloudCaptionAction {
	type Input = ConfigureCloudCaptionInput;
	type Output = CloudCaptionStatus;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if let Some(url) = &input.base_url {
			if !(url.starts_with("https://") || url.starts_with("http://")) {
				return Err("base_url must be an http or https URL".to_string());
			}
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let internal = |e: super::AiError| ActionError::Internal(e.to_string());
		if self.input.clear_key {
			cloud_key::clear_key(&library).await.map_err(internal)?;
		}
		if let Some(key) = self.input.api_key.filter(|k| !k.trim().is_empty()) {
			cloud_key::store_key(&library, key.trim().to_string())
				.await
				.map_err(internal)?;
		}
		let current = library.config().await.settings.ai.cloud_caption;
		let base_url = self.input.base_url.or(current.base_url);
		let model = self.input.model.or(current.model);
		let enabled = !self.input.clear_key && self.input.enabled.unwrap_or(current.enabled);
		if enabled && (base_url.is_none() || model.is_none() || !cloud_key::has_key(&library).await)
		{
			return Err(ActionError::InvalidInput(
				"cloud captioning needs a base_url, a model and an api_key before it can be enabled"
					.to_string(),
			));
		}
		library
			.update_config(|config| {
				let settings = &mut config.settings.ai.cloud_caption;
				settings.enabled = enabled;
				settings.base_url = base_url;
				settings.model = model;
			})
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		Ok(cloud_status(&library).await)
	}

	fn action_kind(&self) -> &'static str {
		"ai.cloud_caption.configure"
	}
}

crate::register_library_action!(ConfigureCloudCaptionAction, "ai.cloud_caption.configure");
