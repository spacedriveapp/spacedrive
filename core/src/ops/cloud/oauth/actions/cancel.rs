//! `cloud.oauth.cancel` — abort a pending OAuth flow.
//!
//! Signals the loopback task via a `watch::Sender<bool>` so the server stops
//! waiting for a callback, then transitions the flow to `Cancelled`. The
//! terminal grace period in the janitor takes care of eventual eviction.

use super::super::flow::OauthFlowStatus;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for `cloud.oauth.cancel`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CancelInput {
	pub flow_id: Uuid,
}

/// Output for `cloud.oauth.cancel`.
///
/// Returns a boolean so the UI can tell whether the flow was still alive —
/// `false` means it had already completed or was already evicted, which is
/// not an error but useful telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CancelOutput {
	pub cancelled: bool,
}

/// Library action entry point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudOauthCancelAction {
	input: CancelInput,
}

impl CloudOauthCancelAction {
	pub fn new(input: CancelInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for CloudOauthCancelAction {
	type Input = CancelInput;
	type Output = CancelOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		_library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let flow_id = self.input.flow_id;
		context.oauth_flows.cancel(&flow_id);

		let now = chrono::Utc::now();
		let existed = context.oauth_flows.mutate(&flow_id, |flow| {
			// Do not overwrite a prior terminal state. Cancelling an already-completed
			// flow is a no-op except that the watch send above may have been skipped.
			if matches!(flow.status, OauthFlowStatus::Pending) {
				flow.status = OauthFlowStatus::Cancelled;
				flow.terminal_at = Some(now);
			}
		});

		Ok(CancelOutput { cancelled: existed })
	}

	fn action_kind(&self) -> &'static str {
		"cloud.oauth.cancel"
	}
}

crate::register_library_action!(CloudOauthCancelAction, "cloud.oauth.cancel");
