use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCurrentLibraryInput {
	pub library_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCurrentLibraryOutput {
	pub success: bool,
}

pub struct SetCurrentLibraryAction {
	pub input: SetCurrentLibraryInput,
}

impl crate::infra::action::CoreAction for SetCurrentLibraryAction {
	type Output = SetCurrentLibraryOutput;
	type Input = SetCurrentLibraryInput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<Self::Output, crate::infra::action::error::ActionError> {
		context
			.session
			.set_current_library(Some(self.input.library_id))
			.await
			.map_err(|e| crate::infra::action::error::ActionError::Internal(e.to_string()))?;
		Ok(SetCurrentLibraryOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"libraries.session.set_current"
	}
}

crate::register_core_action!(SetCurrentLibraryAction, "libraries.session.set_current");
