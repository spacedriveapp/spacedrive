//! Media in a directory, for the gallery views.
//!
//! Reads the volume index rather than walking the filesystem: the arena
//! already knows every entry under the drive and what kind each one is, so a
//! camera roll over a deep tree costs a traversal of memory rather than of
//! disk.

use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, file::File, ContentKind},
	ops::indexing::state::EntryKind,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, path::PathBuf, sync::Arc};

/// Input for media listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingInput {
	/// The directory path to list media for
	pub path: SdPath,
	/// Whether to include media from descendant directories (default: false)
	pub include_descendants: Option<bool>,
	/// Which media types to include (default: both Image and Video)
	pub media_types: Option<Vec<ContentKind>>,
	/// Optional limit on number of results (default: 1000)
	pub limit: Option<u32>,
	/// Sort order for results
	pub sort_by: MediaSortBy,
}

/// Sort options for media listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum MediaSortBy {
	/// Sort by modification date (newest first)
	Modified,
	/// Sort by creation date (newest first)
	Created,
	/// Sort by date taken/captured (newest first)
	DateTaken,
	/// Sort by name (alphabetical)
	Name,
	/// Sort by size (largest first)
	Size,
}

/// Output containing media files
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingOutput {
	/// Media files (images/videos)
	pub files: Vec<File>,
	/// Total count of media files found
	pub total_count: u32,
	/// Whether there are more results than returned
	pub has_more: bool,
}

/// Query to list media content
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingQuery {
	pub input: MediaListingInput,
}

impl MediaListingQuery {
	pub fn new(path: SdPath) -> Self {
		Self {
			input: MediaListingInput {
				path,
				include_descendants: Some(false),
				media_types: Some(vec![ContentKind::Image, ContentKind::Video]),
				limit: Some(1000),
				sort_by: MediaSortBy::DateTaken,
			},
		}
	}

	pub fn with_options(
		path: SdPath,
		include_descendants: Option<bool>,
		media_types: Option<Vec<ContentKind>>,
		limit: Option<u32>,
		sort_by: MediaSortBy,
	) -> Self {
		Self {
			input: MediaListingInput {
				path,
				include_descendants,
				media_types,
				limit,
				sort_by,
			},
		}
	}
}

impl LibraryQuery for MediaListingQuery {
	type Input = MediaListingInput;
	type Output = MediaListingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let SdPath::Physical { device_slug, path } = &self.input.path else {
			return Err(QueryError::Internal(
				"Media listing needs a physical path".to_string(),
			));
		};

		let wanted = self
			.input
			.media_types
			.clone()
			.unwrap_or_else(|| vec![ContentKind::Image, ContentKind::Video]);

		let cache = context.volume_index();
		cache.ensure_restored(path).await;

		let Some(index) = cache.get_for_search(path) else {
			return Ok(MediaListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		};

		let recurse = self.input.include_descendants.unwrap_or(false);
		let mut files = Vec::new();
		let mut queue = VecDeque::from([path.clone()]);
		let mut index = index.write().await;

		while let Some(dir) = queue.pop_front() {
			let Some(children) = index.list_directory(&dir) else {
				continue;
			};

			for child in children {
				let Some(metadata) = index.get_entry_ref(&child) else {
					continue;
				};

				if metadata.kind == EntryKind::Directory {
					if recurse {
						queue.push_back(child);
					}
					continue;
				}

				// A summarised directory holds a count and no children, so
				// recursing into one yields nothing rather than yielding wrong
				// numbers.
				let kind = index.get_content_kind(&child);
				if !wanted.contains(&kind) {
					continue;
				}

				let uuid = index.get_or_assign_uuid(&child);
				let sd_path = SdPath::Physical {
					device_slug: device_slug.clone(),
					path: child.clone(),
				};

				let mut file = File::from_arena(uuid, &metadata, sd_path);
				file.content_kind = kind;
				files.push(file);
			}
		}
		drop(index);

		if matches!(self.input.sort_by, MediaSortBy::DateTaken) {
			crate::ops::search::capture::decorate_files(cache, &mut files).await;
		}
		sort_media(&mut files, &self.input.sort_by);

		let total_count = files.len() as u32;
		let limit = self.input.limit.unwrap_or(1000) as usize;
		let has_more = files.len() > limit;
		files.truncate(limit);

		Ok(MediaListingOutput {
			files,
			total_count,
			has_more,
		})
	}
}

/// Newest first for every date order, since a camera roll reads backwards.
fn sort_media(files: &mut [File], sort_by: &MediaSortBy) {
	match sort_by {
		MediaSortBy::Modified => files.sort_by(|a, b| b.modified_at.cmp(&a.modified_at)),
		MediaSortBy::Created => files.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
		// A photo without a capture time still belongs in the roll, so its
		// birth time stands in rather than sending it to the end.
		MediaSortBy::DateTaken => files.sort_by(|a, b| {
			b.captured_at()
				.unwrap_or(b.created_at)
				.cmp(&a.captured_at().unwrap_or(a.created_at))
		}),
		MediaSortBy::Name => {
			files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
		}
		MediaSortBy::Size => files.sort_by(|a, b| b.size.cmp(&a.size)),
	}
}

crate::register_library_query!(MediaListingQuery, "files.media_listing");
