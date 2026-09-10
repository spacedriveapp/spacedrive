use super::output::LocationsListOutput;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQuery;

impl LibraryQuery for LocationsListQuery {
	type Input = LocationsListQueryInput;
	type Output = LocationsListOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self {})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let locations = crate::location::list(&library, &context)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;

		Ok(LocationsListOutput { locations })
	}
}

crate::register_library_query!(LocationsListQuery, "locations.list");
