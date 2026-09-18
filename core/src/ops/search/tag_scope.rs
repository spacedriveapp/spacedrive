//! Resolve a tag filter to concrete paths, once per query.
//!
//! Assertions live in source stores, so tag membership is a store question
//! answered before collection: include tags intersect to the paths carrying
//! all of them, exclude tags union to the paths carrying any. The sets are
//! absolute in each store root's spelling, which is the arena's spelling too,
//! so candidates check by exact path.
//!
//! Replica partitions drop out while a tag filter is active. Their assertion
//! state lives with their owner, and passing their hits through unfiltered
//! would be a wrong answer rather than a degraded one.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::input::TagFilter;
use crate::ops::indexing::VolumeIndex;

pub struct TagScope {
	/// Paths carrying every include tag; `None` when nothing was included.
	include: Option<HashSet<PathBuf>>,
	exclude: HashSet<PathBuf>,
}

impl TagScope {
	/// Resolve when the filter names any tag; a present-but-empty filter is
	/// no filter.
	pub async fn resolve_if_active(
		cache: &VolumeIndex,
		filter: Option<&TagFilter>,
	) -> Option<Self> {
		let filter = filter?;
		if filter.include.is_empty() && filter.exclude.is_empty() {
			return None;
		}

		let mut include: Option<HashSet<PathBuf>> = None;
		for tag in &filter.include {
			let paths = paths_with_tag(cache, *tag).await;
			include = Some(match include {
				None => paths,
				Some(current) => current.intersection(&paths).cloned().collect(),
			});
		}

		let mut exclude = HashSet::new();
		for tag in &filter.exclude {
			exclude.extend(paths_with_tag(cache, *tag).await);
		}

		Some(Self { include, exclude })
	}

	pub fn admits(&self, path: &Path) -> bool {
		if self.exclude.contains(path) {
			return false;
		}
		match &self.include {
			Some(paths) => paths.contains(path),
			None => true,
		}
	}
}

/// Every path currently carrying the tag, across all local stores, content
/// collapse included.
async fn paths_with_tag(cache: &VolumeIndex, tag: Uuid) -> HashSet<PathBuf> {
	let mut paths = HashSet::new();
	for store in cache.stores().await {
		let records = match store.db().records_with_tag(tag).await {
			Ok(records) => records,
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "tagged records unavailable");
				continue;
			}
		};
		for record in records {
			match store.db().entry_by_uuid(record).await {
				Ok(Some(entry)) => {
					paths.insert(store.root().join(&entry.relative_path));
				}
				Ok(None) => {}
				Err(error) => {
					tracing::warn!(source = %store.id(), %error, "record lookup failed")
				}
			}
		}
	}
	paths
}
