//! Acceptance for the core half of Add to Library
//! (`docs/plans/2026-09-16-add-to-library.md`,
//! `docs/core/acceptance/add-to-library.md`): one add operation resolves the
//! library's defaults with the add's overrides, tracks the containing volume
//! once, places the store where the effective settings say, keeps its own
//! directories out of the index, and removal keeps the catalog so re-adding
//! the scope reopens it under the identity it carries.
//!
//! One `Core` over temporary directories; nothing here needs a second daemon
//! or a real drive. The temporary root sits on whatever volume the runner's
//! home is on, which detection reports, so the volume rows below are the
//! runner's own data volume.

mod helpers;

use std::path::Path;

use helpers::*;
use sd_core::{
	infra::{action::LibraryAction, api::SessionContext, db::entities, query::LibraryQuery},
	library::AddOverrides,
	ops::{
		indexing::{metadata::EntryMetadata, sources::StorePlacement, state::EntryKind},
		sources::{
			delete::action::{DeleteSourceAction, DeleteSourceInput},
			list::query::ListSourcesQuery,
			track::action::{TrackSourceAction, TrackSourceInput, TrackSourceOutput},
		},
	},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

async fn track(
	harness: &IndexingHarness,
	path: &Path,
	name: Option<&str>,
	overrides: AddOverrides,
) -> anyhow::Result<TrackSourceOutput> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: path.to_path_buf(),
		name: name.map(str::to_string),
		overrides,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	if let Some(job) = output.job_id {
		if let Some(walk) = harness
			.library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await?;
		}
	}
	Ok(output)
}

