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

use std::collections::HashMap;

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
	/// The two rungs of the content's hash ladder: the sampled hash every
	/// store keys the content by, and the integrity hash once every byte has
	/// been read. Two stores agree on the sampled hash for the same bytes
	/// whatever rung each has reached, which the uuid does not promise, as it
	/// re-derives from the integrity hash when that lands.
	pub sampled_hash: Option<String>,
	pub integrity_hash: Option<String>,
	pub content_kind: Option<i64>,
	pub content_error: Option<String>,
}

impl FsEntry {
	/// The directory the entry sits in, relative to the source root; "" at
	/// the root. With the name, it is the entry's place in the order
	/// [`files_beneath`] reads in.
	pub fn directory(&self) -> &str {
		self.relative_path
			.rsplit_once('/')
			.map_or("", |(directory, _)| directory)
	}
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
		 f.content_error AS content_error, c.uuid AS content_uuid, \
		 c.sampled_hash AS sampled_hash, c.integrity_hash AS integrity_hash, c.kind AS content_kind"
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
	sampled_hash: Option<String>,
	integrity_hash: Option<String>,
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
		sampled_hash: row.sampled_hash,
		integrity_hash: row.integrity_hash,
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

/// Where a page of [`files_beneath`] starts, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start<'a> {
	/// At the first file.
	First,
	/// After the file named `name` in `directory`, "" for the source root.
	After { directory: &'a str, name: &'a str },
	/// Past the files directly under the source root, at the first directory.
	/// A reader merging stores lands here when its position falls between a
	/// store's root-level files and its directories.
	Directories,
}

/// Which record kinds a read beneath a directory takes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kinds {
	/// Files alone, which is what a listing of what a folder holds wants.
	Files,
	/// Files, directories and symlinks, each in its place in the same order,
	/// for a reader that has to know what a path is as well as that it is.
	All,
}

/// Up to `limit` files beneath the directory at `scope` (source-relative, ""
/// for the source root) at any depth, in the order of their directory's path
/// and then their name, from `start`. Files directly under the source root
/// have no directory row and come first, as directory "".
///
/// The order is the one the `directory_path` path index and each directory's
/// sibling index already keep, so a page reads its own rows and stops rather
/// than sorting the subtree, and the next page resumes from the last row.
/// Only files with an extension in `extensions` are read when it is given,
/// compared lowercase, and hidden files only when asked for.
pub async fn files_beneath(
	pool: &SqlitePool,
	scope: &str,
	start: Start<'_>,
	extensions: Option<&[String]>,
	include_hidden: bool,
	limit: usize,
) -> Result<Vec<FsEntry>> {
	if extensions.is_some_and(<[String]>::is_empty) {
		return Ok(Vec::new());
	}
	beneath(
		pool,
		scope,
		start,
		extensions,
		include_hidden,
		Kinds::Files,
		limit,
	)
	.await
}

/// [`files_beneath`] for every kind of record: directories and symlinks come
/// out in their place among the files, so a reader walking two trees against
/// each other meets a directory on one side where the other has a file.
pub async fn entries_beneath(
	pool: &SqlitePool,
	scope: &str,
	start: Start<'_>,
	include_hidden: bool,
	limit: usize,
) -> Result<Vec<FsEntry>> {
	beneath(pool, scope, start, None, include_hidden, Kinds::All, limit).await
}

