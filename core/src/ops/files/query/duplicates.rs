//! Files this machine holds more than one copy of.
//!
//! Grouped by content identity rather than by name or size, so a file renamed on
//! the way to its second home is still the same bytes and a coincidence of size
//! is not. The identity is derived from the bytes, so two sources agree about it
//! without ever comparing notes, and a group can span drives.
//!
//! What this cannot see yet is the file that exists exactly once here and once
//! on another drive. Each store answers about its own records, so a group has to
//! be duplicated *somewhere* before it can be noticed at all. Finding the rest
//! means asking each store which of the other stores' content ids it holds.

use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::query::{CoreQuery, QueryResult},
	ops::indexing::DuplicateCopy,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Groups asked of each source. Merging happens afterwards, so the answer can
/// hold fewer groups than this and never more.
const MAX_GROUPS: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DuplicatesInput {
	/// Only consider files at least this large. Small files collide in ways
	/// nobody wants to act on: every empty config file in a project is a
	/// duplicate and none of them is worth deleting.
	#[serde(default)]
	pub min_size: Option<u64>,
	/// Groups to return, largest first. Clamped to 500.
	#[serde(default)]
	pub limit: Option<u32>,
	/// Restrict to one source, rather than everything this machine keeps.
	#[serde(default)]
	pub source: Option<Uuid>,
}

/// One copy of some bytes.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DuplicateCopyInfo {
	pub source: Uuid,
	pub record: Uuid,
	pub path: PathBuf,
	/// The same place, addressed on this device, for an action to take.
	pub sd_path: SdPath,
}

/// Bytes that exist in more than one place.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DuplicateGroup {
	/// Derived from the bytes, so it is the same id on every machine that has
	/// seen this file.
	pub content: Uuid,
	pub size: u64,
	pub copies: Vec<DuplicateCopyInfo>,
	/// What keeping one copy instead of all of them would give back.
	pub reclaimable: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DuplicatesOutput {
	pub groups: Vec<DuplicateGroup>,
	/// Total across the returned groups.
	pub reclaimable: u64,
	/// Sources that answered. A detached drive is not one of them, so a small
	/// answer may mean a drive is unplugged rather than a tidy machine.
	pub sources_queried: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DuplicatesQuery {
	input: DuplicatesInput,
}

impl CoreQuery for DuplicatesQuery {
	type Input = DuplicatesInput;
	type Output = DuplicatesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let min_size = self.input.min_size.unwrap_or(0);
		let limit = self.input.limit.unwrap_or(50).min(MAX_GROUPS) as usize;

		let cache = context.volume_index();
		let mut groups: HashMap<Uuid, DuplicateGroup> = HashMap::new();
		let mut sources_queried = 0;

		for source in cache.sources() {
			if self.input.source.is_some_and(|only| only != source.id) {
				continue;
			}
			// A drive in a drawer has records and no files. Reporting them as
			// duplicates would invite someone to delete the copy they can see.
			if !source.attached {
				continue;
			}
			let Some(store) = cache.store_for(&source.root).await else {
				continue;
			};

			sources_queried += 1;
			for copy in store.duplicates(min_size, limit).await {
				merge(&mut groups, source.id, copy);
			}
		}

		let mut groups: Vec<DuplicateGroup> = groups
			.into_values()
			// A group can arrive from one store with a single copy in it, once
			// the same bytes turn out to be duplicated in another. One copy is
			// not a duplicate.
			.filter(|group| group.copies.len() > 1)
			.collect();

		for group in &mut groups {
			group.copies.sort_by(|a, b| a.path.cmp(&b.path));
			group.reclaimable = group.size * (group.copies.len() as u64 - 1);
		}

		groups.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable));
		groups.truncate(limit);

		Ok(DuplicatesOutput {
			reclaimable: groups.iter().map(|group| group.reclaimable).sum(),
			groups,
			sources_queried,
		})
	}
}

/// Fold one copy into the group for its bytes, wherever that group came from.
fn merge(groups: &mut HashMap<Uuid, DuplicateGroup>, source: Uuid, copy: DuplicateCopy) {
	let group = groups
		.entry(copy.content_uuid)
		.or_insert_with(|| DuplicateGroup {
			content: copy.content_uuid,
			size: copy.size,
			copies: Vec::new(),
			reclaimable: 0,
		});

	group.copies.push(DuplicateCopyInfo {
		source,
		record: copy.record_uuid,
		sd_path: SdPath::local(&copy.path),
		path: copy.path,
	});
}

crate::register_core_query!(DuplicatesQuery, "files.duplicates");

#[cfg(test)]
mod tests {
	use super::*;

	fn copy(content: Uuid, path: &str, size: u64) -> DuplicateCopy {
		DuplicateCopy {
			content_uuid: content,
			size,
			record_uuid: Uuid::now_v7(),
			path: PathBuf::from(path),
		}
	}

	#[test]
	fn copies_of_the_same_bytes_merge_across_sources() {
		let content = Uuid::now_v7();
		let (one, two) = (Uuid::now_v7(), Uuid::now_v7());
		let mut groups = HashMap::new();

		merge(&mut groups, one, copy(content, "/a/holiday.jpg", 1_000));
		merge(&mut groups, one, copy(content, "/a/copy.jpg", 1_000));
		merge(&mut groups, two, copy(content, "/b/holiday.jpg", 1_000));

		let group = &groups[&content];
		assert_eq!(group.copies.len(), 3);
		assert_eq!(
			group
				.copies
				.iter()
				.filter(|copy| copy.source == two)
				.count(),
			1,
			"the drive that holds one copy is still part of the group"
		);
	}

	#[test]
	fn different_bytes_stay_apart() {
		let mut groups = HashMap::new();
		let source = Uuid::now_v7();

		merge(&mut groups, source, copy(Uuid::now_v7(), "/a/one", 10));
		merge(&mut groups, source, copy(Uuid::now_v7(), "/a/two", 10));

		assert_eq!(groups.len(), 2, "same size is not the same file");
	}
}
