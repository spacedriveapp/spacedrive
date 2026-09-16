use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	service::external_tools::{ExternalToolId, ExternalToolStatus, ToolInstaller},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolInstallInput {
	pub tool: ExternalToolId,
	pub installer: ToolInstaller,
	/// Installing a host executable is never an implicit side effect. A caller
	/// sets this only after showing the selected package manager to the person.
	pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolInstallOutput {
	pub tool: ExternalToolStatus,
}

pub struct ToolInstallAction {
	input: ToolInstallInput,
}

impl CoreAction for ToolInstallAction {
	type Input = ToolInstallInput;
	type Output = ToolInstallOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if !input.confirm {
			return Err(
				"installing host software requires confirm=true after user approval".to_string(),
			);
		}
		Ok(Self { input })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		context
			.external_tools
			.install(self.input.tool, self.input.installer)
			.await
			.map_err(|error| ActionError::Internal(error.to_string()))?;
		let status = context
			.external_tools
			.statuses()
			.await
			.into_iter()
			.find(|status| status.id == self.input.tool)
			.ok_or_else(|| {
				ActionError::Internal("installed tool disappeared from registry".into())
			})?;
		Ok(ToolInstallOutput { tool: status })
	}

	fn action_kind(&self) -> &'static str {
		"tools.install"
	}
}

crate::register_core_action!(ToolInstallAction, "tools.install");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn installation_requires_explicit_confirmation() {
		let result = ToolInstallAction::from_input(ToolInstallInput {
			tool: ExternalToolId::Ffmpeg,
			installer: ToolInstaller::Homebrew,
			confirm: false,
		});
		assert!(result.is_err());
	}
}
