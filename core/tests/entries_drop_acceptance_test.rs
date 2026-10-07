//! Single-daemon rows of the entries final drop acceptance matrix
//! (`docs/core/acceptance/entries-drop-and-file-operations.md`): the library
//! schema after the drop, the static boundary around the retired tables,
//! pins as Space items, assertions surviving a reindex, a store that
//! describes itself away from its library, non-UTF-8 names, and the rename
//! known limit.
//!
//! Every test runs one `Core` over temporary directories. Nothing here needs
//! a second daemon, a real drive or a network.

mod helpers;

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};

use helpers::*;
use sd_core::{
	domain::{addressing::SdPath, ItemType},
	infra::{
		action::LibraryAction,
		api::SessionContext,
		db::{migration::Migrator, Database},
		query::LibraryQuery,
		sync::registry::SyncableInventoryEntry,
	},
	ops::{
		indexing::IndexScope,
		sources::track::{TrackSourceAction, TrackSourceInput},
		spaces::{
			add_item::{action::AddItemAction, input::AddItemInput},
			create::{action::SpaceCreateAction, input::SpaceCreateInput},
			delete_item::{action::DeleteItemAction, input::DeleteItemInput},
			get_layout::query::{SpaceLayoutQuery, SpaceLayoutQueryInput},
		},
	},
};
use sd_store::{
	db::Stamp,
	source::SourceManager,
	tags::{normalize_tag_path, slug_for_path, TagAssertion, TagDefinition},
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;
use uuid::Uuid;

/// What a library holds after the drop, from the FD4 record in the plan.
const LIBRARY_TABLES: &[&str] = &[
	"assertion_outbox",
	"audit_log",
	"cloud_credentials",
	"device_state_tombstones",
	"devices",
	"seaql_migrations",
	"sources",
	"space_groups",
	"space_items",
	"spaces",
	"sqlite_sequence",
	"sync_checkpoints",
	"tag_staging",
	"volumes",
];

/// Tables the drop removed, by name, so a reader that still names one is
/// caught whether it goes through an entity or a raw statement.
const RETIRED_TABLES: &[&str] = &[
	"entries",
	"entry_closure",
	"directory_paths",
	"content_identities",
	"content_kinds",
	"mime_types",
	"image_media_data",
	"video_media_data",
	"audio_media_data",
	"sidecar",
	"sidecar_availability",
	"collection",
	"collection_entry",
	"location",
	"tag",
	"tag_closure",
	"tag_relationship",
	"tag_usage_pattern",
	"user_metadata",
	"user_metadata_tag",
	"sync_conduit",
	"sync_generation",
	"search_analytics",
	"indexer_rules",
	"search_index",
];

async fn sqlite_master(conn: &DatabaseConnection, kind: &str) -> BTreeSet<String> {
	conn.query_all(Statement::from_string(
		DatabaseBackend::Sqlite,
		format!("SELECT name FROM sqlite_master WHERE type = '{kind}' ORDER BY name"),
	))
	.await
	.expect("sqlite_master")
	.into_iter()
	.map(|row| row.try_get::<String>("", "name").expect("name"))
	.collect()
}

fn expected_tables() -> BTreeSet<String> {
	LIBRARY_TABLES.iter().map(|t| t.to_string()).collect()
}

fn session(harness: &IndexingHarness) -> SessionContext {
	let device_id = sd_core::device::get_current_device_id();
	let device_name = sd_core::device::get_current_device_slug();
	let mut session = SessionContext::device_session(device_id, device_name);
	session.current_library_id = Some(harness.library.id());
	session
}

/// FDA "A fresh `library.db` has no retired tables, indexes, FTS tables, or
/// triggers": the full migration chain on an empty file lands on exactly
/// the fourteen tables and nothing in `sqlite_master` names a retired one.
#[tokio::test]
async fn a_fresh_library_has_exactly_the_fourteen_tables() {
	let dir = tempfile::tempdir().expect("tempdir");
	let db = Database::create(&dir.path().join("library.db"))
		.await
		.expect("database");
	db.migrate().await.expect("migrated");

	assert_eq!(sqlite_master(db.conn(), "table").await, expected_tables());
	assert_nothing_retired(db.conn()).await;
}

async fn assert_nothing_retired(conn: &DatabaseConnection) {
	let everything = conn
		.query_all(Statement::from_string(
			DatabaseBackend::Sqlite,
			"SELECT type, name, tbl_name, COALESCE(sql, '') AS sql FROM sqlite_master".to_string(),
		))
		.await
		.expect("sqlite_master");
	for row in everything {
		let kind: String = row.try_get("", "type").expect("type");
		let name: String = row.try_get("", "name").expect("name");
		let table: String = row.try_get("", "tbl_name").expect("tbl_name");
		let sql: String = row.try_get("", "sql").expect("sql");
		assert_ne!(kind, "trigger", "a trigger survived the drop: {name}");
		for retired in RETIRED_TABLES {
			assert_ne!(&table, retired, "{kind} {name} still hangs off {retired}");
			assert!(
				!sql.contains(&format!("\"{retired}\"")) && !sql.contains(&format!(" {retired} ")),
				"{kind} {name} still names {retired}: {sql}"
			);
		}
		assert!(
			!name.starts_with("search_index"),
			"an FTS shadow table survived: {name}"
		);
	}
}

/// FDA "an upgraded library keeps every user assertion" and the FD4 proof:
/// a library built by the pre-drop chain, holding a Space item using the
/// current navigation contract and rows in the retired tables, converges on
/// the fourteen-table schema with the Space item intact.
#[tokio::test]
async fn a_pre_drop_library_upgrades_to_the_fourteen_table_schema() {
	let dir = tempfile::tempdir().expect("tempdir");
	let db = Database::create(&dir.path().join("library.db"))
		.await
		.expect("database");
	let conn = db.conn();

	let drop_index = Migrator::get_migration_files()
		.iter()
		.position(|migration| migration.name() == "m20260918_000001_drop_entries_world")
		.expect("the drop migration is in the chain");
	Migrator::up(conn, Some(drop_index as u32))
		.await
		.expect("the pre-drop chain applies");
	let before = sqlite_master(conn, "table").await;
	for table in [
		"entries",
		"location",
		"content_identities",
		"tag",
		"search_index",
	] {
		assert!(before.contains(table), "the fixture is pre-drop: {table}");
	}

	// A Space item on the current contract, and a Location item the drop
	// deletes rather than migrates.
	let space = Uuid::new_v4();
	let pin = Uuid::new_v4();
	let location_item = Uuid::new_v4();
	let pinned = SdPath::local("/pinned/folder");
	conn.execute_unprepared(&format!(
		"INSERT INTO spaces (uuid, name, icon, color, \"order\", created_at, updated_at)
		 VALUES (X'{space}', 'Fixture', 'folder', '#fff', 0, '2026-09-01 00:00:00', '2026-09-01 00:00:00');
		 INSERT INTO space_items (uuid, space_id, group_id, entry_uuid, item_type, \"order\", created_at)
		 VALUES (X'{pin}', 1, NULL, NULL, '{path_json}', 0, '2026-09-01 00:00:00');
		 INSERT INTO space_items (uuid, space_id, group_id, entry_uuid, item_type, \"order\", created_at)
		 VALUES (X'{location_item}', 1, NULL, NULL, '{{\"Location\":{{\"location_id\":\"00000000-0000-0000-0000-000000000001\"}}}}', 1, '2026-09-01 00:00:00');",
		space = space.simple(),
		pin = pin.simple(),
		location_item = location_item.simple(),
		path_json = serde_json::to_string(&ItemType::Path { sd_path: pinned.clone() })
			.expect("json")
			.replace('\'', "''"),
	))
	.await
	.expect("fixture rows");

	db.migrate().await.expect("the drop applies on an upgrade");

	assert_eq!(sqlite_master(conn, "table").await, expected_tables());
	assert_nothing_retired(conn).await;
	let items = conn
		.query_all(Statement::from_string(
			DatabaseBackend::Sqlite,
			"SELECT item_type FROM space_items ORDER BY \"order\"".to_string(),
		))
		.await
		.expect("items");
	assert_eq!(
		items.len(),
		1,
		"the Location item is deleted, the pin stays"
	);
	let item_type: String = items[0].try_get("", "item_type").expect("item_type");
	let parsed: ItemType = serde_json::from_str(&item_type).expect("current contract");
	assert_eq!(parsed, ItemType::Path { sd_path: pinned });
	let check = conn
		.query_one(Statement::from_string(
			DatabaseBackend::Sqlite,
			"PRAGMA integrity_check".to_string(),
		))
		.await
		.expect("pragma")
		.expect("a row");
	assert_eq!(
		check
			.try_get::<String>("", "integrity_check")
			.expect("result"),
		"ok"
	);

	// Running the chain again is a no-op.
	db.migrate().await.expect("idempotent");
	assert_eq!(sqlite_master(conn, "table").await, expected_tables());
}

/// FDA "A daemon with an old library either upgrades successfully or stops
/// with the documented recovery path": a library whose migration table
/// names a migration this build does not know is refused, the file is left
/// as it was, and no second library appears beside it.
#[tokio::test]
async fn an_incompatible_library_is_refused_and_left_intact() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("fda_incompatible_library")
		.disable_watcher()
		.build()
		.await?;
	let future = harness
		.core
		.libraries
		.create_library("Future", None, harness.core.context.clone())
		.await?;
	let library_dir = future.path().to_path_buf();
	let libraries_dir = library_dir.parent().expect("libraries dir").to_path_buf();
	harness.core.libraries.close_library(future.id()).await?;
	drop(future);
	let db_path = library_dir.join("library.db");
	{
		let db = Database::open(&db_path).await?;
		db.conn()
			.execute_unprepared(
				"INSERT INTO seaql_migrations (version, applied_at)
				 VALUES ('m20991231_000001_from_the_future', 0)",
			)
			.await?;
	}
	let before = {
		let db = Database::open(&db_path).await?;
		(
			sqlite_master(db.conn(), "table").await,
			sqlite_master(db.conn(), "index").await,
			db.conn()
				.query_all(Statement::from_string(
					DatabaseBackend::Sqlite,
					"SELECT version FROM seaql_migrations ORDER BY version".to_string(),
				))
				.await?
				.len(),
		)
	};
	let siblings_before = std::fs::read_dir(&libraries_dir)?.count();

	let opened = harness
		.core
		.libraries
		.open_library(&library_dir, harness.core.context.clone())
		.await;
	let error = match opened {
		Ok(_) => panic!("a library from a newer build opened"),
		Err(error) => error.to_string(),
	};
	assert!(
		error.contains("m20991231_000001_from_the_future"),
		"the refusal names the migration it cannot run: {error}"
	);
	let after = {
		let db = Database::open(&db_path).await?;
		(
			sqlite_master(db.conn(), "table").await,
			sqlite_master(db.conn(), "index").await,
			db.conn()
				.query_all(Statement::from_string(
					DatabaseBackend::Sqlite,
					"SELECT version FROM seaql_migrations ORDER BY version".to_string(),
				))
				.await?
				.len(),
		)
	};
	assert_eq!(
		after, before,
		"the refused database keeps its schema and rows"
	);
	assert_eq!(
		std::fs::read_dir(&libraries_dir)?.count(),
		siblings_before,
		"no second library was created"
	);

	harness.shutdown().await?;
	Ok(())
}

/// FDA static boundary over the registered models: every model the sync
/// registry knows writes to a table the fourteen-table schema has, so no
/// sync apply can reach a retired table.
#[test]
fn every_registered_sync_model_names_a_current_table() {
	let mut seen = 0;
	for entry in inventory::iter::<SyncableInventoryEntry> {
		let registration = (entry.build)();
		seen += 1;
		assert!(
			LIBRARY_TABLES.contains(&registration.table_name),
			"model {} syncs into {}, which the library no longer has",
			registration.model_type,
			registration.table_name
		);
		assert_ne!(registration.model_type, "entry");
	}
	assert!(seen > 0, "no models registered; the inventory did not link");
}

/// FDA static boundary over the source: the plan's three searches return
/// nothing in production code, and no production Rust file names a retired
/// table in a statement.
#[test]
fn no_production_source_names_the_entry_substrate() {
	let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("workspace root")
		.to_path_buf();
	let roots = ["core/src", "apps", "crates", "packages/ts-client/src"];
	let needles = [
		"entities::entry",
		"entry_closure::Entity",
		"directory_paths::Entity",
		"model_type == \"entry\"",
		"core.ephemeral_status",
		"core.ephemeral_reset",
	];
	let mut hits = Vec::new();
	let mut visited = 0usize;
	for root in roots {
		let root = repo.join(root);
		assert!(root.is_dir(), "{} is not a directory", root.display());
		walk(&root, &mut |path| {
			visited += 1;
			let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
				return;
			};
			if !matches!(ext, "rs" | "ts" | "tsx" | "swift") {
				return;
			}
			let Ok(text) = std::fs::read_to_string(path) else {
				return;
			};
			let shown = path
				.strip_prefix(&repo)
				.unwrap_or(path)
				.display()
				.to_string();
			for needle in needles {
				if text.contains(needle) {
					hits.push(format!("{shown}: {needle}"));
				}
			}
			if text
				.lines()
				.any(|line| line.contains("register_syncable") && line.contains("\"entry\""))
			{
				hits.push(format!("{shown}: register_syncable \"entry\""));
			}
			// Raw statements against the library database live in core; the
			// store crate's own `search_index` is its FTS table, not the
			// library's retired one.
			if ext == "rs" && shown.starts_with("core/src") && !shown.contains("/migration/") {
				for retired in RETIRED_TABLES {
					for form in [
						format!("FROM {retired} "),
						format!("FROM {retired}\n"),
						format!("INTO {retired} "),
						format!("UPDATE {retired} "),
						format!("JOIN {retired} "),
						format!("table_name = \"{retired}\""),
					] {
						if text.contains(&form) {
							hits.push(format!("{shown}: {}", form.trim()));
						}
					}
				}
			}
		});
	}
	assert!(
		visited > 1000,
		"the walk visited only {visited} files, which is not the tree"
	);
	assert!(
		hits.is_empty(),
		"production references remain:\n{}",
		hits.join("\n")
	);
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
	let Ok(entries) = std::fs::read_dir(dir) else {
		return;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		let name = entry.file_name();
		let name = name.to_string_lossy();
		if path.is_dir() {
			if matches!(
				name.as_ref(),
				"target" | "node_modules" | "generated" | "gen" | ".git" | "dist"
			) {
				continue;
			}
			walk(&path, visit);
		} else {
			visit(&path);
		}
	}
}

async fn tag_record(store: &sd_core::ops::indexing::SourceStore, record: Uuid, name: &str) -> Uuid {
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
		.await
		.expect("definition");
	store
		.db()
		.append_tag_assertions(&[TagAssertion {
			tag_uuid: tag.uuid,
			record_uuid: record,
			external_id: Some(name.to_string()),
			content_uuid: None,
			asserted: true,
			stamp: Stamp {
				hlc: hlc(110),
				device_uuid: device,
			},
		}])
		.await
		.expect("assertion");
	tag.uuid
}

/// Wait for the walk's follow-on passes (content identification) to stop
/// writing, so a revision read afterwards is a baseline and not a race.
async fn settle(store: &sd_core::ops::indexing::SourceStore) -> anyhow::Result<()> {
	let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
	let mut last = store.db().revision().await?;
	loop {
		tokio::time::sleep(std::time::Duration::from_millis(700)).await;
		store.flush().await?;
		let now = store.db().revision().await?;
		if now == last && store.files_needing_content_count().await? == 0 {
			return Ok(());
		}
		anyhow::ensure!(
			tokio::time::Instant::now() < deadline,
			"the store did not settle"
		);
		last = now;
	}
}

async fn assertion_rows(
	store: &sd_core::ops::indexing::SourceStore,
) -> Vec<(Vec<u8>, Vec<u8>, String, bool)> {
	sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, bool)>(
		"SELECT tag_uuid, record_uuid, hlc, asserted FROM tag_assertion ORDER BY hlc",
	)
	.fetch_all(store.db().pool())
	.await
	.expect("tag_assertion")
}

