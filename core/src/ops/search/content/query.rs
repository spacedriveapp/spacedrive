//! The `search.content` query: resolve the scope on this device, then ask
//! the source's content text index.

use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::debug;

use super::{
	ContentSearchHit, ContentSearchInput, ContentSearchOutput, SnippetFragment, SpokenTime,
};
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::{
		indexing::text_extract::TextKind,
		search::grep::query::{resolve_local_scope, LocalScope},
	},
};

/// Tokens of context a snippet keeps around the best match.
const SNIPPET_TOKENS: u32 = 24;

/// Search the indexed text of files under a path, on the device that owns
/// them.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentSearchQuery {
	pub input: ContentSearchInput,
}

impl LibraryQuery for ContentSearchQuery {
	type Input = ContentSearchInput;
	type Output = ContentSearchOutput;

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
		let LocalScope {
			source_id,
			registered_root,
			relative,
			..
		} = resolve_local_scope(&context, &session, &self.input.scope, "content search").await?;

		let volume_index = context.volume_index();
		if !volume_index
			.source_config(source_id)
			.is_some_and(|config| config.identify_content)
		{
			return Err(QueryError::InvalidInput(
				"this source does not read file contents, so it has no text index; use search.grep"
					.to_string(),
			));
		}
		let store = volume_index
			.store_for(&registered_root)
			.await
			.ok_or_else(|| QueryError::Internal("the source has no store on this device".into()))?;
		let index = store
			.text_index()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;

		let under = slash_path(&relative);
		let limit = self.input.max_results();
		let offset = self.input.offset().map_err(QueryError::InvalidInput)?;
		debug!(scope = %under, query = %self.input.query, "search.content");

		let search = sd_store::TextSearch {
			expression: sd_store::match_expression(&self.input.query),
			under: under.clone(),
			kinds: self
				.input
				.kinds
				.iter()
				.map(|kind| kind.as_str().to_string())
				.collect(),
			limit: limit + 1,
			offset,
			snippet_tokens: SNIPPET_TOKENS,
		};
		let mut found = index
			.search(&search)
			.await
			.map_err(|e| QueryError::InvalidInput(e.to_string()))?;
		let more = found.len() > limit;
		found.truncate(limit);

		let hits = found
			.into_iter()
			.filter_map(|hit| {
				Some(ContentSearchHit {
					path: relative_to_scope(&hit.external_id, &under),
					kind: TextKind::parse(&hit.kind)?,
					score: hit.score,
					snippet: hit
						.snippet
						.into_iter()
						.map(|part| SnippetFragment {
							text: part.text,
							matched: part.matched,
						})
						.collect(),
					truncated: hit.truncated,
					time: SpokenTime::from_ms(hit.start_ms, hit.end_ms),
				})
			})
			.collect();
		let pending_files = index
			.pending_count_under(&TextKind::extensions(), &under)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;

		Ok(ContentSearchOutput {
			hits,
			next_cursor: more.then(|| (offset + limit).to_string()),
			pending_files,
		})
	}
}

/// A relative path in the store's spelling: `/`-separated, whatever the
/// platform.
fn slash_path(path: &Path) -> String {
	path.components()
		.map(|component| component.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
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

crate::register_library_query!(ContentSearchQuery, "search.content");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hit_paths_read_relative_to_the_scope() {
		assert_eq!(relative_to_scope("a/b/c.md", ""), "a/b/c.md");
		assert_eq!(relative_to_scope("a/b/c.md", "a"), "b/c.md");
		assert_eq!(relative_to_scope("a/b/c.md", "a/b/c.md"), "c.md");
	}
}
