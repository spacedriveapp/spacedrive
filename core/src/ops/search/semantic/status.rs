//! # Semantic search status
//!
//! `search.semantic.status`: whether semantic search runs for a source and
//! how far its vectors are. The explorer asks before offering the mode.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{embedder, query::source_indexes, SemanticStatusInput, SemanticStatusOutput};
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::search::grep::query::resolve_local_scope,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SemanticStatusQuery {
	pub input: SemanticStatusInput,
}

impl LibraryQuery for SemanticStatusQuery {
	type Input = SemanticStatusInput;
	type Output = SemanticStatusOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let scope =
			resolve_local_scope(&context, &session, &self.input.scope, "semantic search").await?;
		let store = source_indexes(&context, &scope).await?;
		let internal = |e: anyhow::Error| QueryError::Internal(e.to_string());
		let vectors = store.vector_index().await.map_err(internal)?;
		let subtrees = store.deep_subtrees().await.map_err(internal)?;
		let counts = vectors
			.counts()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		let pending_documents = vectors
			.pending_count(&subtrees)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		let size_bytes = tokio::fs::metadata(vectors.path())
			.await
			.map(|meta| meta.len())
			.unwrap_or(0);
		let reason = (!embedder::installed(&context).await).then(|| {
			if cfg!(feature = "ai-local") {
				embedder::NO_MODEL.to_string()
			} else {
				embedder::NO_LOCAL_INFERENCE.to_string()
			}
		});
		Ok(SemanticStatusOutput {
			available: reason.is_none(),
			reason,
			model: counts.model,
			documents: counts.documents,
			chunks: counts.chunks,
			pending_documents,
			size_bytes,
		})
	}
}

crate::register_library_query!(SemanticStatusQuery, "search.semantic.status");
