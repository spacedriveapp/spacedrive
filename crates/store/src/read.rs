//! Read a source's records without its writer.
//!
//! The arena is optional acceleration; these queries are the floor under it.
//! A retained source answers listings, lookups and name search straight from
//! its SQLite file, whether or not anything loaded an index for it, through
//! [`crate::source::SourceManager::open_read_only`] — which creates nothing,
//! migrates nothing, loads no [`crate::file::Ledger`], and cannot write.
//!
//! Name search preserves the arena's Unicode case-folded substring semantics
//! by folding titles in Rust over a keyset scan. That is the documented
//! baseline the reliability plan asks to benchmark a candidate index against;
//! SQLite's own `lower()` and `LIKE` fold ASCII only and would silently miss
//! case pairs outside it.

use crate::error::Result;
use crate::file::FileKind;
use sqlx::SqlitePool;
use uuid::Uuid;

/// One filesystem entry as the store retains it: identity, kind, name, its
/// source-relative path, the facet metadata, and content identity when
/// hashing has reached the bytes.
#[derive(Debug, Clone)]
pub struct FsEntry {
	pub uuid: Uuid,
	pub kind: FileKind,
	pub name: String,
	/// Relative to the source root. A directory carries its own path row; a
	/// file is addressed through its parent's row plus its name.
	pub relative_path: String,
	pub size: Option<i64>,
	/// Unix milliseconds, as the walk writes them.
	pub mtime_ms: Option<i64>,
	pub created_ms: Option<i64>,
	pub is_hidden: bool,
	pub extension: Option<String>,
	pub link_target: Option<String>,
	pub content_uuid: Option<Uuid>,
	pub content_kind: Option<i64>,
	pub content_error: Option<String>,
}

/// Every column [`FsEntry`] is built from. A directory's path comes from its
/// own `directory_path` row; a file's from its parent's row plus its title,
/// and a root-level entry from its title alone.
const ENTRY_SELECT: &str = "SELECT r.uuid, r.type, COALESCE(r.title, '') AS title, \
	 COALESCE(own.path, parent.path || '/' || r.title, COALESCE(r.title, '')) AS rel_path, \
	 f.size, f.mtime, r.created_at, COALESCE(f.is_hidden, 0) AS is_hidden, \
	 f.extension, f.link_target, f.content_error, c.uuid AS content_uuid, c.kind AS content_kind \
	 FROM record r \
	 LEFT JOIN directory_path own ON own.record_uuid = r.uuid \
	 LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid \
	 LEFT JOIN facet_file f ON f.record_uuid = r.uuid \
	 LEFT JOIN content c ON c.id = r.content_id";

type EntryRow = (
	Uuid,
	String,
	String,
	String,
	Option<i64>,
	Option<i64>,
	Option<i64>,
	i64,
	Option<String>,
	Option<String>,
	Option<String>,
	Option<Uuid>,
	Option<i64>,
);

fn entry_from_row(row: EntryRow) -> Option<FsEntry> {
	let (
		uuid,
		type_,
		title,
		relative_path,
		size,
		mtime_ms,
		created_ms,
		is_hidden,
		extension,
		link_target,
		content_error,
		content_uuid,
		content_kind,
	) = row;
	// A row of another type in a filesystem store has no entry shape to give.
	let kind = FileKind::parse(&type_)?;
	Some(FsEntry {
		uuid,
		kind,
		name: title,
		relative_path,
		size,
		mtime_ms,
		created_ms,
		is_hidden: is_hidden != 0,
		extension,
		link_target,
		content_uuid,
		content_kind,
		content_error,
	})
}

/// The record at a source-relative path.
///
/// Two probes and no tree walk, which is what keeping paths on directories
/// buys. A directory answers from its own row. A file is its parent's
/// directory row plus its name, so `a/b/c/d.png` is one lookup for `a/b/c`
/// and one for `d.png` beneath it.
pub async fn resolve_path(pool: &SqlitePool, path: &str) -> Result<Option<Uuid>> {
	let directory: Option<(Uuid,)> =
		sqlx::query_as("SELECT record_uuid FROM directory_path WHERE path = ?")
			.bind(path)
			.fetch_optional(pool)
			.await?;
	if let Some((uuid,)) = directory {
		return Ok(Some(uuid));
	}

	let found: Option<(Uuid,)> = match path.rsplit_once('/') {
		Some((parent, name)) => {
			sqlx::query_as(
				"SELECT r.uuid FROM record r
				   JOIN directory_path d ON d.record_uuid = r.parent_uuid
				  WHERE d.path = ? AND r.title = ?",
			)
			.bind(parent)
			.bind(name)
			.fetch_optional(pool)
			.await?
		}
		// Directly under the source root, which has no directory row.
		None => {
			sqlx::query_as("SELECT uuid FROM record WHERE parent_uuid IS NULL AND title = ?")
				.bind(path)
				.fetch_optional(pool)
				.await?
		}
	};

	Ok(found.map(|(uuid,)| uuid))
}

