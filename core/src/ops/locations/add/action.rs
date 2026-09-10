//! Pin a folder, which is what makes it a location.

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities::location::Origin,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationAddInput {
	pub path: crate::domain::addressing::SdPath,
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddAction {
	input: LocationAddInput,
}

impl LocationAddAction {
	pub fn new(input: LocationAddInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LocationAddAction {
	type Input = LocationAddInput;
	type Output = LocationAddOutput;

	fn from_input(input: LocationAddInput) -> Result<Self, String> {
		Ok(LocationAddAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> ActionResult<Self::Output> {
		let path = self
			.input
			.path
			.as_local_path()
			.ok_or_else(|| ActionError::InvalidInput("Only local paths can be pinned".into()))?
			.to_path_buf();

		// The folder's own name is what a person would have typed anyway.
		let name = self.input.name.clone().unwrap_or_else(|| {
			path.file_name()
				.map(|name| name.to_string_lossy().to_string())
				.unwrap_or_else(|| path.to_string_lossy().to_string())
		});

		let location = crate::location::pin(&library, &context, &path, name, Origin::User)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		Ok(LocationAddOutput::new(
			location.id,
			location.sd_path,
			Some(location.name),
		))
	}

	fn action_kind(&self) -> &'static str {
		"locations.add"
	}
}

impl ActionContextProvider for LocationAddAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "pin_location",
				"trigger": "user_action",
				"path": self.input.path.to_string(),
				"name": self.input.name,
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.add"
	}
}

crate::register_library_action!(LocationAddAction, "locations.add");
