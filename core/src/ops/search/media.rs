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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::input::{SearchFilters, SearchScope};
use super::store_search::passes_store_filters;
use super::tag_scope::TagScope;
use crate::context::CoreContext;
use crate::domain::{ContentKind, File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::ops::indexing::volume_index::SourceStatus;

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
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let input = self.input;
		let reached = stores_reaching(&context, &input.scope).await;
		if reached.is_empty() {
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
		// A cursor into a source the scope no longer reaches names a page past
		// the end.
		let start = match &input.after {
			None => 0,
			Some(cursor) => match reached
				.iter()
				.position(|(source, _)| source.id == cursor.source)
			{
				Some(index) => index,
				None => return Ok(finished),
			},
		};

		let cache = context.volume_index();
		let tags = TagScope::resolve_if_active(cache, input.filters.tags.as_ref()).await;
		let judge = Judge {
			needle: input.query.to_lowercase(),
			filters: &input.filters,
			tags: tags.as_ref(),
			registry,
			device_slug: crate::device::get_current_device_slug(),
		};
		let include_hidden = input.filters.include_hidden.unwrap_or(false);
		let limit = input.limit as usize;
		let mut files = Vec::new();

		for (index, (source, scope)) in reached.iter().enumerate().skip(start) {
			let Some(db) = cache.read_store(source.id).await else {
				continue;
			};
			let mut after = input
				.after
				.as_ref()
				.filter(|_| index == start)
				.map(|cursor| (cursor.directory.clone(), cursor.name.clone()));
			let mut found = Vec::new();
			let mut full = false;

			while !full {
				let wanted = limit - files.len() - found.len();
				let batch = sd_store::read::files_beneath(
					db.pool(),
					scope,
					after
						.as_ref()
						.map(|(directory, name)| (directory.as_str(), name.as_str())),
					Some(&extensions),
					include_hidden,
					wanted,
				)
				.await
				.map_err(|error| QueryError::Internal(format!("media page failed: {error}")))?;
				let exhausted = batch.len() < wanted;

				for entry in &batch {
					after = Some(key_of(entry));
					if let Some(file) = judge.admit(entry, &source.root) {
						found.push(file);
						if files.len() + found.len() == limit {
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
			files.append(&mut found);
			if let (true, Some((directory, name))) = (full, after) {
				return Ok(MediaSearchOutput {
					files,
					next: Some(MediaCursor {
						source: source.id,
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

/// How a page judges a store row: the checks the store backend of
/// `search.files` makes, then whether the file is an image or a video.
struct Judge<'a> {
	needle: String,
	filters: &'a SearchFilters,
	tags: Option<&'a TagScope>,
	registry: &'a FileTypeRegistry,
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
}

/// Where a row sits in its store's order: its directory and its name.
fn key_of(entry: &sd_store::FsEntry) -> (String, String) {
	let directory = entry
		.relative_path
		.rsplit_once('/')
		.map_or("", |(directory, _)| directory);
	(directory.to_string(), entry.name.clone())
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

/// The stores a scope reaches, each with the directory it is read from, in
/// root order so pages run source by source.
async fn stores_reaching(
	context: &CoreContext,
	scope: &SearchScope,
) -> Vec<(SourceStatus, String)> {
	// A source whose drive is not mounted has no root to build paths from.
	let mut sources: Vec<SourceStatus> = context
		.volume_index()
		.sources()
		.into_iter()
		.filter(|source| !source.root.as_os_str().is_empty())
		.collect();
	sources.sort_by(|a, b| a.root.cmp(&b.root).then(a.id.cmp(&b.id)));

	let path = match scope {
		SearchScope::Library => {
			return sources
				.into_iter()
				.map(|source| (source, String::new()))
				.collect()
		}
		SearchScope::Path {
			path: SdPath::Physical { device_slug, path },
		} if *device_slug == crate::device::get_current_device_slug() => path,
		SearchScope::Path { .. } => return Vec::new(),
	};
	// The volume decides how a path is spelled, so a scope reached through an
	// alias such as /Users/me is rewritten before it meets any root.
	let path = match context.volume_manager.locate_path(path).await {
		Some((_, spelled)) => spelled,
		None => path.clone(),
	};
	reach_path(sources, &path)
}

/// The sources that hold what is beneath `path`: the innermost one holding
/// the path, read from the path down, and every one nested beneath the path,
/// read whole. The innermost registered source keeps a path's records, so a
/// nested source's files live in its own store rather than the outer one's.
fn reach_path(sources: Vec<SourceStatus>, path: &Path) -> Vec<(SourceStatus, String)> {
	let innermost = sources
		.iter()
		.filter(|source| path.starts_with(&source.root))
		.max_by_key(|source| source.root.as_os_str().len())
		.map(|source| source.id);
	sources
		.into_iter()
		.filter_map(|source| {
			if Some(source.id) == innermost {
				// Store paths are relative with forward slashes whatever the
				// host writes.
				let relative = path
					.strip_prefix(&source.root)
					.ok()?
					.to_str()?
					.replace(std::path::MAIN_SEPARATOR, "/");
				Some((source, relative))
			} else if source.root.starts_with(path) {
				Some((source, String::new()))
			} else {
				None
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn source(root: &str) -> SourceStatus {
		SourceStatus {
			id: Uuid::new_v4(),
			root: PathBuf::from(root),
			volume_uuid: None,
			attached: true,
			restored: true,
			last_seen_secs: 0,
			entry_count: None,
			total_bytes: None,
			directory: None,
			thumbs_path: None,
		}
	}

	fn reached(sources: &[SourceStatus], path: &str) -> Vec<(String, String)> {
		reach_path(sources.to_vec(), &PathBuf::from(path))
			.into_iter()
			.map(|(source, scope)| (source.root.display().to_string(), scope))
			.collect()
	}

	/// A folder is read from the innermost source holding it, and a source
	/// nested beneath the folder is read whole, since its files are in its own
	/// store. A folder above every source reaches each one beneath it.
	#[test]
	fn a_path_reaches_the_source_holding_it_and_those_nested_beneath() {
		let sources = [source("/A"), source("/A/m"), source("/B")];

		assert_eq!(reached(&sources, "/A/x"), [("/A".into(), "x".into())]);
		assert_eq!(
			reached(&sources, "/A"),
			[("/A".into(), String::new()), ("/A/m".into(), String::new())]
		);
		assert_eq!(
			reached(&sources, "/A/m/deep"),
			[("/A/m".into(), "deep".into())]
		);
		assert_eq!(reached(&sources, "/").len(), 3);
		assert!(reached(&sources, "/C").is_empty());
	}

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
		let judge = Judge {
			needle: "élite".to_string(),
			filters: &filters,
			tags: None,
			registry: &registry,
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
}
