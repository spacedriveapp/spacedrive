//! Unmount a FUSE mount the daemon holds.

use super::fuse_mount::action_error;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	service::mounts::fuse::{self, FuseMountInfo},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FuseUnmountInput {
	pub path: PathBuf,
}

pub struct FuseUnmountAction {
	input: FuseUnmountInput,
}

impl CoreAction for FuseUnmountAction {
	type Input = FuseUnmountInput;
	type Output = FuseMountInfo;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(self, _context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		fuse::unmount(&self.input.path).await.map_err(action_error)
	}

	fn action_kind(&self) -> &'static str {
		"mounts.fuse_unmount"
	}
}

crate::register_core_action!(FuseUnmountAction, "mounts.fuse_unmount");
