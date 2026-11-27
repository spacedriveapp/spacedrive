//! The video facet: what a container header says about a file's bytes.
//!
//! Keyed and re-queued exactly like the image facet, by the hash of the
//! content it was read from, so a video is read once per set of bytes however
//! many copies a source holds, and again only when its bytes change. A row
//! whose columns are all empty records a header that named nothing, which
//! keeps those bytes out of the pending set.

use crate::error::Result;
use crate::image::{count_pending, pending, PendingFacet, CONTENT_KEY_IS};

/// What a header read yields, in the facet's own columns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoFacet {
	pub duration_seconds: Option<f64>,
	pub width: Option<i64>,
	pub height: Option<i64>,
	pub codec: Option<String>,
}

/// How many video records are still waiting. `kind` is the content kind
/// discriminant the caller stores for videos.
pub async fn count_files_needing_video_facets(pool: &sqlx::SqlitePool, kind: i64) -> Result<i64> {
	count_pending(pool, "facet_video", kind).await
}

/// Video records past `after` with no current facet row, in row order.
pub async fn files_needing_video_facets(
	pool: &sqlx::SqlitePool,
	kind: i64,
	after: i64,
	batch_size: usize,
) -> Result<Vec<PendingFacet>> {
	pending(pool, "facet_video", kind, after, batch_size).await
}

/// Write a facet row to every record holding the hashed bytes.
pub async fn set_video_facets(
	pool: &sqlx::SqlitePool,
	facets: &[(String, VideoFacet)],
) -> Result<u64> {
	let sql = format!(
		"INSERT INTO facet_video (record_uuid, content_hash, duration_seconds, width, height, codec) \
		 SELECT r.uuid, ?, ?, ?, ?, ? FROM record r JOIN content c ON c.id = r.content_id \
		 WHERE {CONTENT_KEY_IS} \
		 ON CONFLICT (record_uuid) DO UPDATE SET content_hash = excluded.content_hash, \
		 duration_seconds = excluded.duration_seconds, width = excluded.width, \
		 height = excluded.height, codec = excluded.codec"
	);
	let mut tx = pool.begin().await?;
	let mut written = 0;
	for (hash, facet) in facets {
		written += sqlx::query(&sql)
			.bind(hash)
			.bind(facet.duration_seconds)
			.bind(facet.width)
			.bind(facet.height)
			.bind(&facet.codec)
			.bind(hash)
			.bind(hash)
			.execute(&mut *tx)
			.await?
			.rows_affected();
	}
	tx.commit().await?;
	Ok(written)
}
