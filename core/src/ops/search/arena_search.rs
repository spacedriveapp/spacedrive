//! Search over the volume index, with the stores underneath it.
//!
//! Every drive this machine knows about has a partition in the arena, so a
//! library-wide search is a fan-out over all of them and a scoped one resolves
//! the partition that covers the path. The arena is the fast path, not the
//! floor: a registered source whose arena cannot answer is searched in its
//! SQLite store instead, through one backend per source per request. An
//! empty result from whichever backend was selected is final; nothing
//! retries the other one.

use crate::domain::{File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::QueryError;
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::EntryKind;
use crate::ops::indexing::VolumeIndex;
use crate::ops::search::input::{DateField, PaginationOptions, SearchFilters, SortOptions};
use crate::ops::search::output::{FileSearchResult, ScoreBreakdown, SearchFacets};
use crate::ops::search::pipeline;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// One page of search results with the whole match set's accounting: the
/// true pre-pagination total and facets folded over every filtered match,
/// not only the page served.
pub struct SearchPage {
	pub results: Vec<FileSearchResult>,
	pub total: u64,
	pub facets: SearchFacets,
	/// True when any participating store capped its hydration, making the
	/// total a floor rather than an exact count.
	pub approximate: bool,
}

/// Search one partition of the volume index, scoped to a path within it.
/// A scope on another device is served from that device's replica.
///
/// Returns the requested page and the true pre-pagination match count.
pub async fn search_arena(
	query: &str,
	path_scope: &SdPath,
	filters: &SearchFilters,
	sort: &SortOptions,
	pagination: &PaginationOptions,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &VolumeIndex,
	file_type_registry: &FileTypeRegistry,
) -> Result<SearchPage, QueryError> {
	let SdPath::Physical {
		path: local_path,
		device_slug,
	} = path_scope
	else {
		return Ok(SearchPage::empty());
	};

	let tag_scope =
		crate::ops::search::tag_scope::TagScope::resolve_if_active(cache, filters.tags.as_ref())
			.await;

	if *device_slug != crate::device::get_current_device_slug() {
		// A replica's assertion state lives with its owner, so a tag filter
		// removes replica hits rather than passing them through unfiltered.
		if tag_scope.is_some() {
			return Ok(SearchPage::empty());
		}
		let shares = crate::service::mounts::peer::remote_shares().await;
		let Some(share) = shares.iter().find(|share| {
			local_path.starts_with(&share.info.root)
				&& context
					.device_manager
					.get_device_slug(share.device_id)
					.is_some_and(|slug| slug == *device_slug)
		}) else {
			return Ok(SearchPage::empty());
		};

		let matching_paths = {
			let index = share.index.read().await;
			matches_in(&index, query, Some(local_path), filters)
		};
		let results = collect_results(
			&share.index,
			matching_paths,
			query,
			device_slug,
			filters,
			file_type_registry,
		)
		.await?;
		return Ok(SearchPage::single_partition(results, sort, pagination));
	}

	// The volume decides how the scope is written: the index holds the
	// volume's spelling, so a scope reached through an alias such as
	// /Users/me has to be rewritten before it can select a partition or
	// filter its paths.
	let local_path = match context.volume_manager.locate_path(local_path).await {
		Some((_, spelled)) => spelled,
		None => local_path.clone(),
	};

	// A registered source that has not been touched this session restores from
	// its snapshot here, including detached drives, whose indexes serve
	// read-only.
	cache.ensure_restored(&local_path).await;

	// One backend for the scope: the arena when it can answer, the store
	// when it cannot. A partition that merely exists does not answer — its
	// emptiness would be indistinguishable from an empty source.
	if !cache.arena_answers(&local_path) {
		if let Some(page) = store_scoped_page(
			&local_path,
			query,
			filters,
			tag_scope.as_ref(),
			sort,
			pagination,
			cache,
			file_type_registry,
		)
		.await?
		{
			return Ok(page);
		}
	}

	let Some(index_arc) = cache.get_for_search(&local_path) else {
		return Ok(SearchPage::empty());
	};

	let mut matching_paths = {
		let index = index_arc.read().await;
		matches_in(&index, query, Some(&local_path), filters)
	};
	if let Some(scope) = &tag_scope {
		matching_paths.retain(|path| scope.admits(path));
	}

	let results = collect_results(
		&index_arc,
		matching_paths,
		query,
		device_slug,
		filters,
		file_type_registry,
	)
	.await?;
	Ok(SearchPage::single_partition(results, sort, pagination))
}

/// Serve a scoped search from the store of the source containing the scope.
/// `None` when no registered source covers it or its store will not open,
/// which sends the caller back to the arena path.
async fn store_scoped_page(
	scope: &PathBuf,
	query: &str,
	filters: &SearchFilters,
	tag_scope: Option<&crate::ops::search::tag_scope::TagScope>,
	sort: &SortOptions,
	pagination: &PaginationOptions,
	cache: &VolumeIndex,
	file_type_registry: &FileTypeRegistry,
) -> Result<Option<SearchPage>, QueryError> {
	let Some(source) = cache
		.sources()
		.into_iter()
		.filter(|source| scope.starts_with(&source.root))
		.max_by_key(|source| source.root.as_os_str().len())
	else {
		return Ok(None);
	};
	let Some(db) = cache.read_store(source.id).await else {
		return Ok(None);
	};

	let mut partition = crate::ops::search::store_search::search_source_store(
		&db,
		&source.root,
		&crate::device::get_current_device_slug(),
		query,
		Some(scope),
		filters,
		file_type_registry,
	)
	.await?;
	retain_tagged(&mut partition.results, tag_scope);

	Ok(Some(SearchPage::single_partition_with(
		partition.results,
		sort,
		pagination,
		partition.truncated,
	)))
}

impl SearchPage {
	fn empty() -> Self {
		Self {
			results: Vec::new(),
			total: 0,
			facets: SearchFacets::default(),
			approximate: false,
		}
	}

	/// Page one partition's full filtered match set.
	fn single_partition(
		results: Vec<FileSearchResult>,
		sort: &SortOptions,
		pagination: &PaginationOptions,
	) -> Self {
		Self::single_partition_with(results, sort, pagination, false)
	}

	fn single_partition_with(
		results: Vec<FileSearchResult>,
		sort: &SortOptions,
		pagination: &PaginationOptions,
		approximate: bool,
	) -> Self {
		let total = results.len() as u64;
		let facets = SearchFacets::from_results(&results);
		Self {
			results: pipeline::page(results, sort, pagination),
			total,
			facets,
			approximate,
		}
	}
}

/// Search every partition, which is what a library-wide search is now that
/// every attached drive is mapped — including paired devices' sources,
/// whose replicated indexes answer from memory just like the local ones.
/// Returns the requested page and the true pre-pagination match count. Each
/// partition contributes its full filtered match count to the total and only
/// its page-window of candidates to the merge.
pub async fn search_every_index(
	query: &str,
	filters: &SearchFilters,
	sort: &SortOptions,
	pagination: &PaginationOptions,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &VolumeIndex,
	file_type_registry: &FileTypeRegistry,
) -> Result<SearchPage, QueryError> {
	let mut candidates = Vec::new();
	let mut total: u64 = 0;
	let mut facets = SearchFacets::default();
	let mut approximate = false;
	let window = pipeline::window(pagination);
	// Every local path some partition has already answered for, taken from
	// each arena's full match set before it is narrowed to the page window,
	// so the store loop below can drop a hit a nested store would repeat
	// without undercounting what fell outside the page.
	let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
	let local_slug = crate::device::get_current_device_slug();
	let tag_scope =
		crate::ops::search::tag_scope::TagScope::resolve_if_active(cache, filters.tags.as_ref())
			.await;

	for index_arc in cache.all_indexes() {
		let mut matching_paths = {
			let index = index_arc.read().await;
			matches_in(&index, query, None, filters)
		};
		if let Some(scope) = &tag_scope {
			matching_paths.retain(|path| scope.admits(path));
		}

		let mut partition = collect_results(
			&index_arc,
			matching_paths,
			query,
			&local_slug,
			filters,
			file_type_registry,
		)
		.await?;
		total += partition.len() as u64;
		facets.absorb(&partition);
		seen.extend(
			partition
				.iter()
				.filter_map(|result| result.file.sd_path.as_local_path().map(Path::to_path_buf)),
		);
		pipeline::narrow(&mut partition, sort, window);
		candidates.extend(partition);
	}

	for share in crate::service::mounts::peer::remote_shares().await {
		// A replica's assertion state lives with its owner; under a tag
		// filter its hits are removed rather than passed through unfiltered.
		if tag_scope.is_some() {
			break;
		}
		// A hit is addressed by its owning device's slug, which is what
		// routes a listing or preview of it back through the replica.
		let Some(slug) = context.device_manager.get_device_slug(share.device_id) else {
			tracing::debug!(
				"replica of {} has no slug for device {}; skipping in search",
				share.info.root.display(),
				share.device_id
			);
			continue;
		};

		let matching_paths = {
			let index = share.index.read().await;
			matches_in(&index, query, None, filters)
		};

		let mut partition = collect_results(
			&share.index,
			matching_paths,
			query,
			&slug,
			filters,
			file_type_registry,
		)
		.await?;
		total += partition.len() as u64;
		facets.absorb(&partition);
		pipeline::narrow(&mut partition, sort, window);
		candidates.extend(partition);
	}

	// Registered sources no arena answered for read from their stores, one
	// backend per source. A source whose arena contributed above is not
	// re-queried; an empty store answer is final the same way.
	//
	// Nested sources' stores overlap by design: a file under the inner root
	// is committed to both. The innermost source is its owner, so stores
	// are read innermost first and a file an earlier store already answered
	// for is dropped from the outer one, before it is counted, so the total
	// stays the number of distinct files. `seen` also carries the arenas'
	// hits, for a source nested across a volume boundary whose outer arena
	// holds paths under the inner root.
	let mut sources = cache.sources();
	sources.sort_by_key(|source| std::cmp::Reverse(source.root.components().count()));
	for source in sources {
		if cache.arena_answers(&source.root) {
			continue;
		}
		let Some(db) = cache.read_store(source.id).await else {
			continue;
		};

		let store_partition = crate::ops::search::store_search::search_source_store(
			&db,
			&source.root,
			&local_slug,
			query,
			None,
			filters,
			file_type_registry,
		)
		.await?;
		let mut partition = store_partition.results;
		retain_tagged(&mut partition, tag_scope.as_ref());
		partition.retain(|result| match result.file.sd_path.as_local_path() {
			Some(path) => seen.insert(path.to_path_buf()),
			None => true,
		});
		approximate |= store_partition.truncated;

		total += partition.len() as u64;
		facets.absorb(&partition);
		pipeline::narrow(&mut partition, sort, window);
		candidates.extend(partition);
	}

	Ok(SearchPage {
		results: pipeline::page(candidates, sort, pagination),
		total,
		facets,
		approximate,
	})
}

/// Keep the results a tag scope admits, matched by their local path.
fn retain_tagged(
	results: &mut Vec<FileSearchResult>,
	tag_scope: Option<&crate::ops::search::tag_scope::TagScope>,
) {
	if let Some(scope) = tag_scope {
		results.retain(|result| {
			result
				.file
				.sd_path
				.as_local_path()
				.is_none_or(|path| scope.admits(path))
		});
	}
}

/// Paths in one partition whose name contains the query, narrowed to a scope
/// if given.
///
/// Substring matching subsumes exact and prefix hits, and scoring already
/// ranks them above it, so an exact query surfaces its file first without
/// hiding everything else that contains the term. Every name contains the
/// empty query, so when a filter narrows it the whole scope is a candidate;
/// without one it yields the scope's direct children, and nothing
/// library-wide.
fn matches_in(
	index: &crate::ops::indexing::Arena,
	query: &str,
	scope: Option<&PathBuf>,
	filters: &SearchFilters,
) -> Vec<PathBuf> {
	if query.is_empty() {
		if filters.narrows() {
			return match scope {
				Some(root) => index.entries_beneath(root),
				None => index.find_containing(""),
			};
		}
		return match scope {
			Some(path) => index.list_directory(path).unwrap_or_default(),
			None => Vec::new(),
		};
	}

	let query = query.to_lowercase();
	let paths = index.find_containing(&query);

	match scope {
		Some(root) => paths
			.into_iter()
			.filter(|path| path.starts_with(root))
			.collect(),
		None => paths,
	}
}

async fn collect_results(
	index_arc: &std::sync::Arc<tokio::sync::RwLock<crate::ops::indexing::Arena>>,
	matching_paths: Vec<PathBuf>,
	query: &str,
	device_slug: &str,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> Result<Vec<FileSearchResult>, QueryError> {
	// One write lock for the batch rather than one per entry: the lazy uuid
	// assignment needs it and the lock is the expensive part.
	let mut index = index_arc.write().await;
	let mut results = Vec::new();

	for path in matching_paths {
		if let Some(metadata) = index.get_entry_ref(&path) {
			// Bundle internals are lensed out of search; their contents
			// surface through the source that describes them.
			if crate::ops::indexing::lens::is_bundle_internal(&path) {
				continue;
			}

			// Apply filters
			if !passes_arena_filters(&metadata, filters, file_type_registry) {
				continue;
			}

			// Get or assign UUID (lazy generation)
			let uuid = index.get_or_assign_uuid(&path);

			let sd_path = SdPath::Physical {
				device_slug: device_slug.to_string(),
				path: path.clone(),
			};

			let content_kind = index.get_content_kind(&path);

			// Convert to File
			let mut file = File::from_arena(uuid, &metadata, sd_path);
			file.content_kind = content_kind;

			// Score by relevance
			let score = score_match(&file, query);

			results.push(FileSearchResult {
				file,
				score,
				score_breakdown: ScoreBreakdown::new(score, None, 0.0, 0.0, 0.0),
				highlights: Vec::new(),
				matched_content: None,
			});
		}
	}

	Ok(results)
}

/// Check if arena metadata passes the search filters
pub(super) fn passes_arena_filters(
	metadata: &EntryMetadata,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> bool {
	// Hidden files are excluded unless asked for. The predicate judges the
	// entry's own name, so searching inside a hidden folder still finds its
	// visible contents.
	if !filters.include_hidden.unwrap_or(false) && metadata.is_hidden {
		return false;
	}

	// File type filter (extension). Case-folded on both sides: the UI shows
	// lowercased extensions while the filesystem stores whatever casing the
	// file was written with.
	if let Some(ref types) = filters.file_types {
		let ext = metadata
			.path
			.extension()
			.and_then(|e| e.to_str())
			.unwrap_or("");
		if !types.iter().any(|t| t.eq_ignore_ascii_case(ext)) {
			return false;
		}
	}

	// Size filter
	if let Some(ref range) = filters.size_range {
		let size = metadata.size;
		if let Some(min) = range.min {
			if size < min {
				return false;
			}
		}
		if let Some(max) = range.max {
			if size > max {
				return false;
			}
		}
	}

	// Date filter. A missing timestamp fails the filter rather than passing
	// it: an entry that cannot prove it is in the range is not in the range.
	// The index holds no access or indexed-at times, so those fields fail
	// closed until a backend can answer them; the UI offers only the fields
	// advertised as answerable.
	if let Some(ref range) = filters.date_range {
		use chrono::{DateTime, Utc};

		let system_time_opt = match range.field {
			DateField::ModifiedAt => metadata.modified,
			DateField::CreatedAt => metadata.created,
			DateField::AccessedAt => metadata.accessed,
			DateField::IndexedAt => None,
		};

		let Some(system_time) = system_time_opt else {
			return false;
		};
		let date = DateTime::<Utc>::from(system_time);

		if let Some(start) = range.start {
			if date < start {
				return false;
			}
		}
		if let Some(end) = range.end {
			if date > end {
				return false;
			}
		}
	}

	// Content type filter (via extension using FileTypeRegistry)
	if let Some(ref content_types) = filters.content_types {
		// Use FileTypeRegistry to identify content kind by extension
		let identified_kind = file_type_registry.identify_by_extension(&metadata.path);

		// Check if the identified kind matches any of the requested types
		if !content_types.contains(&identified_kind) {
			return false;
		}
	}

	// Tags have no arena representation, so a tag filter is ignored rather
	// than silently excluding everything.

	true
}

/// Score a match based on query relevance. Shared by the arena and store
/// backends so a hit ranks identically whichever one served it.
pub(super) fn score_match(file: &File, query: &str) -> f32 {
	if query.is_empty() {
		return 0.5; // Neutral score for empty queries
	}

	let name = file.name.to_lowercase();
	let query_lower = query.to_lowercase();

	// Exact match
	if name == query_lower {
		return 1.0;
	}

	// Prefix match (file starts with query)
	if name.starts_with(&query_lower) {
		return 0.9;
	}

	// Word boundary match (query matches a complete word)
	if let Some(base_name) = file.name.split('.').next() {
		if base_name.to_lowercase() == query_lower {
			return 0.85;
		}
	}

	// Contains match (query anywhere in filename)
	if name.contains(&query_lower) {
		// Score higher if query is near the beginning
		if let Some(pos) = name.find(&query_lower) {
			let pos_score = 1.0 - (pos as f32 / name.len() as f32);
			return 0.5 + (pos_score * 0.3);
		}
		return 0.5;
	}

	// Weak match (shouldn't happen with find_containing, but just in case)
	0.1
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::search::input::{DateRangeFilter, SizeRangeFilter};

	fn metadata(name: &str, size: u64, modified: Option<std::time::SystemTime>) -> EntryMetadata {
		EntryMetadata {
			path: std::path::PathBuf::from(format!("/vol/{name}")),
			kind: EntryKind::File,
			size,
			modified,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: name.starts_with('.'),
		}
	}

	fn passes(metadata: &EntryMetadata, filters: &SearchFilters) -> bool {
		let registry = FileTypeRegistry::new();
		passes_arena_filters(metadata, filters, &registry)
	}

	/// Hidden entries are excluded by default and included on request; the
	/// data was always in the index, the filter just never read it.
	#[test]
	fn hidden_entries_are_excluded_unless_asked_for() {
		let dotfile = metadata(".env", 1, None);
		let plain = metadata("env.txt", 1, None);

		let defaults = SearchFilters::default();
		assert!(!passes(&dotfile, &defaults));
		assert!(passes(&plain, &defaults));

		let include = SearchFilters {
			include_hidden: Some(true),
			..Default::default()
		};
		assert!(passes(&dotfile, &include));
	}

	/// The UI shows lowercased extensions; the filesystem stores any casing.
	/// The filter has to meet in the middle.
	#[test]
	fn extension_filter_is_case_insensitive() {
		let upper = metadata("PHOTO.JPG", 1, None);
		let filters = SearchFilters {
			file_types: Some(vec!["jpg".to_string()]),
			..Default::default()
		};
		assert!(passes(&upper, &filters));

		let other = metadata("notes.txt", 1, None);
		assert!(!passes(&other, &filters));
	}

	/// An entry that cannot prove it is in a date range is not in the range.
	#[test]
	fn a_missing_timestamp_fails_a_date_filter() {
		let dated = metadata(
			"dated.txt",
			1,
			Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)),
		);
		let undated = metadata("undated.txt", 1, None);

		let filters = SearchFilters {
			date_range: Some(DateRangeFilter {
				field: DateField::ModifiedAt,
				start: Some(chrono::DateTime::from_timestamp(1_500_000_000, 0).unwrap()),
				end: None,
			}),
			..Default::default()
		};
		assert!(passes(&dated, &filters));
		assert!(!passes(&undated, &filters));
	}

	#[test]
	fn size_bounds_are_inclusive() {
		let file = metadata("mid.bin", 100, None);
		let filters = SearchFilters {
			size_range: Some(SizeRangeFilter {
				min: Some(100),
				max: Some(100),
			}),
			..Default::default()
		};
		assert!(passes(&file, &filters));
	}

	/// One capture, two backends, the same answer. The store must mirror the
	/// arena's Unicode folding, hidden default, extension filter and scoring,
	/// so routing a source to SQLite is invisible in what comes back.
	#[tokio::test]
	async fn the_store_backend_matches_the_arena_for_the_same_capture() {
		use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
		use std::time::{Duration, UNIX_EPOCH};

		let root = PathBuf::from("/vol/kept");
		let fixture: &[(&str, u64, bool)] = &[
			("Clip One.MOV", 10_000, false),
			("ÉLITE.mov", 2_000, false),
			(".secret.mov", 3_000, true),
			("notes.txt", 100, false),
		];
		let mtime_secs = 1_700_000_000u64;

		// The arena's copy.
		let mut index = crate::ops::indexing::Arena::new().expect("index");
		for (name, size, hidden) in fixture {
			let path = root.join(name);
			index
				.add_entry(
					path.clone(),
					Uuid::now_v7(),
					EntryMetadata {
						path,
						kind: EntryKind::File,
						size: *size,
						modified: Some(UNIX_EPOCH + Duration::from_secs(mtime_secs)),
						accessed: None,
						created: None,
						inode: None,
						permissions: None,
						uid: None,
						gid: None,
						link_target: None,
						is_hidden: *hidden,
					},
				)
				.expect("entry");
		}
		let index_arc = std::sync::Arc::new(tokio::sync::RwLock::new(index));

		// The store's copy of the same capture.
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = sd_store::SourceManager::new(dir.path().to_path_buf());
		manager
			.create("src-1", &sd_store::filesystem_schema())
			.await
			.expect("create");
		let db = manager.open("src-1").await.expect("open");
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
		let writes: Vec<FileWrite> = fixture
			.iter()
			.map(|(name, size, hidden)| {
				let observation = Observation {
					external_id: name.to_string(),
					kind: FileKind::File,
					name: name.to_string(),
					size: *size as i64,
					mtime: mtime_secs as i64 * 1000,
					created: None,
					accessed: None,
					inode: None,
					mode: Some(0o644),
					uid: None,
					gid: None,
					link_target: None,
					extension: name.rsplit_once('.').map(|(_, e)| e.to_string()),
					is_hidden: *hidden,
					identity: None,
				};
				let resolution = ledger.resolve(&observation);
				FileWrite {
					resolution,
					parent_uuid: None,
					observation,
				}
			})
			.collect();
		db.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");
		drop(db);
		let db = manager.open_read_only("src-1").await.expect("read-only");

		let registry = FileTypeRegistry::new();
		let compare = |arena: Vec<FileSearchResult>,
		               store: Vec<crate::ops::search::output::FileSearchResult>| {
			let mut arena: Vec<(String, u64, f32)> = arena
				.into_iter()
				.map(|r| (r.file.name.clone(), r.file.size, r.score))
				.collect();
			let mut store: Vec<(String, u64, f32)> = store
				.into_iter()
				.map(|r| (r.file.name.clone(), r.file.size, r.score))
				.collect();
			arena.sort_by(|a, b| a.0.cmp(&b.0));
			store.sort_by(|a, b| a.0.cmp(&b.0));
			assert_eq!(arena, store);
			arena
		};

		let run = |query: &'static str, filters: SearchFilters| {
			let index_arc = index_arc.clone();
			let db = &db;
			let root = root.clone();
			let registry = &registry;
			async move {
				let matching = {
					let index = index_arc.read().await;
					matches_in(&index, query, None, &filters)
				};
				let arena = collect_results(&index_arc, matching, query, "dev", &filters, registry)
					.await
					.expect("arena results");

				let store = crate::ops::search::store_search::search_source_store(
					db, &root, "dev", query, None, &filters, registry,
				)
				.await
				.expect("store results");
				assert!(!store.truncated);
				(arena, store.results)
			}
		};

		// Folded matching, hidden excluded by default: both find the ASCII
		// and the accented name and neither surfaces the dotfile.
		let (arena, store) = run("mov", SearchFilters::default()).await;
		let matched = compare(arena, store);
		assert_eq!(
			matched
				.iter()
				.map(|(name, ..)| name.as_str())
				.collect::<Vec<_>>(),
			vec!["Clip One", "ÉLITE"]
		);

		// Hidden included on request, on both backends alike.
		let include_hidden = SearchFilters {
			include_hidden: Some(true),
			..Default::default()
		};
		let (arena, store) = run("mov", include_hidden).await;
		assert_eq!(compare(arena, store).len(), 3);

		// A size floor excludes the same entries from both.
		let sized = SearchFilters {
			size_range: Some(SizeRangeFilter {
				min: Some(5_000),
				max: None,
			}),
			..Default::default()
		};
		let (arena, store) = run("mov", sized).await;
		assert_eq!(compare(arena, store).len(), 1);

		// An empty answer from the selected backend is a real answer.
		let (arena, store) = run("nothing-here", SearchFilters::default()).await;
		assert!(compare(arena, store).is_empty());

		// A filter carries an empty query on both backends alike.
		let movies = SearchFilters {
			file_types: Some(vec!["mov".to_string()]),
			..Default::default()
		};
		let (arena, store) = run("", movies).await;
		assert_eq!(compare(arena, store).len(), 2);

		// Without one, an empty query matches nothing on either.
		let (arena, store) = run("", SearchFilters::default()).await;
		assert!(compare(arena, store).is_empty());
	}

	/// A filter-only search in a folder reaches every entry beneath it, and
	/// nothing in a sibling that shares its name as a prefix.
	#[test]
	fn an_empty_query_with_a_filter_matches_the_whole_scope() {
		let mut index = crate::ops::indexing::Arena::new().expect("index");
		for name in ["photos/a.png", "photos/trip/b.png", "photos-other/c.png"] {
			let entry = metadata(name, 1, None);
			index
				.add_entry(entry.path.clone(), Uuid::now_v7(), entry)
				.expect("entry");
		}
		let scope = PathBuf::from("/vol/photos");
		let images = SearchFilters {
			file_types: Some(vec!["png".to_string()]),
			..Default::default()
		};

		let mut matched = matches_in(&index, "", Some(&scope), &images);
		matched.sort();
		assert_eq!(
			matched,
			vec![
				PathBuf::from("/vol/photos/a.png"),
				PathBuf::from("/vol/photos/trip"),
				PathBuf::from("/vol/photos/trip/b.png"),
			]
		);

		let mut listed = matches_in(&index, "", Some(&scope), &SearchFilters::default());
		listed.sort();
		assert_eq!(
			listed,
			vec![
				PathBuf::from("/vol/photos/a.png"),
				PathBuf::from("/vol/photos/trip"),
			]
		);
	}
}
