//! # Deep scope
//!
//! Which parts of a source get the passes that read file contents: content
//! identity, the content text index and the passes that follow them. The
//! walk and the records cover the whole source; reading the bytes of a home
//! folder's caches, toolchains, logs and agent transcripts cost tens of
//! gigabytes and hours of hashing for files nobody searches or deduplicates.
//!
//! The scope is a list of subtrees, `/`-separated paths relative to the
//! source root, where an empty path is the whole source. It is a person's
//! choice, the "Keep indexed" of `docs/core/design/zero-onboarding-startup.md`,
//! so it is an assertion: it lives in `data.db` beside the tags, never in a
//! generation table, and no rebuild touches it. One row per store, the later
//! HLC stamp winning, so two devices that set it converge the way tag
//! definitions do. A store without a row has the default for its kind of
//! source, which the caller knows and this crate does not.

use crate::db::{SourceDb, Stamp};
use crate::error::Result;

impl SourceDb {
	/// The subtrees a person kept, or `None` when no one has set them.
	pub async fn deep_scope(&self) -> Result<Option<Vec<String>>> {
		// A replica copied from a store older than this table has none.
		let row: Option<(String,)> =
			match sqlx::query_as("SELECT subtrees FROM deep_scope WHERE id = 1")
				.fetch_optional(self.pool())
				.await
			{
				Err(sqlx::Error::Database(db)) if db.message().contains("no such table") => None,
				other => other?,
			};
		row.map(|(subtrees,)| Ok(serde_json::from_str(&subtrees)?))
			.transpose()
	}

	/// Record the kept subtrees, unless the store already holds a list with
	/// a later stamp. Returns whether the row changed.
	pub async fn set_deep_scope(&self, subtrees: &[String], stamp: &Stamp) -> Result<bool> {
		let result = sqlx::query(
			"INSERT INTO deep_scope (id, subtrees, updated_hlc, origin_device)
			 VALUES (1, ?, ?, ?)
			 ON CONFLICT (id) DO UPDATE SET
				subtrees = excluded.subtrees,
				updated_hlc = excluded.updated_hlc,
				origin_device = excluded.origin_device
			 WHERE excluded.updated_hlc > deep_scope.updated_hlc",
		)
		.bind(serde_json::to_string(subtrees)?)
		.bind(&stamp.hlc)
		.bind(stamp.device_uuid)
		.execute(self.pool())
		.await?;
		Ok(result.rows_affected() > 0)
	}
}

/// The subtrees of a source whose contents are read.
///
/// A path is read when the innermost entry holding it is in `read`: a
/// folder in `skip` inside a read folder is left out, and a read folder
/// inside a skipped one is read again. That is how a space with content
/// indexing off sits inside the standard folders, and a space with it on
/// sits inside that one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeepScope {
	/// `/`-separated folders relative to the source root. An empty path
	/// is the whole source; none means no file is read.
	pub read: Vec<String>,
	pub skip: Vec<String>,
}

impl DeepScope {
	/// The whole source.
	pub fn whole() -> Self {
		Self {
			read: vec![String::new()],
			skip: Vec::new(),
		}
	}

	/// Only these folders, with nothing skipped inside them.
	pub fn only(read: &[&str]) -> Self {
		Self {
			read: read.iter().map(|s| s.to_string()).collect(),
			skip: Vec::new(),
		}
	}

	/// Whether no file is read.
	pub fn is_empty(&self) -> bool {
		self.read.is_empty()
	}

	/// Whether every file is read, so a query needs no condition and keeps
	/// its usual plan.
	pub(crate) fn is_whole(&self) -> bool {
		self.skip.is_empty() && self.read.iter().any(|s| s.trim_matches('/').is_empty())
	}

