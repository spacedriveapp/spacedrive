//! Pause and resume replica fetching on this device.
//!
//! Paused, the daemon starts no replica transfer and stops any in flight at
//! its next chunk, keeping the partial file so a resume continues from it.
//! Listings, owner facts and fleet totals still refresh, and the device
//! still serves its own sources. The switch is persisted in the daemon
//! config, so a paused daemon stays paused across a restart.

use crate::{
	config::AppConfig,
	infra::action::{error::ActionError, CoreAction},
	service::mounts::replication,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsReplicationSetPausedInput {
	pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsReplicationSetPausedOutput {
	pub paused: bool,
	pub message: String,
}

pub struct MountsReplicationSetPausedAction {
	input: MountsReplicationSetPausedInput,
}

impl CoreAction for MountsReplicationSetPausedAction {
	type Input = MountsReplicationSetPausedInput;
	type Output = MountsReplicationSetPausedOutput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		let paused = self.input.paused;
		let mut config = AppConfig::load_from(&context.data_dir)
			.map_err(|e| ActionError::Internal(format!("Failed to load config: {e}")))?;
		if config.replication.paused != paused {
			config.replication.paused = paused;
			config
				.save()
				.map_err(|e| ActionError::Internal(format!("Failed to save config: {e}")))?;
		}
		replication::set_paused(paused);
		if !paused {
			crate::service::mounts::resync_connected(context);
		}
		Ok(MountsReplicationSetPausedOutput {
			paused,
			message: if paused {
				"replication paused; transfers in flight stop and keep their partial files"
					.to_string()
			} else {
				"replication resumed; deferred transfers continue from their partial files"
					.to_string()
			},
		})
	}

	fn action_kind(&self) -> &'static str {
		"mounts.replication_set_paused"
	}
}

crate::register_core_action!(
	MountsReplicationSetPausedAction,
	"mounts.replication_set_paused"
);
