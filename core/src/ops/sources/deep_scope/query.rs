//! `sources.deep_scope.get`: where a source reads file contents.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetDeepScopeInput {
	pub source_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetDeepScopeOutput {
	/// Subtrees relative to the source root whose contents are read. An
	/// empty path is the whole source; an empty list is none of it.
	pub subtrees: Vec<String>,
	/// Whether a person chose the list. False means the source's default:
	/// the standard user folders for the home folder, the whole root for
	/// any other source.
	pub chosen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDeepScopeQuery {
	input: GetDeepScopeInput,
}

impl LibraryQuery for GetDeepScopeQuery {
	type Input = GetDeepScopeInput;
	type Output = GetDeepScopeOutput;

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
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let source_id = self.input.source_id;
		let not_tracked = || {
			QueryError::InvalidInput(format!("source {source_id} is not tracked on this device"))
		};
		// Only this library's sources answer; the index holds every library's.
		registry::get(library.db().conn(), source_id)
			.await
			.map_err(|_| not_tracked())?;
		let index = context.volume_index();
		let root = index.source_root(source_id).ok_or_else(not_tracked)?;
		let store = index
			.store_for(&root)
			.await
			.ok_or_else(|| QueryError::Internal("the source has no store on this device".into()))?;
		let chosen = store
			.db()
			.deep_scope()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?
			.is_some();
		let subtrees = store
			.deep_subtrees()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		Ok(GetDeepScopeOutput { subtrees, chosen })
	}
}

crate::register_library_query!(GetDeepScopeQuery, "sources.deep_scope.get");
