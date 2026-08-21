//! Switch the mount read recorder on and off.
//!
//! Enabling starts a fresh timeline, so the usual shape of a measurement is
//! enable, do the thing, read `mounts.read_trace`.

use crate::{
	infra::action::{error::ActionError, CoreAction},
	service::mounts::trace,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsTraceSetInput {
	/// Turn recording on or off. Turning it on clears what came before.
	pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsTraceSetOutput {
	pub recording: bool,
	pub message: String,
}

pub struct MountsTraceSetAction {
	input: MountsTraceSetInput,
}

impl CoreAction for MountsTraceSetAction {
	type Input = MountsTraceSetInput;
	type Output = MountsTraceSetOutput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		trace::set_enabled(self.input.enabled);
		Ok(MountsTraceSetOutput {
			recording: self.input.enabled,
			message: if self.input.enabled {
				"recording mount reads; timeline reset".to_string()
			} else {
				"stopped recording; the trace is still readable".to_string()
			},
		})
	}

	fn action_kind(&self) -> &'static str {
		"mounts.trace_set"
	}
}

crate::register_core_action!(MountsTraceSetAction, "mounts.trace_set");
