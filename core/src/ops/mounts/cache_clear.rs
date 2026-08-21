//! Drop cached blocks. Safe at any moment — the cache is rebuildable, so
//! the next read simply refetches.

use crate::{
	infra::action::{error::ActionError, CoreAction},
	service::mounts::cache,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsCacheClearInput {
	/// Clear the on-disk tier as well. Off by default, so the common case
	/// (free memory, keep the expensive-to-refetch tier) is the safe one.
	#[serde(default)]
	pub include_disk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsCacheClearOutput {
	pub freed_l1_bytes: u64,
	pub freed_l2_bytes: u64,
	pub message: String,
}

pub struct MountsCacheClearAction {
	input: MountsCacheClearInput,
}

impl CoreAction for MountsCacheClearAction {
	type Input = MountsCacheClearInput;
	type Output = MountsCacheClearOutput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		let Some(cache) = cache::cache() else {
			return Err(ActionError::Internal(
				"mounts cache is not running".to_string(),
			));
		};

		let before = cache.snapshot();
		cache.clear(self.input.include_disk).await;

		let freed_l2 = if self.input.include_disk {
			before.l2_bytes
		} else {
			0
		};
		Ok(MountsCacheClearOutput {
			freed_l1_bytes: before.l1_bytes,
			freed_l2_bytes: freed_l2,
			message: if self.input.include_disk {
				format!(
					"cleared {} block(s) from memory and {} from disk",
					before.l1_blocks, before.l2_blocks
				)
			} else {
				format!("cleared {} block(s) from memory", before.l1_blocks)
			},
		})
	}

	fn action_kind(&self) -> &'static str {
		"mounts.cache_clear"
	}
}

crate::register_core_action!(MountsCacheClearAction, "mounts.cache_clear");