/// FDA "Reindexing or evicting a source leaves its assertion tables
/// unchanged" and the release gate "source reindex preserves assertions":
/// a tag applied to a record survives the arena being cleared and the
/// source walked again, row for row, and still reaches the same record.
#[tokio::test]
async fn tags_survive_a_source_reindex() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("fda_tags_reindex")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("tagged").await?;
	for n in 0..5 {
		dir.write_file(&format!("photo-{n}.jpg"), &format!("photo {n}"))
			.await?;
	}
	let tracked = dir.track().await?;
	let cache = harness.core.context.volume_index();
	let store = cache
		.store_for(&tracked.root.join("photo-3.jpg"))
		.await
		.expect("store");
	settle(&store).await?;
	let record = store
		.db()
		.resolve_path("photo-3.jpg")
		.await?
		.expect("the walk committed the file");
	let tag = tag_record(&store, record, "photo-3.jpg").await;
	let before = assertion_rows(&store).await;
	let definitions_before = store.db().tag_definitions().await?.len();
	assert_eq!(store.db().records_with_tag(tag).await?, vec![record]);

	// Evict: the arena forgets the drive. Reindex: walk it again.
	cache.clear_for_reindex(&tracked.root).await;
	harness
		.index_dir(&tracked.root, IndexScope::Recursive)
		.await?;
	settle(&store).await?;

	assert_eq!(
		assertion_rows(&store).await,
		before,
		"assertion rows unchanged"
	);
	assert_eq!(
		store.db().tag_definitions().await?.len(),
		definitions_before
	);
	assert_eq!(
		store.db().resolve_path("photo-3.jpg").await?,
		Some(record),
		"the record keeps its identity across the walk"
	);
	assert_eq!(store.db().records_with_tag(tag).await?, vec![record]);

	harness.shutdown().await?;
	Ok(())
}

