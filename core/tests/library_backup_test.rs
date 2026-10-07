//! A library and its source stores round-trip through backup and restore
//! while the daemon runs, and the manifest catches a tampered copy.

use sd_core::{
	infra::{
		action::{CoreAction, LibraryAction},
		db::entities::{device, space_item},
		query::CoreQuery,
	},
	ops::{
		libraries::backup::{
			manifest::BackupManifest, LibraryBackupAction, LibraryBackupInput,
			LibraryBackupVerifyInput, LibraryBackupVerifyQuery, LibraryRestoreAction,
			LibraryRestoreInput, RestoreMode,
		},
		sources::track::{TrackSourceAction, TrackSourceInput},
		spaces::{
			add_item::{action::AddItemAction, input::AddItemInput},
			create::{action::SpaceCreateAction, input::SpaceCreateInput},
		},
		tags::{
			apply::{action::ApplyTagsAction, input::ApplyTagsInput, input::TagTargets},
			create::{action::CreateTagAction, input::CreateTagInput},
		},
	},
	Core,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// What a store holds, read straight off its file so a live store and a
/// restored one are measured the same way.
#[derive(Debug, PartialEq, Eq)]
struct StoreFacts {
	records: i64,
	contents: i64,
	tag_assertions: i64,
	store_id: Uuid,
	revision: i64,
}

async fn store_facts(db: &sd_store::SourceDb) -> Result<StoreFacts, Error> {
	let (records, contents, tag_assertions): (i64, i64, i64) = sqlx::query_as(
		"SELECT (SELECT COUNT(*) FROM record),
		        (SELECT COUNT(*) FROM content),
		        (SELECT COUNT(*) FROM tag_assertion)",
	)
	.fetch_one(db.pool())
	.await?;
	let revision = db.revision().await?;
	Ok(StoreFacts {
		records,
		contents,
		tag_assertions,
		store_id: revision.store_id,
		revision: revision.value,
	})
}

async fn write_tree(root: &Path, dirs: usize, files: usize, salt: &str) -> Result<usize, Error> {
	let mut written = 0;
	for dir in 0..dirs {
		let subdir = root.join(format!("dir_{dir}"));
		tokio::fs::create_dir_all(&subdir).await?;
		for file in 0..files {
			tokio::fs::write(
				subdir.join(format!("file_{file}.txt")),
				format!("{salt} {dir} {file}"),
			)
			.await?;
			written += 1;
		}
	}
	Ok(written)
}

async fn track(
	core: &Core,
	library: &Arc<sd_core::library::Library>,
	root: PathBuf,
	expected_files: usize,
) -> Result<(Uuid, Arc<sd_core::ops::indexing::SourceStore>), Error> {
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: root,
		name: None,
		unfiltered: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.ok_or("the tracked source has no store")?;
	let deadline = Instant::now() + Duration::from_secs(60);
	loop {
		let contents = store.counts().await.map_or(0, |counts| counts.contents) as usize;
		if contents == expected_files {
			break;
		}
		assert!(
			Instant::now() < deadline,
			"{contents} of {expected_files} files were hashed"
		);
		sleep(Duration::from_millis(50)).await;
	}
	Ok((tracked.id, store))
}

/// What a backup's copy of a store holds, read from the copy itself.
async fn copy_facts(backup_root: &Path, source: Uuid) -> Result<StoreFacts, Error> {
	let copy = backup_root
		.join("sources")
		.join(source.simple().to_string())
		.join("data.db");
	let db = sd_store::SourceManager::open_file_read_only(&copy).await?;
	store_facts(&db).await
}

async fn restored_facts(core: &Core, source: Uuid) -> Result<StoreFacts, Error> {
	let db = core
		.context
		.volume_index()
		.read_store(source)
		.await
		.ok_or("restored source has no readable store")?;
	store_facts(&db).await
}

fn session(core: &Core) -> sd_core::infra::api::SessionContext {
	let device_id = core.context.device_manager.device_id().unwrap_or_default();
	sd_core::infra::api::SessionContext::device_session(device_id, "test".to_string())
}

async fn space_item_count(library: &sd_core::library::Library) -> Result<usize, Error> {
	Ok(space_item::Entity::find()
		.all(library.db().conn())
		.await?
		.len())
}

#[tokio::test]
async fn backup_restores_identically_and_detects_tampering() -> Result<(), Error> {
	let temp = TempDir::new()?;
	let data_dir = temp.path().join("core");
	let core = Core::new(data_dir.clone()).await?;
	let library = core
		.libraries
		.create_library("Backup fixture", None, core.context.clone())
		.await?;

	let root_a = temp.path().join("source-a");
	let root_b = temp.path().join("source-b");
	let files_a = write_tree(&root_a, 4, 5, "a").await?;
	let files_b = write_tree(&root_b, 3, 3, "b").await?;
	let (source_a, store_a) = track(&core, &library, root_a.clone(), files_a).await?;
	let (source_b, store_b) = track(&core, &library, root_b.clone(), files_b).await?;

	// Tags live in the source store as assertions; a space item lives in
	// library.db. Both must survive the round trip.
	let tag = CreateTagAction::from_input(CreateTagInput {
		path: "Backups/Keep".to_string(),
		color: None,
		icon: None,
	})?
	.execute(library.clone(), core.context.clone())
	.await?
	.tag;
	let targets: Vec<Uuid> = store_a
		.db()
		.list_items(3, 0)
		.await?
		.into_iter()
		.map(|row| row.id)
		.collect();
	assert_eq!(targets.len(), 3);
	let applied = ApplyTagsAction::from_input(ApplyTagsInput {
		targets: TagTargets::File(targets),
		tag_ids: vec![tag.id],
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	assert_eq!(applied.targets_tagged, 3);

	let space = SpaceCreateAction::from_input(SpaceCreateInput {
		name: "Pinned".to_string(),
		icon: "folder".to_string(),
		color: "#336699".to_string(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?
	.space;
	AddItemAction::from_input(AddItemInput {
		space_id: space.id,
		group_id: None,
		item_type: sd_core::domain::ItemType::Tag { tag_id: tag.id },
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let items_before = space_item_count(&library).await?;
	assert!(items_before >= 1);

	store_a.flush().await?;
	store_b.flush().await?;
	let facts_a = store_facts(store_a.db()).await?;
	let facts_b = store_facts(store_b.db()).await?;
	assert_eq!(facts_a.records as usize, files_a + 4);
	assert_eq!(facts_a.tag_assertions, 3);

	// A third source is still being walked and hashed while the backup
	// runs, so the store writers are live underneath the copy.
	let root_c = temp.path().join("source-c");
	write_tree(&root_c, 10, 20, "c").await?;
	let tracking = TrackSourceAction::from_input(TrackSourceInput {
		path: root_c.clone(),
		name: None,
		unfiltered: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let writer_root = root_a.clone();
	let writer = tokio::spawn(async move {
		for i in 0..200 {
			let _ = tokio::fs::write(
				writer_root.join(format!("dir_0/churn_{i}.txt")),
				format!("churn {i}"),
			)
			.await;
			sleep(Duration::from_millis(2)).await;
		}
	});

	let backup_dir = temp.path().join("backup-1");
	let backup = LibraryBackupAction::from_input(LibraryBackupInput {
		library_id: library.id(),
		destination: backup_dir.clone(),
		include_sidecars: true,
		include_replicas: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	writer.abort();
	assert!(backup.files >= 4, "library.db, sync.db and three stores");
	assert!(backup_dir.join("manifest.json").is_file());
	assert!(backup
		.sources_without_store
		.iter()
		.all(|id| *id != source_a && *id != source_b));

	let manifest = BackupManifest::load(&backup_dir.join("manifest.json")).await?;
	assert_eq!(manifest.library.id, library.id());
	assert!(manifest.sources.iter().any(|s| s.id == tracking.id));
	let entry_a = manifest
		.sources
		.iter()
		.find(|s| s.id == source_a)
		.and_then(|s| s.store.clone())
		.ok_or("source a has no store entry")?;
	assert_eq!(entry_a.store_id, facts_a.store_id);
	assert!(manifest.verify(&backup_dir).await.is_empty());

	// The churn writer landed in source a's store through its watcher, so
	// the copy is compared with itself rather than with the count taken
	// before the churn; the copy must still carry what was committed then.
	let copy_a = copy_facts(&backup_dir, source_a).await?;
	let copy_b = copy_facts(&backup_dir, source_b).await?;
	assert!(copy_a.records >= facts_a.records);
	assert_eq!(copy_a.tag_assertions, facts_a.tag_assertions);
	assert_eq!(copy_a.store_id, entry_a.store_id);
	assert_eq!(copy_a.revision, entry_a.revision);
	assert_eq!(copy_b, facts_b);

	let verify = LibraryBackupVerifyQuery::from_input(LibraryBackupVerifyInput {
		source: backup_dir.clone(),
	})?
	.execute(core.context.clone(), session(&core))
	.await?;
	assert!(verify.failures.is_empty(), "{:?}", verify.failures);
	assert!(verify.unknown_migrations.is_empty());

	// Restore onto a fresh data directory: a different device, no
	// libraries of its own.
	let fresh_dir = temp.path().join("fresh");
	let fresh = Core::new(fresh_dir.clone()).await?;
	let restored = LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: backup_dir.clone(),
		mode: RestoreMode::New,
		library_id: None,
		force: false,
	})?
	.execute(fresh.context.clone())
	.await?;
	assert_eq!(restored.library_id, library.id());
	assert!(restored.replaced_state.is_none());
	let fresh_library = fresh
		.context
		.get_library(library.id())
		.await
		.ok_or("restored library is not open")?;
	assert_eq!(fresh_library.name().await, "Backup fixture");
	assert_eq!(restored_facts(&fresh, source_a).await?, copy_a);
	assert_eq!(restored_facts(&fresh, source_b).await?, copy_b);
	assert_eq!(space_item_count(&fresh_library).await?, items_before);

	// Hashes of the restored store files equal the manifest's: nothing
	// rewrote them on open.
	for entry in &manifest.files {
		if let Some(rest) = entry.path.strip_prefix("sources/") {
			let (id, name) = rest.split_once('/').ok_or("bad manifest path")?;
			let restored_file = fresh_dir.join("sources").join(id).join(name);
			let (_, hash) =
				sd_core::ops::libraries::backup::snapshot::hash_file(&restored_file).await?;
			assert_eq!(hash, entry.blake3, "{} changed on restore", entry.path);
		}
	}
	fresh.shutdown().await?;

	// Mutate the live library, then restore over it from an archive and
	// prove the mutation is gone.
	let archive = temp.path().join("backup-2.tar.zst");
	LibraryBackupAction::from_input(LibraryBackupInput {
		library_id: library.id(),
		destination: archive.clone(),
		include_sidecars: false,
		include_replicas: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	assert!(archive.is_file());
	let unpacked = temp.path().join("backup-2-unpacked");
	sd_core::ops::libraries::backup::snapshot::unpack(&archive, &unpacked).await?;
	let archive_manifest = BackupManifest::load(&unpacked.join("manifest.json")).await?;
	assert!(archive_manifest.verify(&unpacked).await.is_empty());
	let archive_facts_a = copy_facts(&unpacked, source_a).await?;

	let extra = SpaceCreateAction::from_input(SpaceCreateInput {
		name: "Mutation".to_string(),
		icon: "folder".to_string(),
		color: "#996633".to_string(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?
	.space;
	AddItemAction::from_input(AddItemInput {
		space_id: extra.id,
		group_id: None,
		item_type: sd_core::domain::ItemType::Overview,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let items_mutated = space_item_count(&library).await?;
	assert!(items_mutated > items_before);
	let mutation = ApplyTagsAction::from_input(ApplyTagsInput {
		targets: TagTargets::File(
			store_b
				.db()
				.list_items(2, 0)
				.await?
				.into_iter()
				.map(|row| row.id)
				.collect(),
		),
		tag_ids: vec![tag.id],
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	assert_eq!(mutation.targets_tagged, 2);
	store_b.flush().await?;
	assert_ne!(store_facts(store_b.db()).await?.tag_assertions, 0);

	// Source b's watcher is live and fed while the swap runs, so a store
	// reopened by an event mid-swap would land on the trashed file.
	let churn_root = root_b.clone();
	let churn = tokio::spawn(async move {
		for i in 0..400 {
			let _ = tokio::fs::write(
				churn_root.join(format!("dir_0/swap_{i}.txt")),
				format!("swap {i}"),
			)
			.await;
			sleep(Duration::from_millis(1)).await;
		}
	});
	let replaced = LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: archive.clone(),
		mode: RestoreMode::Replace,
		library_id: None,
		force: false,
	})?
	.execute(core.context.clone())
	.await?;
	churn.abort();
	let trash = replaced
		.replaced_state
		.clone()
		.ok_or("replace kept nothing aside")?;
	assert!(trash.join("library").join("library.db").is_file());
	assert!(trash
		.join("sources")
		.join(source_b.simple().to_string())
		.join("data.db")
		.is_file());
	let reopened = core
		.context
		.get_library(library.id())
		.await
		.ok_or("replaced library is not open")?;
	assert_eq!(space_item_count(&reopened).await?, items_before);
	assert_eq!(restored_facts(&core, source_a).await?, archive_facts_a);
	assert_eq!(restored_facts(&core, source_b).await?.tag_assertions, 0);
	assert_eq!(
		restored_facts(&core, source_b).await?.store_id,
		facts_b.store_id
	);
	// A writer opened after the swap writes to the restored file.
	let live_b = core
		.context
		.volume_index()
		.store_for(&root_b)
		.await
		.ok_or("source b has no store after restore")?;
	assert_eq!(store_facts(live_b.db()).await?.store_id, facts_b.store_id);
	let registered: Vec<Uuid> = core
		.context
		.volume_index()
		.sources()
		.into_iter()
		.map(|status| status.id)
		.collect();
	assert!(registered.contains(&source_a) && registered.contains(&source_b));

	// Writes made through the live handle reach the file on disk under
	// sources/, not the handle's pre-swap inode in the trash.
	let on_disk_b = data_dir
		.join("sources")
		.join(source_b.simple().to_string())
		.join("data.db");
	let before = facts_b.revision;
	tokio::fs::write(root_b.join("dir_1/after_restore.txt"), "after").await?;
	let deadline = Instant::now() + Duration::from_secs(30);
	loop {
		live_b.flush().await?;
		let now = sd_store::SourceManager::open_file_read_only(&on_disk_b).await?;
		if store_facts(&now).await?.revision > before {
			break;
		}
		assert!(
			Instant::now() < deadline,
			"the restored store on disk never saw the post-restore write"
		);
		sleep(Duration::from_millis(100)).await;
	}

	// Tampering with one byte of a store copy is caught by verify and
	// refused by restore before anything is touched.
	let tampered = backup_dir
		.join("sources")
		.join(source_a.simple().to_string())
		.join("data.db");
	let mut bytes = tokio::fs::read(&tampered).await?;
	let last = bytes.len() - 1;
	bytes[last] ^= 0xff;
	tokio::fs::write(&tampered, bytes).await?;
	let verify = LibraryBackupVerifyQuery::from_input(LibraryBackupVerifyInput {
		source: backup_dir.join("manifest.json"),
	})?
	.execute(core.context.clone(), session(&core))
	.await?;
	assert_eq!(verify.failures.len(), 1);
	assert!(verify.failures[0].path.ends_with("data.db"));
	let refused = LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: backup_dir.clone(),
		mode: RestoreMode::Replace,
		library_id: None,
		force: false,
	})?
	.execute(core.context.clone())
	.await;
	let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
	assert!(
		message.contains("does not match its manifest"),
		"restore accepted a tampered backup: {message}"
	);
	assert!(core.context.get_library(library.id()).await.is_some());

	// A library other devices are members of is not replaced without force.
	let reopened = core
		.context
		.get_library(library.id())
		.await
		.ok_or("library is not open")?;
	device::ActiveModel {
		uuid: Set(Uuid::now_v7()),
		name: Set("Other laptop".to_string()),
		slug: Set("other-laptop".to_string()),
		os: Set("linux".to_string()),
		network_addresses: Set(serde_json::json!([])),
		is_online: Set(false),
		last_seen_at: Set(chrono::Utc::now()),
		capabilities: Set(serde_json::json!({})),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
		sync_enabled: Set(true),
		..Default::default()
	}
	.insert(reopened.db().conn())
	.await?;
	let refused = LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: archive.clone(),
		mode: RestoreMode::Replace,
		library_id: None,
		force: false,
	})?
	.execute(core.context.clone())
	.await;
	let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
	assert!(
		message.contains("Other laptop"),
		"restore ignored other members: {message}"
	);
	let forced = LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: archive,
		mode: RestoreMode::Replace,
		library_id: None,
		force: true,
	})?
	.execute(core.context.clone())
	.await?;
	assert_eq!(forced.library_id, library.id());

	core.shutdown().await?;
	Ok(())
}
