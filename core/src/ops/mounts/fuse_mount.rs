//! Mount sources as a read-only directory tree with FUSE (Linux).

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	service::mounts::fuse::{self, FuseError, FuseMountInfo},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FuseMountInput {
	/// An existing, empty directory to mount at.
	pub path: PathBuf,
	/// Show only the directory this grant lends, checking the grant on
	/// every listing and open. Without one, every source is shown.
	#[serde(default)]
	pub grant_token: Option<String>,
}

pub struct FuseMountAction {
	input: FuseMountInput,
}

/// The action error a mount failure surfaces as.
pub(crate) fn action_error(err: FuseError) -> ActionError {
	match err {
		FuseError::Grant(refusal) => ActionError::PermissionDenied {
			action: "mounts.fuse_mount".to_string(),
			reason: refusal.to_string(),
		},
		FuseError::Mount(_) => ActionError::Internal(err.to_string()),
		other => ActionError::InvalidInput(other.to_string()),
	}
}

impl CoreAction for FuseMountAction {
	type Input = FuseMountInput;
	type Output = FuseMountInfo;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		fuse::mount(context, self.input.path, self.input.grant_token)
			.await
			.map_err(action_error)
	}

	fn action_kind(&self) -> &'static str {
		"mounts.fuse_mount"
	}
}

crate::register_core_action!(FuseMountAction, "mounts.fuse_mount");
