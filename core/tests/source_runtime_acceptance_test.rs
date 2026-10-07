//! Single-daemon rows of the R8 source runtime acceptance matrix
//! (`docs/core/acceptance/source-runtime.md`): read routing between a loaded
//! arena and a source's store, pagination honesty, store-only filter fields,
//! nested-source deduplication, status agreement and watcher reporting.
//!
//! Every test runs one `Core` over temporary directories. Nothing here needs
//! a second daemon, a real drive or a network.

mod helpers;

use std::path::{Path, PathBuf};

use helpers::*;
use sd_core::{
	domain::addressing::SdPath,
	infra::{api::SessionContext, query::CoreQuery, query::LibraryQuery},
	ops::{
		core::index_status::query::{IndexStatusInput, IndexStatusQuery},
		indexing::{metadata::EntryMetadata, state::EntryKind, IndexScope, VolumeAnchor},
		search::{
			input::{
				FileSearchInput, PaginationOptions, SearchFilters, SearchMode, SearchScope,
				SortDirection, SortField, SortOptions, TagFilter,
			},
			output::FileSearchOutput,
			query::FileSearchQuery,
		},
		sources::list::query::ListSourcesQuery,
	},
};
use sd_store::db::Stamp;
use sd_store::tags::{normalize_tag_path, slug_for_path, TagAssertion, TagDefinition};
use uuid::Uuid;

fn session(harness: &IndexingHarness) -> SessionContext {
	let device_id = sd_core::device::get_current_device_id();
	let device_name = sd_core::device::get_current_device_slug();
	let mut session = SessionContext::device_session(device_id, device_name);
	session.current_library_id = Some(harness.library.id());
	session
}

async fn search(
	harness: &IndexingHarness,
	input: FileSearchInput,
) -> anyhow::Result<FileSearchOutput> {
	Ok(FileSearchQuery::new(input)
		.execute(harness.core.context.clone(), session(harness))
		.await?)
}

fn by_name(query: &str, scope: SearchScope, limit: u32, offset: u32) -> FileSearchInput {
	FileSearchInput {
		query: query.to_string(),
		scope,
		mode: SearchMode::Normal,
		filters: SearchFilters::default(),
		sort: SortOptions {
			field: SortField::Name,
			direction: SortDirection::Asc,
		},
		pagination: PaginationOptions { limit, offset },
	}
}

fn metadata_for(path: &Path) -> EntryMetadata {
	let metadata = std::fs::metadata(path).expect("metadata");
	EntryMetadata {
		path: path.to_path_buf(),
		kind: if metadata.is_dir() {
			EntryKind::Directory
		} else {
			EntryKind::File
		},
		size: metadata.len(),
		modified: metadata.modified().ok(),
		accessed: None,
		created: None,
		inode: None,
		permissions: None,
		uid: None,
		gid: None,
		link_target: None,
		is_hidden: false,
	}
}

async fn anchor_for(harness: &IndexingHarness, root: &Path) -> (PathBuf, Option<VolumeAnchor>) {
	match harness.core.context.volume_manager.locate_path(root).await {
		Some((volume, spelled)) => (
			spelled,
			Some(VolumeAnchor {
				uuid: volume.id,
				mount_point: volume.mount_point.clone(),
			}),
		),
		None => (root.to_path_buf(), None),
	}
}

/// Register `root` as a source and commit `files` to its store without
/// walking it, which is the shape of a source whose arena is unloaded: its
/// records are retained and nothing in memory can answer for it.
async fn store_only_source(
	harness: &IndexingHarness,
	root: &Path,
	files: &[&str],
) -> anyhow::Result<(Uuid, PathBuf)> {
	let cache = harness.core.context.volume_index();
	let (root, anchor) = anchor_for(harness, root).await;
	let id = cache.register_source(&root, anchor).await?;
	let store = cache
		.store_for(&root.join(files[0]))
		.await
		.expect("the registered source has a store");
	for name in files {
		let path = root.join(name);
		tokio::fs::write(&path, name).await?;
		store
			.identify_one(&metadata_for(&path), None)
			.await
			.expect("identified");
	}
	store.flush().await?;
	Ok((id, root))
}

