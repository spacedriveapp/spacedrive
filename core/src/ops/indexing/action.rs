//! Indexing action handler

use super::job::{IndexMode, IndexPersistence, IndexScope, IndexerJob, IndexerJobConfig};
use super::IndexInput;
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use async_trait::async_trait;
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

pub struct IndexingHandler;

impl IndexingHandler {
	pub fn new() -> Self {
		Self
	}
}

impl LibraryAction for IndexingAction {
	type Output = JobHandle;

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Validate input first
		if let Err(errors) = self.input.validate() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: errors.join("; "),
			});
		}

		// For now, submit one job per path (sequentially). Could be parallelized later.
		// Return the handle of the last job submitted for convenience.
		let mut last_handle: Option<JobHandle> = None;

		for path in &self.input.paths {
			let sd_path = crate::domain::addressing::SdPath::local(path.clone());

			let mut config = match self.input.persistence {
				IndexPersistence::Ephemeral => {
					IndexerJobConfig::ephemeral_browse(sd_path, self.input.scope)
				}
				IndexPersistence::Persistent => {
					// Persistent indexing expects a location context. For now, default to recursive path walk with selected mode.
					// If we later bind paths to a location, we can set location_id properly.
					// Here use ui_navigation/new with mode overridden below when possible.
					let mut c = IndexerJobConfig::ephemeral_browse(sd_path, self.input.scope);
					c.persistence = IndexPersistence::Persistent;
					c
				}
			};

			// Apply selected mode
			config.mode = self.input.mode;

			// TODO: Apply include_hidden via rule_toggles when available

			let job = IndexerJob::new(config);
			let handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(ActionError::Job)?;
			last_handle = Some(handle);
		}

		last_handle.ok_or(ActionError::Validation {
			field: "paths".to_string(),
			message: "No paths provided".to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"indexing.index"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Validate paths
		if self.input.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: "At least one path must be specified".to_string(),
			});
		}

		Ok(())
	}
}
