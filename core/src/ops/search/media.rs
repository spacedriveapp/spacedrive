//! Images and videos beneath a scope, read from source stores a page at a
//! time.
//!
//! A media viewer pages through everything a folder holds. `search.files`
//! rebuilds and sorts its whole match set for every page, so paging a large
//! folder through it repeats that work once per page. This reads each source's
//! store in the order its indexes already keep, directory path and then name,
//! so a page reads its own rows and the next resumes from a cursor.
//!
//! Extension, kind and hidden narrow in SQL. The name, size, date and tag
//! filters are judged per row exactly as the store backend of `search.files`
//! judges them, so a search and the media in it answer the same question. Names
//! match in Rust for the Unicode folding SQLite does not do.
//!
//! Sources this device replicates from paired devices page after its own. Most
//! arrive as index snapshots with no store beside them, so a replica pages from
//! the index this device holds of it, in path order, judged as the arena
//! backend of `search.files` judges its rows.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::arena_search::passes_arena_filters;
use super::input::{SearchFilters, SearchScope};
use super::store_search::passes_store_filters;
use super::tag_scope::TagScope;
use crate::context::CoreContext;
use crate::domain::{ContentKind, File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::{Arena, VolumeIndex};
use crate::ops::paths::reach::{
	every_replica, every_store, replicas_beneath, stores_beneath, Reach, ReplicaReach,
};
use sd_store::read::Start;

/// The most files one page may ask for.
const MAX_PAGE: u32 = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaSearchInput {
	/// Part of a file name, case-folded; empty matches every name.
	pub query: String,
	pub scope: SearchScope,
	pub filters: SearchFilters,
	/// Where the previous page ended; `None` for the first page.
	pub after: Option<MediaCursor>,
	/// Files per page.
	pub limit: u32,
}

/// A place in the order pages are served in: source by source, by root, then
/// by directory path and name within a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MediaCursor {
	pub source: Uuid,
	/// Source-relative directory of the last file served; "" for the root.
	pub directory: String,
	pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaSearchOutput {
	/// Images and videos, in page order.
	pub files: Vec<File>,
	/// Where the next page starts; `None` after the last.
	pub next: Option<MediaCursor>,
	/// Whether any source reaches the scope. A folder outside every source
	/// has nothing to page through, which is not the same as a source that
	/// holds no media.
	pub covered: bool,
}

pub struct MediaSearchQuery {
	input: MediaSearchInput,
}

impl LibraryQuery for MediaSearchQuery {
	type Input = MediaSearchInput;
	type Output = MediaSearchOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.limit == 0 || input.limit > MAX_PAGE {
			return Err(QueryError::InvalidInput(format!(
				"limit must be between 1 and {MAX_PAGE}"
			)));
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let input = self.input;
		let (stores, replicas) = match &input.scope {
			SearchScope::Library => (every_store(&context), every_replica(&context).await),
			SearchScope::Path { path } => (
				stores_beneath(&context, path).await,
				replicas_beneath(&context, path).await,
			),
		};
		if stores.is_empty() && replicas.is_empty() {
			return Ok(MediaSearchOutput {
				files: Vec::new(),
				next: None,
				covered: false,
			});
		}
		let finished = MediaSearchOutput {
			files: Vec::new(),
			next: None,
			covered: true,
		};

		let registry = context.file_type_registry();
		let extensions = media_extensions(registry, &input.filters);
		if extensions.is_empty() {
			return Ok(finished);
		}

		let cache = context.volume_index();
		let tags = TagScope::resolve_if_active(cache, input.filters.tags.as_ref()).await;
		let mut partitions: Vec<Partition> = stores.into_iter().map(Partition::Store).collect();
		// A replica's tags live with its owner, so under a tag filter its files
		// are left out rather than passed through unfiltered.
		if tags.is_none() {
			partitions.extend(replicas.into_iter().map(Partition::Replica));
		}

		// A cursor into a source the scope no longer reaches names a page past
		// the end.
		let start = match &input.after {
			None => 0,
			Some(cursor) => match partitions
				.iter()
				.position(|partition| partition.id() == cursor.source)
			{
				Some(index) => index,
				None => return Ok(finished),
			},
		};

		let judge = Judge {
			needle: input.query.to_lowercase(),
			filters: &input.filters,
			tags: tags.as_ref(),
			registry,
			extensions: &extensions,
			device_slug: crate::device::get_current_device_slug(),
		};
		let limit = input.limit as usize;
		let mut files = Vec::new();

		for (index, partition) in partitions.iter().enumerate().skip(start) {
			let after = input
				.after
				.as_ref()
				.filter(|_| index == start)
				.map(|cursor| (cursor.directory.clone(), cursor.name.clone()));
			let wanted = limit - files.len();
			let page = match partition {
				Partition::Store(reach) => store_page(cache, reach, &judge, after, wanted).await?,
				Partition::Replica(reach) => {
					replica_page(
						&context,
						session.current_library_id,
						reach,
						&judge,
						after,
						wanted,
					)
					.await
				}
			};
			files.extend(page.files);
			if let Some((directory, name)) = page.full_at {
				return Ok(MediaSearchOutput {
					files,
					next: Some(MediaCursor {
						source: partition.id(),
						directory,
						name,
					}),
					covered: true,
				});
			}
		}

		Ok(MediaSearchOutput { files, ..finished })
	}
}

crate::register_library_query!(MediaSearchQuery, "search.media");

/// Where pages read from, in the order they are served: this device's stores
/// by root, then the replicas of paired devices by owner and root.
enum Partition {
	Store(Reach),
	Replica(ReplicaReach),
}

impl Partition {
	fn id(&self) -> Uuid {
		match self {
			Self::Store(reach) => reach.source.id,
			Self::Replica(reach) => reach.share.info.id,
		}
	}
}

/// What one partition gave a page.
#[derive(Default)]
struct Page {
	files: Vec<File>,
	/// Where the page filled up, as `(directory, name)` relative to the
	/// partition's root; `None` when the partition ran out first.
	full_at: Option<(String, String)>,
}

/// Up to `wanted` images and videos from a store after `after`, in the order
/// its indexes keep: directory path, then name.
async fn store_page(
	cache: &VolumeIndex,
	reach: &Reach,
	judge: &Judge<'_>,
	mut after: Option<(String, String)>,
	wanted: usize,
) -> QueryResult<Page> {
	let source = &reach.source;
	let Some(db) = cache.read_store(source.id).await else {
		return Ok(Page::default());
	};
	let mut found = Vec::new();
	let mut full = false;

	while !full {
		let batch_size = wanted - found.len();
		let batch = sd_store::read::files_beneath(
			db.pool(),
			&reach.scope,
			after
				.as_ref()
				.map_or(Start::First, |(directory, name)| Start::After {
					directory,
					name,
				}),
			Some(judge.extensions),
			judge.filters.include_hidden.unwrap_or(false),
			batch_size,
		)
		.await
		.map_err(|error| QueryError::Internal(format!("media page failed: {error}")))?;
		let exhausted = batch.len() < batch_size;

		for entry in &batch {
			after = Some((entry.directory().to_string(), entry.name.clone()));
			if let Some(file) = judge.admit(entry, &source.root) {
				found.push(file);
				if found.len() == wanted {
					full = true;
					break;
				}
			}
		}
		if exhausted {
			break;
		}
	}

	crate::ops::tags::decorate::decorate_from_store(&db, &mut found).await;
	Ok(Page {
		files: found,
		full_at: after.filter(|_| full),
	})
}

/// Up to `wanted` images and videos of a replica after `after`, in path order,
/// read from the index this device holds of it.
async fn replica_page(
	context: &Arc<CoreContext>,
	library_id: Option<Uuid>,
	reach: &ReplicaReach,
	judge: &Judge<'_>,
	after: Option<(String, String)>,
	wanted: usize,
) -> Page {
	let mut page = {
		// One write lock for the page, since a file's id is assigned the
		// first time it is asked for.
		let mut index = reach.share.index.write().await;
		replica_files(
			&mut index,
			&reach.share.info.root,
			&reach.scope,
			&reach.device_slug,
			judge,
			after,
			wanted,
		)
	};
	if let Some(library_id) = library_id {
		crate::ops::tags::decorate::decorate_replica(
			context,
			library_id,
			&reach.share,
			&mut page.files,
		)
		.await;
	}
	page
}

/// [`replica_page`] from a replica's index alone: the files beneath `scope`
/// of the replica at `root`, addressed on the device `device_slug` names.
fn replica_files(
	index: &mut Arena,
	root: &Path,
	scope: &str,
	device_slug: &str,
	judge: &Judge<'_>,
	after: Option<(String, String)>,
	wanted: usize,
) -> Page {
	let scope = if scope.is_empty() {
		root.to_path_buf()
	} else {
		root.join(scope)
	};
	let Some(paths) = index.files_in_scope(&scope, true) else {
		return Page::default();
	};
	let start = after.map_or(0, |(directory, name)| {
		let resume = root.join(directory).join(name);
		paths.partition_point(|path| *path <= resume)
	});
	let mut page = Page::default();
	for path in &paths[start..] {
		let Some(metadata) = index.get_entry_ref(path) else {
			continue;
		};
		if !judge.admits_replica_entry(path, &metadata) {
			continue;
		}
		let content_kind = match index.get_content_kind(path) {
			ContentKind::Unknown => judge.registry.identify_by_extension(path),
			kind => kind,
		};
		let mut file = File::from_arena(
			index.get_or_assign_uuid(path),
			&metadata,
			SdPath::Physical {
				device_slug: device_slug.to_string(),
				path: path.clone(),
			},
		);
		file.content_kind = content_kind;
		page.files.push(file);
		if page.files.len() == wanted {
			page.full_at = Some(cursor_at(root, path));
			break;
		}
	}
	page
}

/// A file's place in a partition as a cursor names it: its directory relative
/// to the root with forward slashes, and its name.
fn cursor_at(root: &Path, path: &Path) -> (String, String) {
	let directory = path
		.parent()
		.and_then(|parent| parent.strip_prefix(root).ok())
		.map(|relative| {
			relative
				.to_string_lossy()
				.replace(std::path::MAIN_SEPARATOR, "/")
		})
		.unwrap_or_default();
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	(directory, name)
}

/// How a page judges a store row: the checks the store backend of
/// `search.files` makes, then whether the file is an image or a video.
struct Judge<'a> {
	needle: String,
	filters: &'a SearchFilters,
	tags: Option<&'a TagScope>,
	registry: &'a FileTypeRegistry,
	/// The extensions a page reads, from [`media_extensions`]: sorted.
	extensions: &'a [String],
	device_slug: String,
}

