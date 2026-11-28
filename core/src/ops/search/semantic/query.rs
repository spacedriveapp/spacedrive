//! # Semantic search query
//!
//! The `search.semantic` query: resolve the scope on this device, embed the
//! query, and rank the source's files by words and meaning.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::debug;

use super::{embedder, SemanticSearchHit, SemanticSearchInput, SemanticSearchOutput};
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::{
		indexing::{deep_scope::slash_path, text_extract::TextKind},
		search::{
			content::{SnippetFragment, SpokenTime},
			grep::query::{resolve_local_scope, LocalScope},
		},
	},
};

/// Tokens of context a word snippet keeps around the best match.
const SNIPPET_TOKENS: u32 = 24;

/// Search the files under a path by meaning and words, on the device that
/// owns them.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SemanticSearchQuery {
	pub input: SemanticSearchInput,
}

/// The source's store, text index and vector index for a resolved scope,
/// refusing a source that does not read file contents.
pub(super) async fn source_indexes(
	context: &Arc<CoreContext>,
	scope: &LocalScope,
) -> QueryResult<Arc<crate::ops::indexing::store::SourceStore>> {
	let volume_index = context.volume_index();
	if !volume_index
		.source_config(scope.source_id)
		.is_some_and(|config| config.identify_content)
	{
		return Err(QueryError::InvalidInput(
			"this source does not read file contents, so it has no text to search; use search.grep"
				.to_string(),
		));
	}
	volume_index
		.store_for(&scope.registered_root)
		.await
		.ok_or_else(|| QueryError::Internal("the source has no store on this device".into()))
}

impl LibraryQuery for SemanticSearchQuery {
	type Input = SemanticSearchInput;
	type Output = SemanticSearchOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		input.validate().map_err(|message| QueryError::Validation {
			field: "input".to_string(),
			message,
		})?;
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
		let embedder = embedder::resolve(&context).await.map_err(|reason| {
			QueryError::InvalidInput(format!("semantic search is unavailable: {reason}"))
		})?;
		let internal = |e: anyhow::Error| QueryError::Internal(e.to_string());
		let text = store.text_index().await.map_err(internal)?;
		let vectors = store.vector_index().await.map_err(internal)?;
		let deep = store.deep_scope().await.map_err(internal)?;

		let query = self.input.query.clone();
		let vector = {
			let embedder = embedder.clone();
			tokio::task::spawn_blocking(move || embedder.embed_query(&query))
				.await
				.map_err(|e| QueryError::Internal(e.to_string()))?
				.map_err(QueryError::Internal)?
		};
		let under = slash_path(&scope.relative);
		let limit = self.input.max_results();
		let offset = self.input.offset().map_err(QueryError::InvalidInput)?;
		debug!(scope = %under, query = %self.input.query, "search.semantic");

		let mut found = sd_store::hybrid_search(
			&text,
			&vectors,
			&sd_store::HybridSearch {
				query: self.input.query.clone(),
				vector,
				under: under.clone(),
				scope: deep.clone(),
				kinds: self
					.input
					.kinds
					.iter()
					.map(|kind| kind.as_str().to_string())
					.collect(),
				limit: limit + 1,
				offset,
				snippet_tokens: SNIPPET_TOKENS,
			},
		)
		.await
		.map_err(|e| QueryError::InvalidInput(e.to_string()))?;
		let more = found.len() > limit && offset + limit < super::input::MAX_OFFSET;
		found.truncate(limit);

		let hits = found
			.into_iter()
			.filter_map(|hit| {
				Some(SemanticSearchHit {
					path: relative_to_scope(&hit.external_id, &under),
					kind: TextKind::parse(&hit.kind)?,
					score: hit.score,
					similarity: hit.similarity,
					word_match: hit.lexical_rank.is_some(),
					snippet: hit
						.snippet
						.into_iter()
						.map(|part| SnippetFragment {
							text: part.text,
							matched: part.matched,
						})
						.collect(),
					time: SpokenTime::from_ms(hit.start_ms, hit.end_ms),
				})
			})
			.collect();
		let pending_documents = vectors
			.pending_count(&deep)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		Ok(SemanticSearchOutput {
			hits,
			next_cursor: more.then(|| (offset + limit).to_string()),
			pending_documents,
		})
	}
}

fn relative_to_scope(external_id: &str, under: &str) -> String {
	if under.is_empty() {
		return external_id.to_string();
	}
	match external_id.strip_prefix(under) {
		Some("") => external_id
			.rsplit('/')
			.next()
			.unwrap_or(external_id)
			.to_string(),
		Some(rest) => rest.trim_start_matches('/').to_string(),
		None => external_id.to_string(),
	}
}

crate::register_library_query!(SemanticSearchQuery, "search.semantic");