/// FDA "A source store moved to a clean library remains self-describing"
/// and "Frozen copies are byte-for-byte untouched": a frozen copy of a store
/// opened from another data directory, with no registry row for it, lists
/// its own models, records and tags; reading it changes none of its bytes.
#[tokio::test]
async fn a_frozen_store_describes_itself_and_is_not_written_by_a_reader() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("fda_frozen_store")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("archive").await?;
	dir.write_file("report.pdf", "r").await?;
	dir.write_file("notes/todo.md", "n").await?;
	let tracked = dir.track().await?;
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(&tracked.root.join("report.pdf"))
		.await
		.expect("store");
	let record = store
		.db()
		.resolve_path("report.pdf")
		.await?
		.expect("committed");
	let tag = tag_record(&store, record, "report.pdf").await;

	let frozen_dir = tempfile::tempdir()?;
	let frozen = store.freeze_into(frozen_dir.path()).await?;
	let bytes_before = blake3::hash(&std::fs::read(&frozen)?);

	// A clean home: nothing but the file.
	let moved_dir = tempfile::tempdir()?;
	let moved = moved_dir.path().join("data.db");
	std::fs::copy(&frozen, &moved)?;
	let reader = SourceManager::open_file_read_only(&moved).await?;
	let schema: (String, String) = sqlx::query_as("SELECT data_type_id, schema_toml FROM _schema")
		.fetch_one(reader.pool())
		.await?;
	assert!(
		!schema.0.is_empty() && schema.1.contains("file"),
		"the store carries its own schema: {schema:?}"
	);
	assert_eq!(
		sd_store::read::entry_by_path(reader.pool(), "notes/todo.md")
			.await?
			.map(|entry| entry.name),
		Some("todo.md".to_string())
	);
	assert_eq!(
		sd_store::tags::records_for_tag(&reader, tag).await?,
		vec![record]
	);
	assert_eq!(
		sd_store::tags::list_tag_definitions(reader.pool())
			.await?
			.len(),
		1
	);
	drop(reader);

	assert_eq!(
		blake3::hash(&std::fs::read(&frozen)?),
		bytes_before,
		"the freeze is untouched"
	);
	assert_eq!(
		blake3::hash(&std::fs::read(&moved)?),
		bytes_before,
		"the read-only open wrote nothing into the moved copy"
	);

	harness.shutdown().await?;
	Ok(())
}

