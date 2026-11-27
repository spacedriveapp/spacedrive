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

/// A condition that holds for a record, whose parent column is `parent`,
/// that sits in one of the folders `subtrees` names or anywhere below them.
///
/// It reads the folders' rows by path range, so a query can be driven from
/// the folders in scope rather than from every record outside it: on a home
/// folder most records are outside, and they never leave the pending set.
/// The subtrees are written as quoted literals rather than binds, so a
/// caller's query keeps its own bind order.
pub(crate) fn subtree_condition(subtrees: &[String], parent: &str) -> String {
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

/// Whether `subtrees` covers the whole source, so a query needs no
/// subtree condition and keeps its usual plan.
pub(crate) fn whole(subtrees: &[String]) -> bool {
	subtrees.iter().any(|s| s.trim_matches('/').is_empty())
}

fn literal(text: &str) -> String {
	format!("'{}'", text.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
	use super::*;

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
