//! `spaces.list`: every space in the library, by name.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::Space,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::spaces::store,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpacesListQueryInput {
	/// Only the spaces in this source.
	pub source_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpacesListQueryOutput {
	pub spaces: Vec<Space>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacesListQuery {
	input: SpacesListQueryInput,
}

impl LibraryQuery for SpacesListQuery {
	type Input = SpacesListQueryInput;
	type Output = SpacesListQueryOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
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
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let spaces = store::all(&library, &context)
			.await
			.map_err(QueryError::Internal)?
			.into_iter()
			.filter(|space| self.input.source_id.is_none_or(|id| space.source_id == id))
			.collect();
		Ok(SpacesListQueryOutput { spaces })
	}
}

crate::register_library_query!(SpacesListQuery, "spaces.list");