fn names(output: &FileSearchOutput) -> Vec<String> {
	output
		.results
		.iter()
		.map(|result| result.file.name.clone())
		.collect()
}

/// R8 "Suitable loaded arena returns no matches".
///
/// A walked source answers from its arena. A record only its store knows
/// about must not surface, because an empty arena answer is final and never
/// triggers a second backend.
#[tokio::test]
async fn an_empty_answer_from_a_suitable_arena_is_final() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_arena_empty_is_final")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("walked").await?;
	dir.write_file("alpha.txt", "a").await?;
	dir.write_file("beta.txt", "b").await?;
	let tracked = dir.track().await?;
	let cache = harness.core.context.volume_index();
	assert!(
		cache.arena_answers(&tracked.root),
		"a walked source answers"
	);

	// The store learns of a record the arena never saw.
	let ghost = tracked.root.join("gamma-ghost.txt");
	tokio::fs::write(&ghost, "ghost").await?;
	let store = cache.store_for(&ghost).await.expect("store");
	store
		.identify_one(&metadata_for(&ghost), None)
		.await
		.expect("identified");
	store.flush().await?;
	assert!(
		store.db().resolve_path("gamma-ghost.txt").await?.is_some(),
		"the store holds the ghost"
	);

	let scope = SearchScope::Path {
		path: SdPath::local(tracked.root.clone()),
	};
	let found = search(&harness, by_name("alpha", scope.clone(), 10, 0)).await?;
	assert_eq!(names(&found), vec!["alpha"], "the arena answers the scope");

	let empty = search(&harness, by_name("gamma-ghost", scope, 10, 0)).await?;
	assert_eq!(
		empty.total_found, 0,
		"no SQL retry after an empty arena answer"
	);
	assert!(empty.results.is_empty());

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Loaded arena has insufficient coverage or query support".
///
/// A registered source nothing walked this session has no arena that can
/// answer, so a search over it reads the store. The limitation stays
/// explicit: the volume index reports that the arena does not answer.
#[tokio::test]
async fn an_unwalked_source_answers_from_its_store() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_store_fallback")
		.disable_watcher()
		.build()
		.await?;
	let root = harness.temp_path().join("retained");
	tokio::fs::create_dir_all(&root).await?;
	let (_, root) = store_only_source(&harness, &root, &["delta-kept.txt", "notes.md"]).await?;
	let cache = harness.core.context.volume_index();
	assert!(!cache.arena_answers(&root), "nothing in memory covers it");

	let scoped = search(
		&harness,
		by_name(
			"delta",
			SearchScope::Path {
				path: SdPath::local(root.clone()),
			},
			10,
			0,
		),
	)
	.await?;
	assert_eq!(names(&scoped), vec!["delta-kept"]);
	assert!(scoped.total_is_exact);

	let library = search(&harness, by_name("delta-kept", SearchScope::Library, 10, 0)).await?;
	assert_eq!(
		names(&library),
		vec!["delta-kept"],
		"a library search reaches the store too"
	);
	assert!(
		!cache.arena_answers(&root),
		"answering from the store loaded no arena"
	);

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Five suitable arenas and 95 stores".
///
/// One backend per source: with five sources walked and 95 left to their
/// stores, a library search returns the same global results, total and
/// pages as it did when every source answered from its store.
#[tokio::test]
async fn five_arenas_and_ninety_five_stores_page_the_same() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_mixed_backends")
		.disable_watcher()
		.build()
		.await?;
	let base = harness.temp_path().join("fleet");
	let mut roots = Vec::new();
	for n in 0..100 {
		let root = base.join(format!("source-{n:03}"));
		tokio::fs::create_dir_all(&root).await?;
		let files: Vec<String> = (0..3).map(|k| format!("doc-{n:03}-{k}.txt")).collect();
		let names: Vec<&str> = files.iter().map(String::as_str).collect();
		let (_, root) = store_only_source(&harness, &root, &names).await?;
		roots.push(root);
	}

	let page = |offset| by_name("doc-", SearchScope::Library, 25, offset);
	let store_only: Vec<FileSearchOutput> = {
		let mut pages = Vec::new();
		for offset in [0, 25, 275] {
			pages.push(search(&harness, page(offset)).await?);
		}
		pages
	};
	assert_eq!(store_only[0].total_found, 300);
	assert!(store_only[0].total_is_exact);
	assert_eq!(store_only[0].results.len(), 25);
	assert_eq!(store_only[2].results.len(), 25);

	for root in &roots[..5] {
		harness.index_dir(root, IndexScope::Recursive).await?;
	}
	let cache = harness.core.context.volume_index();
	assert!(roots[..5].iter().all(|root| cache.arena_answers(root)));
	assert!(roots[5..].iter().all(|root| !cache.arena_answers(root)));

	for (offset, before) in [0, 25, 275].into_iter().zip(&store_only) {
		let after = search(&harness, page(offset)).await?;
		assert_eq!(after.total_found, before.total_found, "offset {offset}");
		assert_eq!(
			names(&after),
			names(before),
			"page at offset {offset} differs once arenas join"
		);
		let paths = |output: &FileSearchOutput| -> Vec<String> {
			output
				.results
				.iter()
				.map(|r| format!("{:?}", r.file.sd_path))
				.collect()
		};
		assert_eq!(paths(&after), paths(before));
	}

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Search with requested limit five".
#[tokio::test]
async fn a_limit_of_five_returns_five_with_an_honest_total() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_limit_five")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("paged").await?;
	for n in 0..12 {
		dir.write_file(&format!("page-{n:02}.txt"), "p").await?;
	}
	let tracked = dir.track().await?;

	let scope = SearchScope::Path {
		path: SdPath::local(tracked.root.clone()),
	};
	let first = search(&harness, by_name("page-", scope.clone(), 5, 0)).await?;
	assert_eq!(first.results.len(), 5);
	assert_eq!(first.files.len(), 5);
	assert_eq!(first.total_found, 12);
	assert!(first.total_is_exact);
	assert_eq!(first.pagination.limit, 5);
	assert_eq!(first.pagination.offset, 0);
	assert_eq!(first.pagination.total_pages, 3);
	assert!(first.pagination.has_next);
	assert!(!first.pagination.has_previous);
	assert_eq!(
		names(&first),
		["page-00", "page-01", "page-02", "page-03", "page-04"]
	);

	let last = search(&harness, by_name("page-", scope, 5, 10)).await?;
	assert_eq!(names(&last), ["page-10", "page-11"]);
	assert_eq!(last.total_found, 12);
	assert!(!last.pagination.has_next);
	assert!(last.pagination.has_previous);

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Arena candidates need store-only filter or sort fields".
///
/// Tags live in the store, not the arena. A tag filter over an arena-backed
/// source must narrow before pagination: the one tagged file is the whole
/// result, not whichever file the first page happened to hold.
#[tokio::test]
async fn a_store_only_filter_narrows_before_pagination() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_store_filter")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("tagged").await?;
	for n in 0..10 {
		dir.write_file(&format!("tagged-{n}.txt"), "t").await?;
	}
	let tracked = dir.track().await?;
	let cache = harness.core.context.volume_index();
	assert!(cache.arena_answers(&tracked.root));

	let store = cache
		.store_for(&tracked.root.join("tagged-7.txt"))
		.await
		.expect("store");
	let record = store
		.db()
		.resolve_path("tagged-7.txt")
		.await?
		.expect("the walk committed the file");
	let device = Uuid::new_v4();
	let hlc = |t: u64| format!("{t:016x}-{:016x}-{device}", 0);
	let path = normalize_tag_path("Keep").expect("tag path");
	let tag = TagDefinition {
		uuid: Uuid::new_v4(),
		slug_id: slug_for_path(&path),
		path,
		color: None,
		icon: None,
		updated_hlc: hlc(100),
		origin_device: device,
	};
	store
		.db()
		.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await?;
	store
		.db()
		.append_tag_assertions(&[TagAssertion {
			tag_uuid: tag.uuid,
			record_uuid: record,
			external_id: Some("tagged-7.txt".to_string()),
			content_uuid: None,
			asserted: true,
			stamp: Stamp {
				hlc: hlc(110),
				device_uuid: device,
			},
		}])
		.await?;

	let mut input = by_name("tagged", SearchScope::Library, 1, 0);
	input.filters.tags = Some(TagFilter {
		include: vec![tag.uuid],
		exclude: vec![],
	});
	let page = search(&harness, input).await?;
	assert_eq!(
		names(&page),
		vec!["tagged-7"],
		"the filter applied before the page was cut"
	);
	assert_eq!(page.total_found, 1);

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Same record in multiple representations", the loaded half: a file
/// under a nested source is one hit, since both registrations share the
/// drive's one arena.
#[tokio::test]
async fn a_file_under_nested_sources_is_one_hit_from_the_arena() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_nested_arena")
		.disable_watcher()
		.build()
		.await?;
	let outer = harness.create_test_dir("outer").await?;
	outer.write_file("inner/unique-photo.jpg", "jpeg").await?;
	outer.track().await?;
	track_nested(&harness, outer.path().join("inner")).await?;
	assert_eq!(
		harness.core.context.volume_index().sources().len(),
		2,
		"two registrations"
	);

	let hits = search(
		&harness,
		by_name("unique-photo", SearchScope::Library, 10, 0),
	)
	.await?;
	assert_eq!(hits.total_found, 1, "one file, one hit");
	assert_eq!(names(&hits), vec!["unique-photo"]);

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Same record in multiple representations", the store half: after a
/// restart with no snapshot, both nested stores hold the file and a library
/// search must still report it once with its innermost owner.
#[tokio::test]
async fn a_file_under_nested_sources_is_one_hit_from_the_stores() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_nested_stores")
		.disable_watcher()
		.build()
		.await?;
	let outer = harness.create_test_dir("outer").await?;
	outer.write_file("inner/unique-photo.jpg", "jpeg").await?;
	outer.track().await?;
	track_nested(&harness, outer.path().join("inner")).await?;

	// The daemon restarts and no arena snapshot survives, so every source
	// answers from its store.
	let cache = harness.core.context.volume_index();
	let snapshots: Vec<PathBuf> = cache
		.sources()
		.iter()
		.filter_map(|source| cache.source_snapshot_path(source.id))
		.collect();
	cache.detach_library();
	for snapshot in snapshots {
		let _ = std::fs::remove_file(snapshot);
	}
	cache.attach_library(harness.library.db().clone()).await?;
	assert!(cache
		.sources()
		.iter()
		.all(|source| !cache.arena_answers(&source.root)));

	let hits = search(
		&harness,
		by_name("unique-photo", SearchScope::Library, 10, 0),
	)
	.await?;
	assert_eq!(
		hits.total_found, 1,
		"one file, one hit, whichever stores hold it"
	);
	assert_eq!(names(&hits), vec!["unique-photo"]);

	harness.shutdown().await?;
	Ok(())
}

