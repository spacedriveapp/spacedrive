//! Redundancy summary query implementation

use super::{input::RedundancySummaryInput, output::RedundancySummaryOutput};
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

/// Redundancy summary query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RedundancySummaryQuery {
	pub input: RedundancySummaryInput,
}

impl LibraryQuery for RedundancySummaryQuery {
	type Input = RedundancySummaryInput;
	type Output = RedundancySummaryOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Redundancy is a property of content across every source store and
		// replica, grouped by the volume each source lives on. That
		// aggregation does not exist, and an empty summary would read as
		// nothing at risk, so the query says so instead.
		Err(QueryError::Internal(
			"redundancy is not computed over source stores".to_string(),
		))
	}
}

crate::register_library_query!(RedundancySummaryQuery, "redundancy.summary");
