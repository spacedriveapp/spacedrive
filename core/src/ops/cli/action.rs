//! `cli.install` and `cli.uninstall`: link or unlink `sd`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::CliInstallStatus;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CliInstallInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CliInstallOutput {
	pub status: CliInstallStatus,
	/// Administrator access was refused, so the link went to `~/.local/bin`
	/// instead of `/usr/local/bin`.
	pub fell_back: bool,
	/// A command that a default PATH finds before the new link, such as an
	/// older `/usr/local/bin/sd` that only an administrator can replace.
	pub shadowed_by: Option<String>,
}

pub struct CliInstallAction;

impl CoreAction for CliInstallAction {
	type Input = CliInstallInput;
	type Output = CliInstallOutput;

	fn from_input(_input: Self::Input) -> Result<Self, String> {
		Ok(Self)
	}

	async fn execute(self, _context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let installed = super::install().await.map_err(ActionError::Internal)?;
		Ok(CliInstallOutput {
			status: super::status().await,
			fell_back: installed.fell_back,
			shadowed_by: installed.shadowed_by,
		})
	}

	fn action_kind(&self) -> &'static str {
		"cli.install"
	}
}

crate::register_core_action!(CliInstallAction, "cli.install");

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CliUninstallInput {}

pub struct CliUninstallAction;

impl CoreAction for CliUninstallAction {
	type Input = CliUninstallInput;
	type Output = CliInstallStatus;

	fn from_input(_input: Self::Input) -> Result<Self, String> {
		Ok(Self)
	}

	async fn execute(self, _context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		super::uninstall().await.map_err(ActionError::Internal)?;
		Ok(super::status().await)
	}

	fn action_kind(&self) -> &'static str {
		"cli.uninstall"
	}
}

crate::register_core_action!(CliUninstallAction, "cli.uninstall");
