//! `cloud.oauth.poll` — read the current status of an OAuth flow.
//!
//! Registered as a library query because the UI needs to observe progress
//! without mutating state. The query deliberately does not remove completed
//! flows from the store — the frontend may poll twice (double-click, retry),
//! so we rely on the janitor's terminal grace period to evict instead.

use super::super::flow::OauthFlowStatus;
use crate::{
	context::CoreContext,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for `cloud.oauth.poll`: the flow id issued by `cloud.oauth.start`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PollInput {
	pub flow_id: Uuid,
}

/// Output for `cloud.oauth.poll` — the flow's current status.
///
/// A thin wrapper around [`OauthFlowStatus`] so TypeScript gets a named type.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PollOutput {
	pub status: OauthFlowStatus,
}

/// Library query entry point.
pub struct CloudOauthPollQuery {
	input: PollInput,
}

impl LibraryQuery for CloudOauthPollQuery {
	type Input = PollInput;
	type Output = PollOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: SessionContext,
	) -> QueryResult<Self::Output> {
		let status = context
			.oauth_flows
			.status(&self.input.flow_id)
			.ok_or_else(|| {
				QueryError::InvalidInput(format!("unknown oauth flow {}", self.input.flow_id))
			})?;
		Ok(PollOutput { status })
	}
}

crate::register_library_query!(CloudOauthPollQuery, "cloud.oauth.poll");