async fn beneath(
	pool: &SqlitePool,
	scope: &str,
	start: Start<'_>,
	extensions: Option<&[String]>,
	include_hidden: bool,
	kinds: Kinds,
	limit: usize,
) -> Result<Vec<FsEntry>> {
	if limit == 0 {
		return Ok(Vec::new());
	}
	let mut entries = Vec::new();
	for statement in beneath_statements(scope, start, extensions, include_hidden, kinds) {
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

/// Which of `contents`, by sampled hash, some file beneath the directory at
/// `scope` holds, "" for the whole source, each with the integrity hash its
/// content row carries once every byte has been read. Answered from the
/// content and record indexes a chunk at a time, so asking about a page of
/// files costs a page of lookups rather than a read of the store.
pub async fn contents_beneath(
	pool: &SqlitePool,
	contents: &[String],
	scope: &str,
) -> Result<HashMap<String, Option<String>>> {
	let mut present = HashMap::new();
	for chunk in contents.chunks(LOOKUP_CHUNK) {
		let statement = contents_statement(chunk.len(), scope);
		let mut query = sqlx::query_as::<_, (String, Option<String>)>(&statement.sql);
		for content in chunk {
			query = query.bind(content);
		}
		for value in &statement.binds {
			query = query.bind(value);
		}
		present.extend(query.fetch_all(pool).await?);
	}
	Ok(present)
}

/// One file beneath the directory at `scope` for each of `contents`, by
/// sampled hash, that some file there holds. A store keys a content by its
/// sampled hash, so every file holding one is believed to hold the same
/// bytes, and reading one of them in full settles the content for all.
pub async fn holders_beneath(
	pool: &SqlitePool,
	contents: &[String],
	scope: &str,
) -> Result<Vec<FsEntry>> {
	let mut holders = Vec::new();
	for chunk in contents.chunks(LOOKUP_CHUNK) {
		let statement = holders_statement(chunk.len(), scope);
		let mut query = sqlx::query_as::<_, EntryRow>(&statement.sql);
		for content in chunk {
			query = query.bind(content);
		}
		for value in &statement.binds {
			query = query.bind(value);
		}
		holders.extend(
			query
				.fetch_all(pool)
				.await?
				.into_iter()
				.filter_map(entry_from_row),
		);
	}
	Ok(holders)
}

/// How many records in the store hold each of `contents`, by sampled hash.
/// Summed across every store, one holder means a file is the last copy of
/// its bytes anywhere.
pub async fn content_holders(
	pool: &SqlitePool,
	contents: &[String],
) -> Result<HashMap<String, i64>> {
	let mut holders = HashMap::new();
	for chunk in contents.chunks(LOOKUP_CHUNK) {
		let sql = format!(
			"SELECT c.sampled_hash, COUNT(*) FROM content c \
			 JOIN record r ON r.content_id = c.id \
			 WHERE c.sampled_hash IN ({}) GROUP BY c.sampled_hash",
			vec!["?"; chunk.len()].join(", ")
		);
		let mut query = sqlx::query_as::<_, (String, i64)>(&sql);
		for content in chunk {
			query = query.bind(content);
		}
		holders.extend(query.fetch_all(pool).await?);
	}
	Ok(holders)
}

/// How many tag assertions stand on records beneath the directory at
/// `scope`, "" for the whole source. What a move off the volume would leave
/// behind, since assertions belong to the records of the store they are in.
pub async fn assertions_beneath(pool: &SqlitePool, scope: &str) -> Result<i64> {
	let (mut conditions, binds) = beneath_scope(scope);
	conditions.insert(0, "t.asserted = 1".to_string());
	let sql = format!(
		"SELECT COUNT(*) FROM tag_assertion t \
		 JOIN record r ON r.uuid = t.record_uuid \
		 LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid \
		 WHERE {}",
		conditions.join(" AND ")
	);
	let mut query = sqlx::query_scalar::<_, i64>(&sql);
	for value in &binds {
		query = query.bind(value);
	}
	Ok(query.fetch_one(pool).await?)
}

/// Sampled hashes one content lookup asks about, under SQLite's bind limit.
const LOOKUP_CHUNK: usize = 400;

/// The content rows among `count` sampled hashes that a file beneath `scope`
/// points at, with their integrity hashes.
fn contents_statement(count: usize, scope: &str) -> Statement {
	let (mut conditions, binds) = beneath_scope(scope);
	conditions.insert(
		0,
		format!("c.sampled_hash IN ({})", vec!["?"; count].join(", ")),
	);
	Statement {
		sql: format!(
			"SELECT DISTINCT c.sampled_hash, c.integrity_hash FROM content c \
			 JOIN record r ON r.content_id = c.id \
			 LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid \
			 WHERE {}",
			conditions.join(" AND ")
		),
		binds,
	}
}

/// The first file beneath `scope` pointing at each content row among `count`
/// sampled hashes, as an entry.
fn holders_statement(count: usize, scope: &str) -> Statement {
	let (mut conditions, binds) = beneath_scope(scope);
	conditions.insert(
		0,
		format!("c.sampled_hash IN ({})", vec!["?"; count].join(", ")),
	);
	Statement {
		sql: format!(
			"{ENTRY_SELECT} WHERE r.rowid IN (\
			 SELECT MIN(r.rowid) FROM content c \
			 JOIN record r ON r.content_id = c.id \
			 LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid \
			 WHERE {} GROUP BY c.id)",
			conditions.join(" AND ")
		),
		binds,
	}
}

/// One statement of a [`files_beneath`] page: its SQL, ending in `LIMIT ?`,
/// and the text it binds before the limit, in order.
struct Statement {
	sql: String,
	binds: Vec<String>,
}

/// The terms over `parent.path` that hold a query to the directory at `scope`
/// and everything beneath it, with their binds; none for the whole source.
/// `[scope, scope + "0")` holds the scope and its descendants, since '0' is
/// the byte after '/'. It also holds siblings that only share the prefix,
/// like "scope-2", which the second term drops.
fn beneath_scope(scope: &str) -> (Vec<String>, Vec<String>) {
	if scope.is_empty() {
		return (Vec::new(), Vec::new());
	}
	(
		vec![
			"parent.path < ?".to_string(),
			"(parent.path = ? OR parent.path >= ?)".to_string(),
		],
		vec![format!("{scope}0"), scope.to_string(), format!("{scope}/")],
	)
}

/// The statements a page runs in turn: the files directly under the source
/// root, when the scope is the root and the page has not moved past them,
/// then the files in directories.
fn beneath_statements(
	scope: &str,
	start: Start<'_>,
	extensions: Option<&[String]>,
	include_hidden: bool,
	kinds: Kinds,
) -> Vec<Statement> {
	// `+` keeps the type term off `idx_record_type`, which would drive the
	// scan from every file in the store and sort them.
	let mut shared = vec![match kinds {
		Kinds::Files => "+r.type = 'file'".to_string(),
		Kinds::All => "+r.type IN ('file', 'directory', 'symlink')".to_string(),
	}];
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

	let root_files = match start {
		Start::First => Some(None),
		Start::After {
			directory: "",
			name,
		} => Some(Some(name)),
		Start::After { .. } | Start::Directories => None,
	};
	if let (true, Some(after_name)) = (scope.is_empty(), root_files) {
		let mut conditions = vec!["r.parent_uuid IS NULL".to_string()];
		conditions.extend(shared.iter().cloned());
		let mut binds = shared_binds.clone();
		if let Some(name) = after_name {
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

	let (mut conditions, mut binds) = beneath_scope(scope);
	match start {
		Start::After { directory, name } if !directory.is_empty() => {
			conditions.push("parent.path >= ?".to_string());
			binds.push(directory.to_string());
			// With the path at or past the cursor's directory, a later
			// directory passes whatever the name, and the cursor's own
			// directory needs a later name.
			conditions.push("(parent.path > ? OR r.title > ?)".to_string());
			binds.push(directory.to_string());
			binds.push(name.to_string());
		}
		_ if !scope.is_empty() => {
			conditions.push("parent.path >= ?".to_string());
			binds.push(scope.to_string());
		}
		_ => {}
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

	/// The steps of a query's plan, as SQLite describes them.
	async fn plan(
		pool: &SqlitePool,
		sql: &str,
		binds: &[String],
		limit: Option<i64>,
	) -> Vec<String> {
		let sql = format!("EXPLAIN QUERY PLAN {sql}");
		let mut query = sqlx::query_as::<_, (i64, i64, i64, String)>(&sql);
		for value in binds {
			query = query.bind(value);
		}
		if let Some(limit) = limit {
			query = query.bind(limit);
		}
		query
			.fetch_all(pool)
			.await
			.expect("plan")
			.into_iter()
			.map(|(_, _, _, detail)| detail)
			.collect()
	}

	/// Neither statement of a page sorts, and a content lookup never scans the
	/// records. A plan that sorted would read the whole subtree for every page,
	/// which is the cost the cursor exists to avoid. Plans depend on the schema
	/// alone, so an empty store answers.
	#[tokio::test]
	async fn reads_beneath_a_directory_follow_the_indexes() {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().to_path_buf());
		manager
			.create("plan", &filesystem_schema())
			.await
			.expect("source created");
		let db = manager.open("plan").await.expect("open");
		let extensions = vec!["jpg".to_string(), "mov".to_string()];

		let pages = [
			("", Start::First),
			(
				"",
				Start::After {
					directory: "",
					name: "a.jpg",
				},
			),
			(
				"",
				Start::After {
					directory: "2019/trip",
					name: "b.jpg",
				},
			),
			("", Start::Directories),
			("2019", Start::First),
			(
				"2019",
				Start::After {
					directory: "2019/trip",
					name: "b.jpg",
				},
			),
		];
		for (scope, start) in pages {
			for statement in
				beneath_statements(scope, start, Some(&extensions), false, Kinds::Files)
					.into_iter()
					.chain(beneath_statements(scope, start, None, true, Kinds::All))
			{
				let steps = plan(db.pool(), &statement.sql, &statement.binds, Some(100)).await;
				assert!(
					!steps.iter().any(|step| step.contains("TEMP B-TREE")),
					"scope {scope:?} from {start:?} sorts: {steps:?}"
				);
			}
		}

		for statement in [contents_statement(1, "2019"), holders_statement(1, "2019")] {
			let mut binds = vec!["hash".to_string()];
			binds.extend(statement.binds);
			let steps = plan(db.pool(), &statement.sql, &binds, None).await;
			assert!(
				steps.iter().any(|step| step.contains("sampled_hash=?")),
				"a content lookup starts from the sampled hash index: {steps:?}"
			);
			assert!(
				!steps.iter().any(|step| step.starts_with("SCAN r")),
				"a content lookup never scans the records: {steps:?}"
			);
		}
	}
}