	/// A condition that holds for a record, whose parent column is
	/// `parent`, that this scope reads. A file at the source root has no
	/// parent row, so a skip test on it is false rather than null.
	pub(crate) fn condition(&self, parent: &str) -> String {
		if self.is_whole() {
			return "1".to_string();
		}
		if self.read.is_empty() {
			return "0".to_string();
		}
		let skips: Vec<&str> = self.skip.iter().map(|s| s.trim_matches('/')).collect();
		let clauses: Vec<String> = self
			.read
			.iter()
			.map(|read| {
				let read = read.trim_matches('/');
				let inside: Vec<String> = skips
					.iter()
					.filter(|skip| **skip != read && within(read, skip))
					.map(|skip| skip.to_string())
					.collect();
				let held = subtree_condition(&[read.to_string()], parent);
				if inside.is_empty() {
					held
				} else {
					format!(
						"({held} AND NOT IFNULL({}, 0))",
						subtree_condition(&inside, parent)
					)
				}
			})
			.collect();
		format!("({})", clauses.join(" OR "))
	}
}

/// Whether `inner` is `outer` or below it.
fn within(outer: &str, inner: &str) -> bool {
	outer.is_empty()
		|| inner == outer
		|| inner
			.strip_prefix(outer)
			.is_some_and(|rest| rest.starts_with('/'))
}

/// A condition that holds for a record, whose parent column is `parent`,
/// that sits in one of the folders `subtrees` names or anywhere below them.
///
/// It reads the folders' rows by path range, so a query can be driven from
/// the folders in scope rather than from every record outside it: on a home
/// folder most records are outside, and they never leave the pending set.
/// The subtrees are written as quoted literals rather than binds, so a
/// caller's query keeps its own bind order.
fn subtree_condition(subtrees: &[String], parent: &str) -> String {
	let subtrees: Vec<&str> = subtrees.iter().map(|s| s.trim_matches('/')).collect();
	if subtrees.contains(&"") {
		return "1".to_string();
	}
	if subtrees.is_empty() {
		return "0".to_string();
	}
	// Every path below a folder sorts between "folder/" and "folder0",
	// since '0' is the byte after '/'.
	let any: Vec<String> = subtrees
		.iter()
		.map(|subtree| {
			format!(
				"path = {} OR (path > {} AND path < {})",
				literal(subtree),
				literal(&format!("{subtree}/")),
				literal(&format!("{subtree}0"))
			)
		})
		.collect();
	format!(
		"{parent} IN (SELECT record_uuid FROM directory_path WHERE {})",
		any.join(" OR ")
	)
}

fn literal(text: &str) -> String {
	format!("'{}'", text.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_innermost_folder_decides() {
		let scope = DeepScope {
			read: vec!["".into(), "Documents/private/shared".into()],
			skip: vec!["Documents/private".into()],
		};
		assert_eq!(
			scope.condition("p"),
			"((1 AND NOT IFNULL(p IN (SELECT record_uuid FROM directory_path WHERE \
			 path = 'Documents/private' OR (path > 'Documents/private/' AND path < 'Documents/private0')), 0)) \
			 OR p IN (SELECT record_uuid FROM directory_path WHERE path = 'Documents/private/shared' \
			 OR (path > 'Documents/private/shared/' AND path < 'Documents/private/shared0')))"
		);
		assert_eq!(DeepScope::whole().condition("p"), "1");
		assert_eq!(DeepScope::default().condition("p"), "0");
		let skipped_elsewhere = DeepScope {
			read: vec!["Desktop".into()],
			skip: vec!["Documents".into()],
		};
		assert!(!skipped_elsewhere.condition("p").contains("NOT"));
	}

	#[test]
	fn a_subtree_matches_its_folder_and_below_and_quotes_are_literal() {
		assert_eq!(subtree_condition(&["".into(), "a".into()], "p"), "1");
		assert_eq!(subtree_condition(&[], "p"), "0");
		assert_eq!(
			subtree_condition(&["/Bob's/".into()], "r.parent_uuid"),
			"r.parent_uuid IN (SELECT record_uuid FROM directory_path WHERE \
			 path = 'Bob''s' OR (path > 'Bob''s/' AND path < 'Bob''s0'))"
		);
	}
}
