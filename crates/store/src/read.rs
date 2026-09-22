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
use sqlx::{FromRow, SqlitePool};
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
	pub atime_ms: Option<i64>,
	pub created_ms: Option<i64>,
	pub is_hidden: bool,
	pub extension: Option<String>,
	pub link_target: Option<String>,
	pub inode: Option<i64>,
	pub mode: Option<i64>,
	pub uid: Option<i64>,
	pub gid: Option<i64>,
	pub content_uuid: Option<Uuid>,
	pub content_kind: Option<i64>,
	pub content_error: Option<String>,
}

/// Every column [`FsEntry`] is built from, over the aliases `r` (the record),
/// `own` and `parent` (its own and its parent's `directory_path` rows), `f`
/// and `c`. A directory's path comes from its own row; a file's from its
/// parent's row plus its title, and a root-level entry from its title alone.
macro_rules! entry_columns {
	() => {
		"r.rowid AS rowid, r.uuid AS uuid, r.type AS kind, \
		 COALESCE(r.title, '') AS title, \
		 COALESCE(own.path, parent.path || '/' || r.title, COALESCE(r.title, '')) AS rel_path, \
		 f.size AS size, f.mtime AS mtime_ms, f.atime AS atime_ms, r.created_at AS created_ms, \
		 COALESCE(f.is_hidden, 0) AS is_hidden, f.extension AS extension, \
		 f.link_target AS link_target, f.inode AS inode, f.mode AS mode, f.uid AS uid, f.gid AS gid, \
		 f.content_error AS content_error, c.uuid AS content_uuid, c.kind AS content_kind"
	};
}

const ENTRY_SELECT: &str = concat!(
	"SELECT ",
	entry_columns!(),
	" FROM record r \
	 LEFT JOIN directory_path own ON own.record_uuid = r.uuid \
	 LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid \
	 LEFT JOIN facet_file f ON f.record_uuid = r.uuid \
	 LEFT JOIN content c ON c.id = r.content_id"
);

/// The same columns, driven from the directories: `CROSS JOIN` holds
/// `directory_path` as the outer loop, so rows arrive in its path index's
/// order and, within a directory, in the sibling index's title order.
const BENEATH_SELECT: &str = concat!(
	"SELECT ",
	entry_columns!(),
	" FROM directory_path parent \
	 CROSS JOIN record r ON r.parent_uuid = parent.record_uuid \
	 LEFT JOIN directory_path own ON own.record_uuid = r.uuid \
	 LEFT JOIN facet_file f ON f.record_uuid = r.uuid \
	 LEFT JOIN content c ON c.id = r.content_id"
);

#[derive(FromRow)]
struct EntryRow {
	rowid: i64,
	uuid: Uuid,
	kind: String,
	title: String,
	rel_path: String,
	size: Option<i64>,
	mtime_ms: Option<i64>,
	atime_ms: Option<i64>,
	created_ms: Option<i64>,
	is_hidden: i64,
	extension: Option<String>,
	link_target: Option<String>,
	inode: Option<i64>,
	mode: Option<i64>,
	uid: Option<i64>,
	gid: Option<i64>,
	content_error: Option<String>,
	content_uuid: Option<Uuid>,
	content_kind: Option<i64>,
}

fn entry_from_row(row: EntryRow) -> Option<FsEntry> {
	// A row of another type in a filesystem store has no entry shape to give.
	let kind = FileKind::parse(&row.kind)?;
	Some(FsEntry {
		uuid: row.uuid,
		kind,
		name: row.title,
		relative_path: row.rel_path,
		size: row.size,
		mtime_ms: row.mtime_ms,
		atime_ms: row.atime_ms,
		created_ms: row.created_ms,
		is_hidden: row.is_hidden != 0,
		extension: row.extension,
		link_target: row.link_target,
		inode: row.inode,
		mode: row.mode,
		uid: row.uid,
		gid: row.gid,
		content_uuid: row.content_uuid,
		content_kind: row.content_kind,
		content_error: row.content_error,
	})
}

