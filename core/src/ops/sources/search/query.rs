//! Full-text search across archive sources.
//!
//! Exposes the archive engine's search router (per-source FTS5 with rank
//! merging) as an op, so imported notes, mail, and history answer queries
//! the moment their sync completes.

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceSearchInput {
	pub query: String,
	/// Restrict to one source.
	#[serde(default)]
	pub source_id: Option<String>,
	/// Restrict to one data type ("note", "email", …).
	#[serde(default)]
	pub data_type: Option<String>,
	#[serde(default)]
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceSearchResult {
	pub id: String,
	pub external_id: String,
	pub record_type: String,
	pub title: String,
	pub preview: String,
	pub subtitle: Option<String>,
	pub snippet: Option<String>,
	pub rank: f64,
	pub source_id: String,
	pub source_name: String,
	pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchQuery {
	pub input: SourceSearchInput,
}

impl LibraryQuery for SourceSearchQuery {
	type Input = SourceSearchInput;
	type Output = Vec<SourceSearchResult>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.query.trim().is_empty() {
			return Err(QueryError::Validation {
				field: "query".to_string(),
				message: "query cannot be empty".to_string(),
			});
		}
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
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}
		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		let filter = sd_archive::SearchFilter {
			source_id: self.input.source_id,
			data_type: self.input.data_type,
			limit: self.input.limit.map(|l| l as usize),
			date_after: None,
			date_before: None,
			sort_by_date: false,
		};

		let results = source_manager
			.search(&self.input.query, Some(filter))
			.await
			.map_err(QueryError::Internal)?;

		Ok(results
			.into_iter()
			.map(|r| SourceSearchResult {
				id: r.id,
				external_id: r.external_id,
				record_type: r.record_type,
				title: r.title,
				preview: r.preview,
				subtitle: r.subtitle,
				snippet: r.snippet,
				rank: r.rank,
				source_id: r.source_id,
				source_name: r.source_name,
				data_type: r.data_type,
			})
			.collect())
	}
}

crate::register_library_query!(SourceSearchQuery, "sources.search");
