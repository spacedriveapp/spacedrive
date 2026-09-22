//! Publish where a window is looking.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::focus::{NavigationFocus, DEFAULT_GROUP};
use crate::{
	context::CoreContext,
	domain::{resource::EventEmitter, SdPath},
	infra::action::{error::ActionError, CoreAction},
	ops::search::FileSearchInput,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetNavigationFocusInput {
	/// The focus group to publish into; the default group when omitted.
	#[serde(default)]
	pub group: Option<String>,
	/// Where the window is looking, or `None` when it is showing something
	/// without a path.
	pub path: Option<SdPath>,
	/// The search the window is running, as it sends it, when it is running one.
	#[serde(default)]
	pub search: Option<FileSearchInput>,
	#[serde(default)]
	pub library_id: Option<Uuid>,
	/// Label naming the publishing window, echoed back to followers.
	#[serde(default)]
	pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetNavigationFocusOutput {
	pub focus: NavigationFocus,
	/// False when the group was already at this position and no event went out.
	pub moved: bool,
}

pub struct SetNavigationFocusAction {
	focus: NavigationFocus,
}

impl CoreAction for SetNavigationFocusAction {
	type Input = SetNavigationFocusInput;
	type Output = SetNavigationFocusOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		let group = input.group.unwrap_or_else(|| DEFAULT_GROUP.to_string());
		if group.is_empty() {
			return Err("focus group must not be empty".to_string());
		}
		Ok(Self {
			focus: NavigationFocus {
				id: NavigationFocus::id_for_group(&group),
				group,
				path: input.path,
				search: input.search,
				library_id: input.library_id,
				origin: input.origin,
				updated_at: Utc::now(),
			},
		})
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		match context.navigation_focus.set(self.focus.clone()) {
			Some(focus) => {
				focus
					.emit_changed(&context.events)
					.map_err(|error| ActionError::Internal(error.to_string()))?;
				Ok(SetNavigationFocusOutput { focus, moved: true })
			}
			None => Ok(SetNavigationFocusOutput {
				focus: self.focus,
				moved: false,
			}),
		}
	}

	fn action_kind(&self) -> &'static str {
		"navigation.set_focus"
	}
}

crate::register_core_action!(SetNavigationFocusAction, "navigation.set_focus");
