//! Unmount an NFS mount the daemon holds.

use super::nfs_mount::action_error;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	service::mounts::nfs::{self, NfsMountInfo},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NfsUnmountInput {
	pub path: PathBuf,
}

pub struct NfsUnmountAction {
	input: NfsUnmountInput,
}

impl CoreAction for NfsUnmountAction {
	type Input = NfsUnmountInput;
	type Output = NfsMountInfo;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(self, _context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		nfs::unmount(&self.input.path).await.map_err(action_error)
	}

	fn action_kind(&self) -> &'static str {
		"mounts.nfs_unmount"
	}
}

crate::register_core_action!(NfsUnmountAction, "mounts.nfs_unmount");