/// Track `root` as a second, nested source and wait for its walk.
async fn track_nested(harness: &IndexingHarness, root: PathBuf) -> anyhow::Result<()> {
	use sd_core::infra::job::types::JobId;
	let output = sd_core::ops::sources::track::action::track_and_index(
		&harness.library,
		&harness.core.context,
		root,
		None,
		&Default::default(),
	)
	.await
	.map_err(|e| anyhow::anyhow!("{e}"))?;
	let job_id = output.job_id.expect("tracking dispatched a walk");
	if let Some(walk) = harness.library.jobs().get_job(JobId(job_id)).await {
		walk.wait().await?;
	}
	Ok(())
}

/// R8 "Status query and resource event".
///
/// The listing, the index status and the store must describe one source
/// with one count and one observation time once its walk has committed.
#[tokio::test]
async fn listing_status_and_store_agree_on_a_sources_count() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_counts_agree")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("counted").await?;
	dir.write_file("a.txt", "a").await?;
	dir.write_file("b.txt", "b").await?;
	dir.write_file("sub/c.txt", "c").await?;
	let tracked = dir.track().await?;

	let cache = harness.core.context.volume_index();
	let store = cache
		.store_for(&tracked.root.join("a.txt"))
		.await
		.expect("store");
	store.flush().await?;
	let committed = store.counts().await.expect("committed counts");
	assert_eq!(
		committed.records, 4,
		"three files and one directory committed"
	);

	let listed = ListSourcesQuery::all()
		.execute(harness.core.context.clone(), session(&harness))
		.await?
		.into_iter()
		.find(|source| source.id == tracked.id)
		.expect("the source is listed");
	assert_eq!(
		listed.item_count as u64, committed.records,
		"sources.list reports the committed count"
	);

	let status = IndexStatusQuery::from_input(IndexStatusInput::default())?
		.execute(harness.core.context.clone(), session(&harness))
		.await?;
	let in_status = status
		.sources
		.iter()
		.find(|source| source.id == tracked.id)
		.expect("index status lists the source");
	assert_eq!(
		in_status.entry_count,
		Some(committed.records),
		"core.index_status reports the same count"
	);
	assert_eq!(
		listed
			.last_seen_at
			.as_deref()
			.and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
			.map(|at| at.timestamp() as u64),
		Some(in_status.last_seen_secs),
		"the same observation time on both surfaces"
	);

	harness.shutdown().await?;
	Ok(())
}