/// File counts per content kind, from the content rows records point at.
/// Kinds are the store's integers; the caller owns the enum mapping.
pub async fn content_kind_counts(pool: &SqlitePool) -> Result<Vec<(i64, i64)>> {
	Ok(sqlx::query_as(
		"SELECT c.kind, COUNT(*) FROM record r JOIN content c ON c.id = r.content_id
			 WHERE c.kind IS NOT NULL GROUP BY c.kind",
	)
	.fetch_all(pool)
	.await?)
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

/// Up to `limit` files beneath the directory at `scope` (source-relative, ""
/// for the source root) at any depth, in the order of their directory's path
/// and then their name, starting after `after` as `(directory, name)`.
/// Files directly under the source root have no directory row and come
/// first, as directory "".
///
/// The order is the one the `directory_path` path index and each directory's
/// sibling index already keep, so a page reads its own rows and stops rather
/// than sorting the subtree, and the next page resumes from the last row.
/// Only files with an extension in `extensions` are read when it is given,
/// compared lowercase, and hidden files only when asked for.
pub async fn files_beneath(
	pool: &SqlitePool,
	scope: &str,
	after: Option<(&str, &str)>,
	extensions: Option<&[String]>,
	include_hidden: bool,
	limit: usize,
) -> Result<Vec<FsEntry>> {
	if limit == 0 || extensions.is_some_and(<[String]>::is_empty) {
		return Ok(Vec::new());
	}
	let mut entries = Vec::new();
	for statement in beneath_statements(scope, after, extensions, include_hidden) {
		let mut query = sqlx::query_as::<_, EntryRow>(&statement.sql);
		for value in &statement.binds {
			query = query.bind(value);
		}
		let rows = query
			.bind((limit - entries.len()) as i64)
			.fetch_all(pool)
			.await?;
		entries.extend(rows.into_iter().filter_map(entry_from_row));
		if entries.len() >= limit {
			break;
		}
	}
	Ok(entries)
}

/// One statement of a [`files_beneath`] page: its SQL, ending in `LIMIT ?`,
/// and the text it binds before the limit, in order.
struct Statement {
	sql: String,
	binds: Vec<String>,
}

/// The statements a page runs in turn: the files directly under the source
/// root, when the scope is the root and the page has not moved past them,
/// then the files in directories.
fn beneath_statements(
	scope: &str,
	after: Option<(&str, &str)>,
	extensions: Option<&[String]>,
	include_hidden: bool,
) -> Vec<Statement> {
	// `+` keeps the type term off `idx_record_type`, which would drive the
	// scan from every file in the store and sort them.
	let mut shared = vec!["+r.type = 'file'".to_string()];
	let mut shared_binds = Vec::new();
	if !include_hidden {
		shared.push("COALESCE(f.is_hidden, 0) != 1".to_string());
	}
	if let Some(extensions) = extensions {
		shared.push(format!(
			"lower(f.extension) IN ({})",
			vec!["?"; extensions.len()].join(", ")
		));
		shared_binds.extend(extensions.iter().cloned());
	}

	let mut statements = Vec::new();

	let after_directory = after.map(|(directory, _)| directory);
	if scope.is_empty() && after_directory.is_none_or(str::is_empty) {
		let mut conditions = vec!["r.parent_uuid IS NULL".to_string()];
		conditions.extend(shared.iter().cloned());
		let mut binds = shared_binds.clone();
		if let Some((_, name)) = after {
			conditions.push("r.title > ?".to_string());
			binds.push(name.to_string());
		}
		statements.push(Statement {
			sql: format!(
				"{ENTRY_SELECT} WHERE {} ORDER BY r.title LIMIT ?",
				conditions.join(" AND ")
			),
			binds,
		});
	}

	let mut conditions = Vec::new();
	let mut binds = Vec::new();
	if !scope.is_empty() {
		// `[scope, scope + "0")` holds the scope and everything beneath it,
		// since '0' is the byte after '/'. It also holds siblings that only
		// share the prefix, like "scope-2", which the second term drops.
		conditions.push("parent.path < ?".to_string());
		binds.push(format!("{scope}0"));
		conditions.push("(parent.path = ? OR parent.path >= ?)".to_string());
		binds.push(scope.to_string());
		binds.push(format!("{scope}/"));
	}
	match after.filter(|(directory, _)| !directory.is_empty()) {
		Some((directory, name)) => {
			conditions.push("parent.path >= ?".to_string());
			binds.push(directory.to_string());
			// With the path at or past the cursor's directory, a later
			// directory passes whatever the name, and the cursor's own
			// directory needs a later name.
			conditions.push("(parent.path > ? OR r.title > ?)".to_string());
			binds.push(directory.to_string());
			binds.push(name.to_string());
		}
		None if !scope.is_empty() => {
			conditions.push("parent.path >= ?".to_string());
			binds.push(scope.to_string());
		}
		None => {}
	}
	conditions.extend(shared);
	binds.extend(shared_binds);
	statements.push(Statement {
		sql: format!(
			"{BENEATH_SELECT} WHERE {} ORDER BY parent.path, r.title LIMIT ?",
			conditions.join(" AND ")
		),
		binds,
	});

	statements
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

/// One page of every filesystem entry in the store, ordered by rowid.
/// Feed the returned high-water mark back in to continue; fewer rows than
/// `limit` means the scan is done. This is what rebuilding an arena from a
/// delivered database walks.
pub async fn all_entries_page(
	pool: &SqlitePool,
	after_rowid: i64,
	limit: usize,
) -> Result<(Vec<FsEntry>, i64)> {
	let sql = format!("{ENTRY_SELECT} WHERE r.rowid > ? ORDER BY r.rowid LIMIT ?");
	let rows: Vec<EntryRow> = sqlx::query_as(&sql)
		.bind(after_rowid)
		.bind(limit as i64)
		.fetch_all(pool)
		.await?;
	let last = rows.last().map(|row| row.rowid).unwrap_or(after_rowid);
	Ok((rows.into_iter().filter_map(entry_from_row).collect(), last))
}

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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::file::filesystem_schema;
	use crate::source::SourceManager;

	/// Neither statement of a page sorts. A plan that did would read the whole
	/// subtree for every page, which is the cost the cursor exists to avoid.
	/// Plans depend on the schema alone, so an empty store answers.
	#[tokio::test]
	async fn a_page_beneath_a_directory_follows_the_indexes() {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().to_path_buf());
		manager
			.create("plan", &filesystem_schema())
			.await
			.expect("source created");
		let db = manager.open("plan").await.expect("open");
		let extensions = vec!["jpg".to_string(), "mov".to_string()];

		let pages = [
			("", None),
			("", Some(("", "a.jpg"))),
			("", Some(("2019/trip", "b.jpg"))),
			("2019", None),
			("2019", Some(("2019/trip", "b.jpg"))),
		];
		for (scope, after) in pages {
			for statement in beneath_statements(scope, after, Some(&extensions), false) {
				let sql = format!("EXPLAIN QUERY PLAN {}", statement.sql);
				let mut query = sqlx::query_as::<_, (i64, i64, i64, String)>(&sql);
				for value in &statement.binds {
					query = query.bind(value);
				}
				let plan: Vec<String> = query
					.bind(100_i64)
					.fetch_all(db.pool())
					.await
					.expect("plan")
					.into_iter()
					.map(|(_, _, _, detail)| detail)
					.collect();
				assert!(
					!plan.iter().any(|step| step.contains("TEMP B-TREE")),
					"scope {scope:?} after {after:?} sorts: {plan:?}"
				);
			}
		}
	}
}
