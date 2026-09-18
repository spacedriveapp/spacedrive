//! Listing for built-in collections (screenshots, screen recordings, …).
//!
//! Collections are identified at index time and stored as flags in each
//! drive's arena; this query fans out across every partition and
//! assembles the flagged entries. No pattern matching happens here.

use crate::{
	context::CoreContext,
	domain::file::File,
	infra::query::{CoreQuery, QueryError, QueryResult},
	ops::indexing::collections,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CollectionListingInput {
	/// Built-in collection slug, e.g. "screenshots".
	pub slug: String,
	/// Maximum entries returned, newest first. Defaults to 2000.
	#[serde(default)]
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CollectionListingOutput {
	pub display_name: String,
	pub files: Vec<File>,
	pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CollectionListingQuery {
	input: CollectionListingInput,
}

impl CoreQuery for CollectionListingQuery {
	type Input = CollectionListingInput;
	type Output = CollectionListingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// "recent" is a time-window collection: it decays, so it reads live
		// metadata from the arena rather than index-time flags.
		if self.input.slug == "recent" {
			return recent_listing(context, self.input.limit.unwrap_or(500) as usize).await;
		}

		let Some(mask) = collections::mask_for_slug(&self.input.slug) else {
			return Err(QueryError::InvalidInput(format!(
				"unknown collection: {}",
				self.input.slug
			)));
		};
		let display_name = collections::display_name(&self.input.slug)
			.unwrap_or(&self.input.slug)
			.to_string();
		let limit = self.input.limit.unwrap_or(2000) as usize;
		let device_slug = crate::device::get_current_device_slug();

		let cache = context.volume_index();

		// Partitions that haven't been touched this session restore from their
		// snapshots here. Every mapped drive, not only what is registered over
		// one: a machine can map several drives and keep nothing, and the
		// collections a person sees are on the drives either way.
		cache.restore_everything().await;

		let mut files: Vec<File> = Vec::new();
		let mut total = 0usize;

		for index in cache.all_indexes() {
			// Write access: returning entries assigns uuids to results that
			// never had one (volume indexing defers minting).
			let mut index = index.write().await;
			let paths = index.collection_paths(mask);
			total += paths.len();
			for path in paths {
				// Lens: captures living inside a package (a Photos library's
				// originals) surface through their source, not collections.
				if crate::ops::indexing::lens::is_bundle_internal(&path) {
					total -= 1;
					continue;
				}
				let Some(metadata) = index.get_entry_ref(&path) else {
					continue;
				};
				let entry_uuid = index.get_or_assign_uuid(&path);
				let content_kind = index.get_content_kind(&path);
				let sd_path = crate::domain::addressing::SdPath::Physical {
					device_slug: device_slug.clone(),
					path: path.clone(),
				};
				let mut file = File::from_arena(entry_uuid, &metadata, sd_path);
				file.content_kind = content_kind;
				files.push(file);
			}
		}

		// Newest first; the natural order for captures.
		files.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
		files.truncate(limit);

		Ok(CollectionListingOutput {
			display_name,
			files,
			total_count: total as u32,
		})
	}
}

async fn recent_listing(
	context: Arc<CoreContext>,
	limit: usize,
) -> QueryResult<CollectionListingOutput> {
	let device_slug = crate::device::get_current_device_slug();
	let cache = context.volume_index();
	cache.restore_everything().await;

	let mut files: Vec<File> = Vec::new();
	for index in cache.all_indexes() {
		let mut index = index.write().await;
		// Each partition contributes its own newest `limit`; the merged set
		// is re-sorted and truncated below.
		let recent = index.recent_files(limit);
		for (path, metadata) in recent {
			if metadata.is_hidden || crate::ops::indexing::lens::is_bundle_internal(&path) {
				continue;
			}
			let entry_uuid = index.get_or_assign_uuid(&path);
			let content_kind = index.get_content_kind(&path);
			let sd_path = crate::domain::addressing::SdPath::Physical {
				device_slug: device_slug.clone(),
				path,
			};
			let mut file = File::from_arena(entry_uuid, &metadata, sd_path);
			file.content_kind = content_kind;
			files.push(file);
		}
	}

	files.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
	let total = files.len() as u32;
	files.truncate(limit);

	Ok(CollectionListingOutput {
		display_name: "Recents".to_string(),
		files,
		total_count: total,
	})
}

crate::register_core_query!(CollectionListingQuery, "files.collection_listing");