/// One entry by its record uuid.
pub async fn entry_by_uuid(pool: &SqlitePool, uuid: Uuid) -> Result<Option<FsEntry>> {
	let sql = format!("{ENTRY_SELECT} WHERE r.uuid = ?");
	let row: Option<EntryRow> = sqlx::query_as(&sql).bind(uuid).fetch_optional(pool).await?;
	Ok(row.and_then(entry_from_row))
}

/// One entry by its source-relative path.
pub async fn entry_by_path(pool: &SqlitePool, path: &str) -> Result<Option<FsEntry>> {
	match resolve_path(pool, path).await? {
		Some(uuid) => entry_by_uuid(pool, uuid).await,
		None => Ok(None),
	}
}

/// Entries for a batch of record uuids, in no particular order. Chunked under
/// SQLite's bind limit; callers that care about order re-map by uuid.
pub async fn entries_by_uuids(pool: &SqlitePool, uuids: &[Uuid]) -> Result<Vec<FsEntry>> {
	let mut entries = Vec::with_capacity(uuids.len());
	for chunk in uuids.chunks(400) {
		let placeholders = vec!["?"; chunk.len()].join(", ");
		let sql = format!("{ENTRY_SELECT} WHERE r.uuid IN ({placeholders})");
		let mut query = sqlx::query_as::<_, EntryRow>(&sql);
		for uuid in chunk {
			query = query.bind(uuid);
		}
		entries.extend(
			query
				.fetch_all(pool)
				.await?
				.into_iter()
				.filter_map(entry_from_row),
		);
	}
	Ok(entries)
}

/// Direct children of a directory, by the parent's record uuid; `None` lists
/// the source root. Ordered by name for a stable listing.
pub async fn children_of(
	pool: &SqlitePool,
	parent: Option<Uuid>,
	include_hidden: bool,
) -> Result<Vec<FsEntry>> {
	let mut sql = format!(
		"{ENTRY_SELECT} WHERE ((?1 IS NULL AND r.parent_uuid IS NULL) OR r.parent_uuid = ?1)"
	);
	if !include_hidden {
		sql.push_str(" AND COALESCE(f.is_hidden, 0) != 1");
	}
	sql.push_str(" ORDER BY title");

	let rows: Vec<EntryRow> = sqlx::query_as(&sql).bind(parent).fetch_all(pool).await?;
	Ok(rows.into_iter().filter_map(entry_from_row).collect())
}

/// What a title search returns: the hydrated matches up to the cap, and the
/// exact total past it. The total keeps counting after hydration stops, so a
/// capped page still reports how much it stands for.
#[derive(Debug)]
pub struct TitleMatches {
	pub entries: Vec<FsEntry>,
	pub total: u64,
	pub truncated: bool,
}

/// How many rows one scan batch carries. Large enough that a millions-row
/// store is a few hundred round trips, small enough that a batch is cheap.
const SCAN_BATCH: i64 = 10_000;

/// Case-folded substring search over record titles.
///
/// Folding happens in Rust so the semantics match the arena's registry
/// exactly; the scan pages by rowid so memory stays bounded by the batch and
/// the matches, never by the store.
pub async fn search_titles(pool: &SqlitePool, needle: &str, cap: usize) -> Result<TitleMatches> {
	let needle = needle.to_lowercase();
	let mut matched: Vec<Uuid> = Vec::new();
	let mut total = 0u64;
	let mut after_rowid = 0i64;

	loop {
		let rows: Vec<(i64, Uuid, String)> = sqlx::query_as(
			"SELECT rowid, uuid, COALESCE(title, '') FROM record
			 WHERE rowid > ? ORDER BY rowid LIMIT ?",
		)
		.bind(after_rowid)
		.bind(SCAN_BATCH)
		.fetch_all(pool)
		.await?;

		let Some((last_rowid, _, _)) = rows.last() else {
			break;
		};
		after_rowid = *last_rowid;

		for (_, uuid, title) in &rows {
			if title.to_lowercase().contains(&needle) {
				total += 1;
				if matched.len() < cap {
					matched.push(*uuid);
				}
			}
		}

		if rows.len() < SCAN_BATCH as usize {
			break;
		}
	}

	let truncated = (matched.len() as u64) < total;
	let entries = entries_by_uuids(pool, &matched).await?;
	Ok(TitleMatches {
		entries,
		total,
		truncated,
	})
}
