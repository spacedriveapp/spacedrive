//! Mount sources as a read-only directory tree through a localhost NFS server.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	service::mounts::nfs::{self, NfsError, NfsMountInfo},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NfsMountInput {
	/// An existing, empty directory to mount at.
	pub path: PathBuf,
	/// Show only the directory this grant lends, checking the grant on
	/// every listing and open. Without one, every source is shown.
	#[serde(default)]
	pub grant_token: Option<String>,
}

pub struct NfsMountAction {
	input: NfsMountInput,
}

/// The action error a mount failure surfaces as.
pub(crate) fn action_error(err: NfsError) -> ActionError {
	match err {
		NfsError::Grant(refusal) => ActionError::PermissionDenied {
			action: "mounts.nfs_mount".to_string(),
			reason: refusal.to_string(),
		},
		NfsError::Mount(_) => ActionError::Internal(err.to_string()),
		other => ActionError::InvalidInput(other.to_string()),
	}
}

impl CoreAction for NfsMountAction {
	type Input = NfsMountInput;
	type Output = NfsMountInfo;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		nfs::mount(context, self.input.path, self.input.grant_token)
			.await
			.map_err(action_error)
	}

	fn action_kind(&self) -> &'static str {
		"mounts.nfs_mount"
	}
}

crate::register_core_action!(NfsMountAction, "mounts.nfs_mount");