/// R8 "Failed watcher subscription".
///
/// A root the OS refuses to watch is not watched. The volume index must
/// not report it active after the subscription failed.
#[tokio::test]
async fn a_refused_watch_is_not_reported_active() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("r8_watch_refused")
		.build()
		.await?;
	let watcher = harness
		.core
		.context
		.get_fs_watcher()
		.await
		.expect("watcher service");
	let dir = harness.create_test_dir("vanishing").await?;
	dir.write_file("a.txt", "a").await?;
	harness.index_dir(dir.path(), IndexScope::Recursive).await?;
	let cache = harness.core.context.volume_index();
	let root = match harness
		.core
		.context
		.volume_manager
		.locate_path(dir.path())
		.await
	{
		Some((_, spelled)) => spelled,
		None => dir.path().to_path_buf(),
	};
	cache.unregister_from_watching(&root);
	let _ = watcher.unwatch_path(&root).await;

	// The directory vanishes before the subscription is attempted.
	tokio::fs::remove_dir_all(dir.path()).await?;
	let refused = watcher.watch_root(root.clone()).await;
	assert!(refused.is_err(), "the OS cannot watch a missing directory");
	assert!(
		!cache.is_watched(&root),
		"a refused subscription must not be reported as an active watch"
	);
	assert!(!watcher.watched_paths().await.contains(&root));
	let status = IndexStatusQuery::from_input(IndexStatusInput::default())?
		.execute(harness.core.context.clone(), session(&harness))
		.await?;
	assert!(
		!status.watched_paths.contains(&root),
		"core.index_status does not list the refused root as watched"
	);
	let refusal = status
		.refused_watches
		.iter()
		.find(|r| r.path == root)
		.expect("core.index_status reports the refusal");
	assert!(!refusal.reason.is_empty(), "the refusal carries its reason");

	// The directory comes back and the retry reconciles the root to active.
	tokio::fs::create_dir_all(dir.path()).await?;
	watcher.retry_refused_watches().await;
	assert!(cache.is_watched(&root), "a successful retry arms the watch");
	assert!(watcher.watched_paths().await.contains(&root));
	assert!(
		cache.refused_watches().is_empty(),
		"a retried root leaves the refused list"
	);

	harness.shutdown().await?;
	Ok(())
}