async fn add_pin(harness: &IndexingHarness, space: Uuid, path: &Path) -> Uuid {
	AddItemAction::from_input(AddItemInput {
		space_id: space,
		group_id: None,
		item_type: ItemType::Path {
			sd_path: SdPath::local(path),
		},
	})
	.expect("input")
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.expect("added")
	.item
	.id
}

async fn layout(harness: &IndexingHarness, space: Uuid) -> sd_core::domain::SpaceLayout {
	SpaceLayoutQuery::from_input(SpaceLayoutQueryInput { space_id: space })
		.expect("input")
		.execute(harness.core.context.clone(), session(harness))
		.await
		.expect("layout")
}

/// FDA "Adding or removing a bookmark neither indexes nor deletes records
/// and does not change processing policies": a pin is a Space item of the
/// Path kind; adding and removing one dispatches no walk, moves no store
/// revision, and leaves the source's configuration as it was. A pin on a
/// folder nothing tracks is still a pin.
#[tokio::test]
async fn a_pin_is_a_space_item_with_no_indexing_side_effect() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("fda_pins")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("pinned").await?;
	dir.write_file("a.txt", "a").await?;
	dir.write_file("sub/b.txt", "b").await?;
	let tracked = dir.track().await?;
	let cache = harness.core.context.volume_index();
	let store = cache
		.store_for(&tracked.root.join("a.txt"))
		.await
		.expect("store");
	settle(&store).await?;
	let revision = store.db().revision().await?;
	let records = store.db().count_all().await?;
	let config = cache.source_config(tracked.id).map(|c| c.unfiltered);
	let jobs_before = harness.library.jobs().list_jobs(None).await?.len();

	let space = SpaceCreateAction::from_input(SpaceCreateInput {
		name: "Pins".into(),
		icon: "star".into(),
		color: "#ffffff".into(),
	})
	.expect("input")
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space
	.id;

	let untracked = harness.temp_path().join("elsewhere");
	tokio::fs::create_dir_all(&untracked).await?;
	let inside = add_pin(&harness, space, &tracked.root.join("sub")).await;
	let outside = add_pin(&harness, space, &untracked).await;

	let layout = layout(&harness, space).await;
	let pinned: Vec<PathBuf> = layout
		.space_items
		.iter()
		.filter_map(|item| match &item.item_type {
			ItemType::Path { sd_path } => sd_path.as_local_path().map(Path::to_path_buf),
			_ => None,
		})
		.collect();
	assert_eq!(pinned.len(), 2, "{:?}", layout.space_items);
	assert!(pinned.contains(&untracked));

	store.flush().await?;
	assert_eq!(store.db().revision().await?, revision, "no store write");
	assert_eq!(store.db().count_all().await?, records, "no record added");
	assert_eq!(
		harness.library.jobs().list_jobs(None).await?.len(),
		jobs_before,
		"no walk dispatched"
	);
	assert_eq!(
		cache.source_config(tracked.id).map(|c| c.unfiltered),
		config,
		"policy unchanged"
	);
	assert_eq!(
		cache.source_id_for(&untracked),
		None,
		"pinning does not track"
	);

	for item in [inside, outside] {
		DeleteItemAction::from_input(DeleteItemInput { item_id: item })
			.expect("input")
			.execute(harness.library.clone(), harness.core.context.clone())
			.await?;
	}
	store.flush().await?;
	assert_eq!(
		store.db().revision().await?,
		revision,
		"unpinning deletes nothing"
	);
	assert_eq!(store.db().count_all().await?, records);
	assert!(tracked.root.join("sub/b.txt").exists());
	assert!(layout_is_empty(&harness, space).await);

	harness.shutdown().await?;
	Ok(())
}

