//! The wire surface: one library action per write and a query for state.
//!
//! The local socket is trusted, yet these still go through the grant: they
//! exist so a client holding only a token, and the tests, reach the same
//! code the agent endpoint calls.

use std::sync::Arc;

use super::{
	ops, DeleteInput, MakeDirInput, MoveInput, PathState, StatInput, WriteError, WriteFileInput,
	WriteOutcome,
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		query::{LibraryQuery, QueryError, QueryResult},
	},
	library::Library,
};

fn refused(action: &str, error: WriteError) -> ActionError {
	match error {
		WriteError::Grant(grant) => ActionError::PermissionDenied {
			action: action.to_string(),
			reason: grant.to_string(),
		},
		stale @ WriteError::Stale { .. } => ActionError::Validation {
			field: "expect".to_string(),
			message: stale.to_string(),
		},
		other => ActionError::InvalidInput(other.to_string()),
	}
}

macro_rules! agent_action {
	($action:ident, $input:ty, $kind:literal, $run:path) => {
		pub struct $action {
			input: $input,
		}

		impl LibraryAction for $action {
			type Input = $input;
			type Output = WriteOutcome;

			fn from_input(input: Self::Input) -> Result<Self, String> {
				Ok(Self { input })
			}

			async fn execute(
				self,
				_library: Arc<Library>,
				context: Arc<CoreContext>,
			) -> Result<Self::Output, ActionError> {
				$run(&context, self.input)
					.await
					.map_err(|error| refused($kind, error))
			}

			fn action_kind(&self) -> &'static str {
				$kind
			}
		}

		crate::register_library_action!($action, $kind);
	};
}

agent_action!(
	AgentWriteFileAction,
	WriteFileInput,
	"agent.write_file",
	ops::write_file
);
agent_action!(
	AgentMakeDirAction,
	MakeDirInput,
	"agent.make_dir",
	ops::make_dir
);
agent_action!(AgentMoveAction, MoveInput, "agent.move", ops::move_path);
agent_action!(AgentDeleteAction, DeleteInput, "agent.delete", ops::delete);

/// A path's state inside a grant: what a write's precondition names.
pub struct AgentStatQuery {
	input: StatInput,
}

impl LibraryQuery for AgentStatQuery {
	type Input = StatInput;
	type Output = PathState;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		ops::stat(&context, self.input)
			.await
			.map_err(|error| QueryError::InvalidInput(error.to_string()))
	}
}

crate::register_library_query!(AgentStatQuery, "agent.stat");
