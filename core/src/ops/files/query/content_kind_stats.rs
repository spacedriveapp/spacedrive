//! Query to get content kind statistics
//!
//! File counts grouped by content kind, summed across every local source
//! store. Kinds live on content rows, so the count covers hashed files; a
//! file whose bytes are not identified yet has no kind to count. An
//! extension kind counts under its own name beside the built-in kinds, with
//! its parent as `kind`, whether or not the extension is loaded.

use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, domain::ContentKind, infra::query::LibraryQuery};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for content kind statistics query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentKindStatsInput {}

/// A single content kind with its file count
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentKindStat {
	/// The content kind (image, video, audio, etc.); the parent for an
	/// extension kind
	pub kind: ContentKind,
	/// The name of the content kind: the built-in name, or the extension
	/// kind's id (`<extension id>:<kind>`)
	pub name: String,
	/// The number of files with this content kind
	pub file_count: i64,
}

/// Output containing content kind statistics
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentKindStatsOutput {
	/// Statistics for each content kind
	pub stats: Vec<ContentKindStat>,
	/// Total number of files across all content kinds
	pub total_files: i64,
}

/// Query to get content kind statistics
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentKindStatsQuery {
	pub input: ContentKindStatsInput,
}

impl ContentKindStatsQuery {
	pub fn new() -> Self {
		Self {
			input: ContentKindStatsInput {},
		}
	}
}

impl LibraryQuery for ContentKindStatsQuery {
	type Input = ContentKindStatsInput;
	type Output = ContentKindStatsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let cache = context.volume_index();

		let mut by_kind: std::collections::HashMap<(i64, Option<String>), i64> =
			std::collections::HashMap::new();
		for store in cache.stores().await {
			match sd_store::read::content_kind_counts(store.db().pool()).await {
				Ok(counts) => {
					for (kind, kind_name, count) in counts {
						*by_kind.entry((kind, kind_name)).or_default() += count;
					}
				}
				Err(error) => {
					tracing::warn!(source = %store.id(), %error, "content kind counts unavailable")
				}
			}
		}

		let mut stats: Vec<ContentKindStat> = by_kind
			.into_iter()
			.filter_map(|((kind, kind_name), count)| {
				let kind = ContentKind::try_from(kind as i32).ok()?;
				Some(ContentKindStat {
					name: kind_name.unwrap_or_else(|| kind.to_string()),
					kind,
					file_count: count,
				})
			})
			.collect();
		stats.sort_by(|a, b| b.file_count.cmp(&a.file_count).then(a.name.cmp(&b.name)));
		let total_files = stats.iter().map(|s| s.file_count).sum();

		Ok(ContentKindStatsOutput { stats, total_files })
	}
}

crate::register_library_query!(ContentKindStatsQuery, "files.content_kind_stats");
