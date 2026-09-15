//! Update a source's name or capture policy.
//!
//! Capture is the one write-time policy a source has, so changing it has one
//! asymmetry worth naming: turning the rules off means the store is missing
//! what they skipped, and only a walk can supply it, so one is dispatched.
//! Turning the rules on removes nothing, because hiding is the lens's job and
//! a store never un-captures.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::indexing::ephemeral::SourceConfig,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateSourceInput {
	pub source_id: String,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub unfiltered: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateSourceOutput {
	pub name: String,
	pub unfiltered: bool,
	/// The walk dispatched to capture what the rules previously skipped.
	/// Only set when the policy widened.
	pub rewalk_job: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSourceAction {
	input: UpdateSourceInput,
}

impl LibraryAction for UpdateSourceAction {
	type Input = UpdateSourceInput;
	type Output = UpdateSourceOutput;

	fn from_input(input: UpdateSourceInput) -> Result<Self, String> {
		if input.source_id.trim().is_empty() {
			return Err("Source ID cannot be empty".to_string());
		}
		if input.name.is_none() && input.unfiltered.is_none() {
			return Err("Nothing to update".to_string());
		}
		if input
			.name
			.as_deref()
			.is_some_and(|name| name.trim().is_empty())
		{
			return Err("Name cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let source_id = uuid::Uuid::parse_str(&self.input.source_id)
			.map_err(|e| ActionError::Internal(format!("Invalid source ID: {e}")))?;

		let cache = context.ephemeral_cache();
		let previous = cache.source_config(source_id).ok_or_else(|| {
			ActionError::Internal(format!(
				"source {source_id} is not registered on this machine"
			))
		})?;

		if let Some(name) = self.input.name.clone() {
			cache.set_source_name(source_id, name).await;
		}

		let unfiltered = self.input.unfiltered.unwrap_or(previous.unfiltered);
		let mut rewalk_job = None;
		if self.input.unfiltered.is_some() {
			cache
				.set_source_config(source_id, SourceConfig { unfiltered })
				.await;

			// Widening capture means the store lacks what the rules skipped,
			// and only a walk can supply it. Narrowing removes nothing: the
			// records stand, and the lens decides what a person sees.
			if unfiltered && !previous.unfiltered {
				let root = cache.source_root(source_id).ok_or_else(|| {
					ActionError::Internal("source has no root on this machine".to_string())
				})?;
				let output = crate::ops::sources::track::action::track_and_index(
					&library, &context, root, true,
				)
				.await?;
				rewalk_job = output.job_id;
			}
		}

		let name = cache.source_name(source_id).unwrap_or_default();

		Ok(UpdateSourceOutput {
			name,
			unfiltered,
			rewalk_job,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.update"
	}
}

crate::register_library_action!(UpdateSourceAction, "sources.update");