impl Judge<'_> {
	fn admit(&self, entry: &sd_store::FsEntry, root: &Path) -> Option<File> {
		let absolute = root.join(&entry.relative_path);
		// Bundle internals are lensed out here as they are out of search.
		if crate::ops::indexing::lens::is_bundle_internal(&absolute)
			|| (!self.needle.is_empty() && !entry.name.to_lowercase().contains(&self.needle))
			|| !passes_store_filters(entry, self.filters, &absolute, self.registry)
			|| self.tags.is_some_and(|tags| !tags.admits(&absolute))
		{
			return None;
		}
		let file = File::from_store_entry(
			entry,
			SdPath::Physical {
				device_slug: self.device_slug.clone(),
				path: absolute,
			},
		);
		matches!(file.content_kind, ContentKind::Image | ContentKind::Video).then_some(file)
	}

	/// Whether an entry of a replica's index is an image or a video the
	/// filters admit, judged as the arena backend of `search.files` judges its
	/// rows. A replica carries no tags here, so it is never asked under a tag
	/// filter.
	fn admits_replica_entry(&self, path: &Path, metadata: &EntryMetadata) -> bool {
		let media = path
			.extension()
			.and_then(|extension| extension.to_str())
			.is_some_and(|extension| {
				self.extensions
					.binary_search(&extension.to_lowercase())
					.is_ok()
			});
		let name = path
			.file_name()
			.map(|name| name.to_string_lossy().to_lowercase())
			.unwrap_or_default();
		media
			&& !crate::ops::indexing::lens::is_bundle_internal(path)
			&& (self.needle.is_empty() || name.contains(&self.needle))
			&& passes_arena_filters(metadata, self.filters, self.registry)
	}
}