async fn untrack(
	harness: &IndexingHarness,
	id: Uuid,
	delete_catalog: bool,
) -> anyhow::Result<sd_core::ops::sources::delete::action::DeleteSourceOutput> {
	Ok(DeleteSourceAction::from_input(DeleteSourceInput {
		source_id: id.to_string(),
		delete_catalog,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

async fn listed_ids(harness: &IndexingHarness) -> Vec<Uuid> {
	let device_id = sd_core::device::get_current_device_id();
	let mut session =
		SessionContext::device_session(device_id, sd_core::device::get_current_device_slug());
	session.current_library_id = Some(harness.library.id());
	ListSourcesQuery::from_input(sd_core::ops::sources::list::query::ListSourcesInput {
		data_type: None,
	})
	.unwrap()
	.execute(harness.core.context.clone(), session)
	.await
	.expect("sources.list")
	.into_iter()
	.map(|source| source.id)
	.collect()
}

async fn volume_rows(harness: &IndexingHarness, uuid: Uuid) -> u64 {
	entities::volume::Entity::find()
		.filter(entities::volume::Column::Uuid.eq(uuid))
		.count(harness.library.db().conn())
		.await
		.expect("count volumes")
}

/// The identity a source's store holds for a file, which is what tags and
/// every other assertion attach to.
async fn record_id(harness: &IndexingHarness, path: &Path) -> Uuid {
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(path)
		.await
		.expect("the path is under a registered source");
	let metadata = std::fs::metadata(path).expect("metadata");
	store
		.identify_one(
			&EntryMetadata {
				path: path.to_path_buf(),
				kind: EntryKind::File,
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
			None,
		)
		.await
		.expect("identified")
}

/// Rows 1 to 7 of the matrix: defaults, overrides, one volume registration
/// for every folder on the drive, managed directories excluded, removal
/// keeping the catalog, re-adding reopening it, and deletion being explicit.
#[tokio::test]
async fn add_to_library_resolves_defaults_tracks_the_volume_and_keeps_the_catalog(
) -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("add_to_library_acceptance")
		.disable_watcher()
		.build()
		.await?;

	let photos = harness.create_test_dir("Photos").await?;
	photos.write_file("a.jpg", "a").await?;
	photos.write_file("2024/b.jpg", "b").await?;
	let video = harness.create_test_dir("Video").await?;
	video.write_file("clip.mov", "clip").await?;

	// 1. Defaults: the library's Adding content settings, as the plan
	// proposes them, with the store in the library.
	let first = track(&harness, photos.path(), None, AddOverrides::default()).await?;
	assert_eq!(first.settings.placement, StorePlacement::InLibrary);
	assert!(first.settings.keep_offline_copy);
	assert!(!first.settings.unfiltered);
	assert!(first.settings.identify_content);
	assert_eq!(first.name, "Photos");
	assert!(!first.catalog_reused);
	let photos_store = first.store_path.clone().expect("a store on this machine");
	assert!(
		photos_store.starts_with(
			harness
				.core
				.context
				.volume_index()
				.source_dirs()
				.unwrap()
				.root()
		),
		"an in-library store sits under the data directory: {}",
		photos_store.display()
	);
	assert!(photos_store.join("data.db").exists());
	assert!(
		photos_store.join("source.json").exists(),
		"the store carries its descriptor"
	);

	// 3. The folder's volume is registered in the library by the add, and
	// the index maps it so its attachment follows the drive.
	let volume = first
		.volume_uuid
		.expect("the temp root sits on a detected volume");
	assert_eq!(volume_rows(&harness, volume).await, 1);
	assert_eq!(
		harness.core.context.volume_index().volume_mounted(volume),
		Some(true)
	);

	// 2. Overrides: on-source placement and unfiltered capture for one add,
	// without touching the library's defaults.
	let second = track(
		&harness,
		video.path(),
		Some("Clips"),
		AddOverrides {
			placement: Some(StorePlacement::OnSource),
			unfiltered: Some(true),
			identify_content: Some(false),
			..AddOverrides::default()
		},
	)
	.await?;
	assert_eq!(second.settings.placement, StorePlacement::OnSource);
	assert!(second.settings.unfiltered);
	assert!(!second.settings.identify_content);
	assert!(second.settings.keep_offline_copy, "untouched default");
	assert_eq!(second.name, "Clips");
	let video_store = second.store_path.clone().expect("a store on this machine");
	assert_eq!(
		video_store,
		video
			.path()
			.join(".spacedrive")
			.join("sources")
			.join(second.id.simple().to_string())
	);
	assert!(video_store.join("data.db").exists());
	let records = {
		let mut session = SessionContext::device_session(
			sd_core::device::get_current_device_id(),
			sd_core::device::get_current_device_slug(),
		);
		session.current_library_id = Some(harness.library.id());
		sd_core::ops::sources::list_records::query::ListSourceRecordsQuery::from_input(
			sd_core::ops::sources::list_records::query::ListSourceRecordsInput {
				source_id: second.id.to_string(),
				limit: 100,
				offset: 0,
			},
		)
		.unwrap()
		.execute(harness.core.context.clone(), session)
		.await?
	};
	assert!(
		!records.is_empty(),
		"the library's read queries reach the on-source store"
	);
	let defaults = harness.library.config().await.settings.adding;
	assert_eq!(defaults.placement, StorePlacement::InLibrary);
	assert!(
		!defaults.unfiltered,
		"an override never writes the defaults"
	);

	// 3. A second folder on the same drive reuses the volume registration.
	assert_eq!(second.volume_uuid, Some(volume));
	assert_eq!(volume_rows(&harness, volume).await, 1);

	// 4. The on-source store is inside the unfiltered source's scope and is
	// not indexed, by either the source or an enclosing one.
	let cache = harness.core.context.volume_index();
	let video_source = cache.store_for(video.path()).await.unwrap();
	assert!(
		video_source
			.contains_path(&video.path().join("clip.mov"))
			.await
	);
	assert!(
		!video_source
			.contains_path(&video.path().join(".spacedrive"))
			.await,
		"an unfiltered source does not ingest its own store"
	);
	assert!(
		!video_source
			.contains_path(&video_store.join("data.db"))
			.await,
		"nor the files inside it"
	);
	let enclosing = track(&harness, harness.temp_path(), None, AddOverrides::default()).await?;
	let enclosing_store = cache.store_for(harness.temp_path()).await.unwrap();
	assert_eq!(enclosing_store.id(), enclosing.id);
	assert!(
		!enclosing_store
			.contains_path(&video.path().join(".spacedrive"))
			.await,
		"an enclosing source excludes the managed directory too"
	);
	assert!(
		!enclosing_store
			.contains_path(&harness.temp_path().join("data"))
			.await,
		"the library's own data directory is never indexed"
	);

	// 5. Removal keeps the catalog. The registration is gone, the store and
	// the identities it holds are not.
	let a_jpg = photos.path().join("a.jpg");
	let identity_before = record_id(&harness, &a_jpg).await;
	let removed = untrack(&harness, first.id, false).await?;
	assert!(removed.deleted);
	assert!(!removed.catalog_deleted);
	assert_eq!(removed.catalog_path, Some(photos_store.clone()));
	assert!(photos_store.join("data.db").exists(), "the catalog stays");
	assert!(!listed_ids(&harness).await.contains(&first.id));
	assert!(cache
		.store_for(&a_jpg)
		.await
		.is_none_or(|s| s.id() != first.id));
	assert_eq!(
		volume_rows(&harness, volume).await,
		1,
		"removing a folder does not forget the drive"
	);

	// 6. Re-adding the scope reopens the catalog under the same identity:
	// the file keeps the record id its assertions point at.
	let again = track(&harness, photos.path(), None, AddOverrides::default()).await?;
	assert_eq!(again.id, first.id, "the store's identity is adopted");
	assert!(again.catalog_reused);
	assert_eq!(again.store_path, Some(photos_store.clone()));
	assert_eq!(record_id(&harness, &a_jpg).await, identity_before);
	assert!(listed_ids(&harness).await.contains(&first.id));

	// 7. Deleting the catalog is explicit, and a later add starts over.
	let deleted = untrack(&harness, first.id, true).await?;
	assert!(deleted.catalog_deleted);
	assert!(!photos_store.exists());
	let fresh = track(&harness, photos.path(), None, AddOverrides::default()).await?;
	assert_ne!(fresh.id, first.id);
	assert!(!fresh.catalog_reused);

	// A re-track names nothing, so it changes nothing: the on-source source
	// keeps its placement, its unfiltered capture and its opt-out from
	// content identification whatever the library defaults say.
	let retracked = track(&harness, video.path(), None, AddOverrides::default()).await?;
	assert_eq!(retracked.id, second.id);
	assert_eq!(retracked.settings, second.settings);
	assert!(retracked.catalog_reused);

	// Removing the on-source source and adding it back with the default
	// placement reopens the catalog on the drive rather than starting an
	// empty one in the library.
	let clip = video.path().join("clip.mov");
	let clip_identity = record_id(&harness, &clip).await;
	let removed = untrack(&harness, second.id, false).await?;
	assert_eq!(removed.catalog_path, Some(video_store.clone()));
	let readded = track(&harness, video.path(), None, AddOverrides::default()).await?;
	assert_eq!(readded.id, second.id);
	assert_eq!(readded.settings.placement, StorePlacement::OnSource);
	assert_eq!(readded.store_path, Some(video_store.clone()));
	assert!(readded.catalog_reused);
	assert_eq!(record_id(&harness, &clip).await, clip_identity);
	assert!(
		!harness
			.core
			.context
			.volume_index()
			.source_dirs()
			.unwrap()
			.source_dir(second.id)
			.join("data.db")
			.exists(),
		"no empty in-library store was started beside the drive's catalog"
	);

	// A changed default applies to the next add and moves nothing.
	harness
		.library
		.update_config(|config| config.settings.adding.unfiltered = true)
		.await?;
	let music = harness.create_test_dir("Music").await?;
	music.write_file("song.mp3", "song").await?;
	let third = track(&harness, music.path(), None, AddOverrides::default()).await?;
	assert!(third.settings.unfiltered);
	assert_eq!(third.settings.placement, StorePlacement::InLibrary);
	assert_eq!(
		cache.store_dir(second.id),
		Some(video_store),
		"existing stores stay where they were placed"
	);

	harness.shutdown().await
}