async fn layout_is_empty(harness: &IndexingHarness, space: Uuid) -> bool {
	layout(harness, space).await.space_items.is_empty()
}

/// FDA "Saved navigation survives" a restart: a pin added before the core
/// stops is listed, and resolves to its folder, after the same data
/// directory is opened again.
///
/// Built on a bare `Core` rather than the harness, so the library handle
/// can be dropped before the restart and its lock released, and run in a
/// child process with its own working directory: `KeyManager::close` swaps
/// the secrets database for a redb file literally named `:memory:` in the
/// working directory, which every other shut-down core holds a lock on, so
/// in a shared process the first core keeps `secrets.redb` open and the
/// restart fails with "Database already open".
#[tokio::test]
async fn a_pin_survives_a_restart() -> anyhow::Result<()> {
	in_child("pin_restart_child").await?;
	Ok(())
}

/// The restart behind `a_pin_survives_a_restart`.
#[tokio::test]
#[ignore = "run by a_pin_survives_a_restart in a child process"]
async fn pin_restart_child() -> anyhow::Result<()> {
	let temp = tempfile::tempdir()?;
	let data_dir = temp.path().join("data");
	let files = temp.path().join("pinned");
	tokio::fs::create_dir_all(files.join("keep")).await?;
	tokio::fs::write(files.join("keep/a.txt"), "a").await?;

	let core = sd_core::Core::new(data_dir.clone())
		.await
		.map_err(|e| anyhow::anyhow!("{e}"))?;
	let library = core
		.libraries
		.create_library("Pins", None, core.context.clone())
		.await?;
	let library_id = library.id();
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: files.clone(),
		name: None,
		unfiltered: false,
	})
	.map_err(anyhow::Error::msg)?
	.execute(library.clone(), core.context.clone())
	.await?;
	if let Some(job) = tracked.job_id {
		if let Some(walk) = library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await?;
		}
	}
	let space = library
		.db()
		.conn()
		.query_one(Statement::from_string(
			DatabaseBackend::Sqlite,
			"SELECT uuid FROM spaces ORDER BY id LIMIT 1".to_string(),
		))
		.await?
		.expect("the default space")
		.try_get::<Uuid>("", "uuid")?;
	let folder = tracked.root.join("keep");
	let pin = ItemType::Path {
		sd_path: SdPath::local(&folder),
	};
	AddItemAction::from_input(AddItemInput {
		space_id: space,
		group_id: None,
		item_type: pin.clone(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await?;

	let session_for = |library_id: Uuid| {
		let mut session = SessionContext::device_session(
			sd_core::device::get_current_device_id(),
			sd_core::device::get_current_device_slug(),
		);
		session.current_library_id = Some(library_id);
		session
	};
	let pins = |layout: sd_core::domain::SpaceLayout| {
		layout
			.space_items
			.into_iter()
			.filter(|item| item.item_type == pin)
			.collect::<Vec<_>>()
	};
	let before = SpaceLayoutQuery::from_input(SpaceLayoutQueryInput { space_id: space })
		.expect("input")
		.execute(core.context.clone(), session_for(library_id))
		.await?;
	assert_eq!(pins(before).len(), 1);

	drop(library);
	core.libraries.close_library(library_id).await?;
	core.shutdown().await.map_err(|e| anyhow::anyhow!("{e}"))?;
	drop(core);

	let core = sd_core::Core::new(data_dir)
		.await
		.map_err(|e| anyhow::anyhow!("{e}"))?;
	assert!(
		core.libraries.get_library(library_id).await.is_some(),
		"the library reopened"
	);
	let after = SpaceLayoutQuery::from_input(SpaceLayoutQueryInput { space_id: space })
		.expect("input")
		.execute(core.context.clone(), session_for(library_id))
		.await?;
	let after = pins(after);
	assert_eq!(after.len(), 1);
	assert!(
		after[0]
			.resolved_file
			.as_ref()
			.is_some_and(|file| file.name == "keep"),
		"the pin resolves to its folder after the restart: {:?}",
		after[0].resolved_file
	);

	core.shutdown().await.map_err(|e| anyhow::anyhow!("{e}"))?;
	Ok(())
}

/// Release gate "Non-UTF-8 or otherwise unrepresentable names fail visibly"
/// and the known limit "Non-UTF-8 names are retained lossily and logged": a
/// file whose name is not UTF-8 is still a record, under the replacement
/// character, and the walk said so at warning level.
///
/// The warning is read from a child process, because the tracing subscriber
/// is global to the test binary and whichever harness builds first owns it.
#[cfg(unix)]
#[tokio::test]
async fn a_non_utf8_name_is_retained_lossily_and_reported() -> anyhow::Result<()> {
	let printed = in_child("non_utf8_child_walk").await?;
	assert!(
		printed
			.lines()
			.any(|line| line.contains("WARN") && line.contains("file name is not valid UTF-8")),
		"the walk warned about the name it could not keep:\n{printed}"
	);
	Ok(())
}

/// Run one ignored test of this binary in a child process with warnings
/// on, and answer with everything it printed. The harness's fmt layer
/// writes to stdout; the child's own panics go to stderr.
async fn in_child(test: &str) -> anyhow::Result<String> {
	// Its own working directory: `KeyManager::close` creates a redb file
	// named `:memory:` there, and redb's lock on it is per file, so a child
	// sharing the parent's directory cannot shut its core down cleanly.
	let cwd = tempfile::tempdir()?;
	let output = tokio::process::Command::new(std::env::current_exe()?)
		.args([
			test,
			"--exact",
			"--ignored",
			"--nocapture",
			"--test-threads=1",
		])
		.env("RUST_LOG", "sd_core=warn")
		.current_dir(cwd.path())
		.output()
		.await?;
	let printed = format!(
		"{}\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	anyhow::ensure!(output.status.success(), "the child test failed:\n{printed}");
	Ok(printed)
}

/// The walk behind `a_non_utf8_name_is_retained_lossily_and_reported`.
#[cfg(unix)]
#[tokio::test]
#[ignore = "run by a_non_utf8_name_is_retained_lossily_and_reported in a child process"]
async fn non_utf8_child_walk() -> anyhow::Result<()> {
	use std::os::unix::ffi::OsStrExt;

	let harness = IndexingHarnessBuilder::new("fda_non_utf8")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("names").await?;
	dir.write_file("fine.txt", "f").await?;
	let bad = dir
		.path()
		.join(std::ffi::OsStr::from_bytes(b"bad\xFFname.txt"));
	tokio::fs::write(&bad, "b").await?;

	let tracked = dir.track().await?;
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(&tracked.root.join("fine.txt"))
		.await
		.expect("store");
	store.flush().await?;

	let lossy = "bad\u{FFFD}name.txt";
	let entry = sd_store::read::entry_by_path(store.db().pool(), lossy).await?;
	assert!(
		entry.is_some(),
		"the name is retained under the replacement character"
	);
	assert_eq!(store.db().count("file").await?, 2);

	harness.shutdown().await?;
	Ok(())
}

/// A rename over an existing file inside a source lands as one row under
/// `UNIQUE(parent_uuid, title)`: after `a.txt` is
/// renamed over `b.txt` on disk and the store is told, the way the watcher
/// tells it, the store should hold one file named `b.txt` and no `a.txt`.
#[tokio::test]
async fn a_rename_over_an_existing_file_lands_as_one_row() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("fda_rename_over")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("renames").await?;
	dir.write_file("a.txt", "aaaa").await?;
	dir.write_file("b.txt", "bb").await?;
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: dir.path().to_path_buf(),
		name: None,
		unfiltered: true,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	if let Some(job) = tracked.job_id {
		if let Some(walk) = harness
			.library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await?;
		}
	}
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(&tracked.root.join("a.txt"))
		.await
		.expect("store");
	store.flush().await?;
	let a = store.db().resolve_path("a.txt").await?.expect("a");
	assert!(store.db().resolve_path("b.txt").await?.is_some());

	let (from, to) = (tracked.root.join("a.txt"), tracked.root.join("b.txt"));
	tokio::fs::rename(&from, &to).await?;
	let metadata = std::fs::metadata(&to)?;
	store
		.renamed(
			&from,
			&sd_core::ops::indexing::metadata::EntryMetadata {
				path: to.clone(),
				kind: sd_core::ops::indexing::state::EntryKind::File,
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
			},
		)
		.await;
	store.flush().await?;

	assert_eq!(
		store.db().resolve_path("a.txt").await?,
		None,
		"the overwritten name is gone"
	);
	let b = store.db().resolve_path("b.txt").await?.expect("b.txt");
	assert_eq!(b, a, "the moved record keeps its identity at its new name");
	let titles: Vec<(String,)> =
		sqlx::query_as("SELECT title FROM record WHERE type = 'file' ORDER BY title")
			.fetch_all(store.db().pool())
			.await?;
	assert_eq!(
		titles,
		vec![("b.txt".to_string(),)],
		"one row for the one file left on disk"
	);

	harness.shutdown().await?;
	Ok(())
}
