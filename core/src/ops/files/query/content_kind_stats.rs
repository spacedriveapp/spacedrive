//! Query to get content kind statistics
//!
//! This query returns file counts grouped by content kind (image, video, audio, etc.).
//! The counts are pre-calculated and stored in the content_kinds table by the statistics
//! recalculation system, making this query very efficient.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext, domain::ContentKind, infra::db::entities::content_kind,
	infra::query::LibraryQuery,
};
use sea_orm::{EntityTrait, Order, QueryOrder};
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
	/// The content kind (image, video, audio, etc.)
	pub kind: ContentKind,
	/// The name of the content kind
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

		let db = library.db();

		// Fetch all content kinds with their file counts
		let content_kinds = content_kind::Entity::find()
			.order_by(content_kind::Column::Id, Order::Asc)
			.all(db.conn())
			.await?;

		let mut stats = Vec::new();
		let mut total_files = 0i64;

		for ck in content_kinds {
			let kind = ContentKind::from_id(ck.id);
			let file_count = ck.file_count;
			total_files += file_count;

			stats.push(ContentKindStat {
				kind,
				name: ck.name,
				file_count,
			});
		}

		Ok(ContentKindStatsOutput { stats, total_files })
	}
}

// Register the query
crate::register_library_query!(ContentKindStatsQuery, "files.content_kind_stats");