/// The extensions a page reads: those of images and videos, narrowed to the
/// kinds and extensions the filters name. Lowercase, which is how the store
/// compares them.
fn media_extensions(registry: &FileTypeRegistry, filters: &SearchFilters) -> Vec<String> {
	let mut extensions: Vec<String> = [ContentKind::Image, ContentKind::Video]
		.into_iter()
		.filter(|kind| {
			filters
				.content_types
				.as_ref()
				.is_none_or(|kinds| kinds.contains(kind))
		})
		.flat_map(|kind| registry.get_extensions_for_category(kind))
		.map(str::to_lowercase)
		.filter(|extension| {
			filters
				.file_types
				.as_ref()
				.is_none_or(|types| types.iter().any(|t| t.eq_ignore_ascii_case(extension)))
		})
		.collect();
	extensions.sort_unstable();
	extensions.dedup();
	extensions
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn media_extensions_narrow_to_the_kinds_and_types_asked_for() {
		let registry = FileTypeRegistry::new();
		let all = media_extensions(&registry, &SearchFilters::default());
		assert!(all.contains(&"jpg".to_string()) && all.contains(&"mov".to_string()));

		let images = SearchFilters {
			content_types: Some(vec![ContentKind::Image]),
			..Default::default()
		};
		let only_images = media_extensions(&registry, &images);
		assert!(only_images.contains(&"jpg".to_string()));
		assert!(!only_images.contains(&"mov".to_string()));

		let jpegs = SearchFilters {
			file_types: Some(vec!["JPG".to_string()]),
			..Default::default()
		};
		assert_eq!(media_extensions(&registry, &jpegs), ["jpg"]);

		let documents = SearchFilters {
			content_types: Some(vec![ContentKind::Document]),
			..Default::default()
		};
		assert!(media_extensions(&registry, &documents).is_empty());
	}

	fn entry(relative_path: &str, hidden: bool) -> sd_store::FsEntry {
		let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
		sd_store::FsEntry {
			uuid: Uuid::new_v4(),
			kind: sd_store::FileKind::File,
			name: name.to_string(),
			relative_path: relative_path.to_string(),
			size: Some(1),
			mtime_ms: Some(1_700_000_000_000),
			atime_ms: None,
			created_ms: None,
			is_hidden: hidden,
			extension: name.rsplit_once('.').map(|(_, e)| e.to_string()),
			link_target: None,
			inode: None,
			mode: None,
			uid: None,
			gid: None,
			content_uuid: None,
			sampled_hash: None,
			integrity_hash: None,
			content_kind: None,
			content_error: None,
		}
	}

	/// A row passes on its name folded the way search folds it, and only as an
	/// image or a video.
	#[test]
	fn a_row_is_judged_like_a_search_hit_and_kept_only_as_media() {
		let registry = FileTypeRegistry::new();
		let filters = SearchFilters::default();
		let extensions = media_extensions(&registry, &filters);
		let judge = Judge {
			needle: "élite".to_string(),
			filters: &filters,
			tags: None,
			registry: &registry,
			extensions: &extensions,
			device_slug: "laptop".to_string(),
		};
		let root = Path::new("/vol/kept");

		let admitted = judge
			.admit(&entry("trip/ÉLITE.mov", false), root)
			.expect("a folded name match is admitted");
		assert_eq!(
			admitted.sd_path,
			SdPath::Physical {
				device_slug: "laptop".to_string(),
				path: PathBuf::from("/vol/kept/trip/ÉLITE.mov"),
			}
		);
		assert!(judge.admit(&entry("trip/other.mov", false), root).is_none());
		assert!(judge.admit(&entry("trip/élite.txt", false), root).is_none());
		assert!(judge.admit(&entry("trip/.élite.mov", true), root).is_none());
	}

	/// A replica's index holding these files beneath `root`.
	fn replica_index(root: &Path, names: &[&str]) -> Arena {
		let mut index = Arena::new().expect("index");
		for name in names {
			let path = root.join(name);
			let metadata = EntryMetadata {
				path: path.clone(),
				kind: crate::ops::indexing::state::EntryKind::File,
				size: 1,
				modified: None,
				accessed: None,
				created: None,
				inode: None,
				permissions: None,
				uid: None,
				gid: None,
				link_target: None,
				is_hidden: name.rsplit('/').next().is_some_and(|n| n.starts_with('.')),
			};
			index
				.add_entry(path, Uuid::now_v7(), metadata)
				.expect("entry");
		}
		index
	}

	fn paths_of(page: &Page) -> Vec<String> {
		page.files
			.iter()
			.map(|file| match &file.sd_path {
				SdPath::Physical { path, .. } => path.display().to_string(),
				other => panic!("replica files have physical paths, got {other}"),
			})
			.collect()
	}

	/// A replica pages its images and videos in path order, addressed on the
	/// device that owns them, and a page resumes where the last one filled.
	#[test]
	fn a_replica_pages_its_media_in_path_order() {
		let registry = FileTypeRegistry::new();
		let filters = SearchFilters::default();
		let extensions = media_extensions(&registry, &filters);
		let judge = Judge {
			needle: String::new(),
			filters: &filters,
			tags: None,
			registry: &registry,
			extensions: &extensions,
			device_slug: "laptop".to_string(),
		};
		let root = Path::new("/mnt/pool/photos");
		let mut index = replica_index(
			root,
			&["d.png", "b/c.MOV", "b/notes.txt", "b/.hidden.png", "a.jpg"],
		);

		let first = replica_files(&mut index, root, "", "titan", &judge, None, 2);
		assert_eq!(
			paths_of(&first),
			["/mnt/pool/photos/a.jpg", "/mnt/pool/photos/b/c.MOV"]
		);
		assert!(first.files.iter().all(|file| !file.is_local
			&& matches!(&file.sd_path, SdPath::Physical { device_slug, .. } if device_slug == "titan")));
		assert_eq!(first.files[1].content_kind, ContentKind::Video);
		assert_eq!(first.full_at, Some(("b".to_string(), "c.MOV".to_string())));

		let second = replica_files(&mut index, root, "", "titan", &judge, first.full_at, 2);
		assert_eq!(paths_of(&second), ["/mnt/pool/photos/d.png"]);
		assert_eq!(second.full_at, None);

		let scoped = replica_files(&mut index, root, "b", "titan", &judge, None, 10);
		assert_eq!(paths_of(&scoped), ["/mnt/pool/photos/b/c.MOV"]);

		let missing = replica_files(&mut index, root, "absent", "titan", &judge, None, 10);
		assert!(missing.files.is_empty() && missing.full_at.is_none());
	}
}
