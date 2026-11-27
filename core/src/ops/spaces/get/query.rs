//! `spaces.get`: one space by id.

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
pub struct SpaceGetQueryInput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGetQueryOutput {
	pub space: Space,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceGetQuery {
	input: SpaceGetQueryInput,
}

impl LibraryQuery for SpaceGetQuery {
	type Input = SpaceGetQueryInput;
	type Output = SpaceGetQueryOutput;

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
		let space = store::find(&library, &context, self.input.id)
			.await
			.map_err(QueryError::InvalidInput)?;
		Ok(SpaceGetQueryOutput { space })
	}
}

crate::register_library_query!(SpaceGetQuery, "spaces.get");
