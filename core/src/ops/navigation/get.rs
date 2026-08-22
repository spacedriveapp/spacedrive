//! Read a focus group's current position.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::focus::{NavigationFocus, DEFAULT_GROUP};
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NavigationFocusInput {
	/// The focus group to read; the default group when omitted.
	#[serde(default)]
	pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NavigationFocusOutput {
	pub focus: NavigationFocus,
}

pub struct NavigationFocusQuery {
	group: String,
}

impl CoreQuery for NavigationFocusQuery {
	type Input = NavigationFocusInput;
	type Output = NavigationFocusOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			group: input.group.unwrap_or_else(|| DEFAULT_GROUP.to_string()),
		})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// A group nobody has published to reads as empty rather than missing,
		// so a follower that starts first has something to render.
		let focus = context
			.navigation_focus
			.get(&self.group)
			.unwrap_or_else(|| NavigationFocus::empty(self.group));
		Ok(NavigationFocusOutput { focus })
	}
}

crate::register_core_query!(NavigationFocusQuery, "navigation.focus");
