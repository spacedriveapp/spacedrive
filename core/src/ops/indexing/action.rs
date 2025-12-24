//! # Indexing Action Handler
//!
//! Bridges user-facing indexing requests (from CLI, API, UI) to the internal IndexerJob system.
//! Actions validate inputs, convert paths to SdPaths, dispatch jobs to the library's job queue,
//! and track execution context for observability. Each action can spawn multiple jobs (one per
//! path), but returns only the last handle for API simplicity.

use super::job::{IndexMode, IndexPersistence, IndexScope, IndexerJob, IndexerJobConfig};
use super::IndexInput;
use crate::{
	context::CoreContext,
	infra::{
		action::{context::ActionContextProvider, error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexingAction {
	pub input: IndexInput,
}

impl IndexingAction {
	pub fn new(input: IndexInput) -> Self {
		Self { input }
	}
}

pub struct IndexingActionBuilder {
	input: IndexInput,
}

impl IndexingActionBuilder {
	pub fn from_input(input: IndexInput) -> Self {
		Self { input }
	}
}

impl crate::infra::action::builder::ActionBuilder for IndexingActionBuilder {
	type Action = IndexingAction;
	type Error = crate::infra::action::builder::ActionBuildError;

	fn validate(&self) -> Result<(), Self::Error> {
		self.input
			.validate()
			.map_err(crate::infra::action::builder::ActionBuildError::validations)
	}

	fn build(self) -> Result<Self::Action, Self::Error> {
		self.validate()?;
		Ok(IndexingAction::new(self.input))
	}
}

impl LibraryAction for IndexingAction {
	type Input = IndexInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(input: IndexInput) -> Result<Self, String> {
		Ok(IndexingAction::new(input))
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		if let Err(errors) = self.input.validate() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: errors.join("; "),
			});
		}
		Ok(crate::infra::action::ValidationResult::Success)
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let mut last_handle: Option<JobHandle> = None;

		for path in &self.input.paths {
			let sd_path = crate::domain::addressing::SdPath::local(path.clone());

			let mut config = match self.input.persistence {
				IndexPersistence::Ephemeral => {
					IndexerJobConfig::ephemeral_browse(sd_path, self.input.scope)
				}
				IndexPersistence::Persistent => {
					// Persistent mode stores entries in the database but doesn't require a location binding yet.
					let mut c = IndexerJobConfig::ephemeral_browse(sd_path, self.input.scope);
					c.persistence = IndexPersistence::Persistent;
					c
				}
			};

			config.mode = self.input.mode.clone();

			// TODO: Apply include_hidden via rule_toggles when available

			let job = IndexerJob::new(config);
			let handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(ActionError::Job)?;
			last_handle = Some(handle);
		}

		last_handle
			.ok_or(ActionError::Validation {
				field: "paths".to_string(),
				message: "No paths provided".to_string(),
			})
			.map(|handle| handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"indexing.index"
	}
}

impl ActionContextProvider for IndexingAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "manual_scan",
				"trigger": "user_action",
				"paths_count": self.input.paths.len(),
				"mode": self.input.mode,
				"scope": self.input.scope,
				"persistence": self.input.persistence
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"indexing.scan"
	}
}

crate::register_library_action!(IndexingAction, "indexing.start");
