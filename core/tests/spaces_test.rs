//! Spaces: any folder can be one, and a workspace space is created empty.
//!
//! `docs/core/design/spaces.md`. A space lives in its source's store as an
//! assertion; these tests drive the library actions the CLI and the app call.

mod helpers;

use helpers::*;
use sd_core::{
	domain::{Space, SpaceKind},
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	ops::spaces::{
		SpaceCreateAction, SpaceCreateInput, SpaceDeleteAction, SpaceDeleteInput, SpaceGetQuery,
		SpaceGetQueryInput, SpaceUpdateAction, SpaceUpdateInput, SpacesListQuery,
		SpacesListQueryInput,
	},
};
use std::path::PathBuf;
use uuid::Uuid;

fn scope(read: &[&str], skip: &[&str]) -> sd_store::DeepScope {
	sd_store::DeepScope {
		read: read.iter().map(|s| s.to_string()).collect(),
		skip: skip.iter().map(|s| s.to_string()).collect(),
	}
}

fn session(harness: &IndexingHarness) -> SessionContext {
	let device_id = sd_core::device::get_current_device_id();
	let device_name = sd_core::device::get_current_device_slug();
	let mut session = SessionContext::device_session(device_id, device_name);
	session.current_library_id = Some(harness.library.id());
	session
}

async fn create(
	harness: &IndexingHarness,
	kind: SpaceKind,
	path: Option<PathBuf>,
	name: Option<&str>,
) -> anyhow::Result<Space> {
	Ok(SpaceCreateAction::from_input(SpaceCreateInput {
		name: name.map(str::to_string),
		kind,
		path,
		content_indexing: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space)
}

async fn list(harness: &IndexingHarness) -> Vec<Space> {
	SpacesListQuery::from_input(SpacesListQueryInput { source_id: None })
		.expect("input")
		.execute(harness.core.context.clone(), session(harness))
		.await
		.expect("list")
		.spaces
}

#[tokio::test]
async fn a_folder_in_a_source_becomes_a_space_and_stops_being_one() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("spaces_folder")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("home").await?;
	dir.write_file("Documents/notes.md", "notes").await?;
	let tracked = dir.track().await?;
	let documents = tracked.root.join("Documents");

	let space = create(&harness, SpaceKind::Folder, Some(documents.clone()), None).await?;
	assert_eq!(space.name, "Documents");
	assert_eq!(space.kind, SpaceKind::Folder);
	assert_eq!(space.source_id, tracked.id);
	assert_eq!(space.root, "Documents");
	assert_eq!(space.path.as_deref(), Some(documents.as_path()));
	assert!(space.content_indexing, "content indexing is on by default");
	assert_eq!(list(&harness).await, vec![space.clone()]);

	assert!(
		create(&harness, SpaceKind::Folder, Some(documents.clone()), None)
			.await
			.is_err(),
		"one folder is one space"
	);

	let updated = SpaceUpdateAction::from_input(SpaceUpdateInput {
		id: space.id,
		name: Some("Notes".into()),
		content_indexing: Some(false),
		versioning: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space;
	assert_eq!(updated.name, "Notes");
	assert!(!updated.content_indexing);
	let got = SpaceGetQuery::from_input(SpaceGetQueryInput { id: space.id })
		.expect("input")
		.execute(harness.core.context.clone(), session(&harness))
		.await?
		.space;
	assert_eq!(got, updated);

	SpaceDeleteAction::from_input(SpaceDeleteInput { id: space.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	assert!(list(&harness).await.is_empty());
	assert!(
		documents.join("notes.md").exists(),
		"deleting a space leaves its files"
	);
	assert!(
		SpaceGetQuery::from_input(SpaceGetQueryInput { id: space.id })
			.expect("input")
			.execute(harness.core.context.clone(), session(&harness))
			.await
			.is_err()
	);

	// The folder can be a space again after its old one was deleted.
	create(&harness, SpaceKind::Folder, Some(documents), Some("Again")).await?;
	assert_eq!(list(&harness).await.len(), 1);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn a_folder_no_source_covers_is_tracked_when_it_becomes_a_space() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("spaces_untracked")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("project").await?;
	dir.write_file("README.md", "hello").await?;

	let space = create(
		&harness,
		SpaceKind::Folder,
		Some(dir.path().to_path_buf()),
		None,
	)
	.await?;
	let index = harness.core.context.volume_index();
	assert_eq!(index.source_id_for(dir.path()), Some(space.source_id));
	assert_eq!(space.root, "");
	assert!(create(
		&harness,
		SpaceKind::Folder,
		Some(harness.temp_path().join("missing")),
		None
	)
	.await
	.is_err());

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn a_workspace_space_is_an_empty_folder_tracked_as_its_own_source() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("spaces_workspace")
		.disable_watcher()
		.build()
		.await?;

	let space = create(&harness, SpaceKind::Workspace, None, Some("research-agent")).await?;
	let path = space.path.clone().expect("located here");
	assert_eq!(space.kind, SpaceKind::Workspace);
	assert_eq!(space.name, "research-agent");
	assert_eq!(
		path,
		harness
			.core
			.context
			.data_dir
			.join("spaces")
			.join(space.id.to_string())
	);
	assert!(path.is_dir());
	assert_eq!(std::fs::read_dir(&path)?.count(), 0, "created empty");
	assert!(
		!sd_core::config::is_managed(&path.join("draft.md")),
		"a workspace under the data directory is walked and watched"
	);
	let index = harness.core.context.volume_index();
	assert_eq!(index.source_id_for(&path), Some(space.source_id));
	assert_eq!(index.source_root(space.source_id), Some(path.clone()));
	assert_eq!(space.root, "");
	assert_eq!(list(&harness).await, vec![space]);

	// A chosen folder must be empty.
	let chosen = harness.temp_path().join("agent-work");
	std::fs::create_dir_all(&chosen)?;
	std::fs::write(chosen.join("x"), "x")?;
	assert!(
		create(&harness, SpaceKind::Workspace, Some(chosen.clone()), None)
			.await
			.is_err()
	);
	std::fs::remove_file(chosen.join("x"))?;
	let chosen_space = create(&harness, SpaceKind::Workspace, Some(chosen.clone()), None).await?;
	assert_eq!(chosen_space.path.as_deref(), Some(chosen.as_path()));
	assert_eq!(chosen_space.name, "Workspace");
	assert_ne!(chosen_space.id, Uuid::nil());

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn a_deep_scope_set_before_spaces_becomes_spaces_that_decide_what_is_read(
) -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("spaces_adopt_deep_scope")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("home").await?;
	dir.write_file("Projects/plan.md", "plan").await?;
	dir.write_file("Archive/old.md", "old").await?;
	let tracked = dir.track().await?;
	let index = harness.core.context.volume_index();
	let store = index
		.store_for(&tracked.root)
		.await
		.expect("the source has a store");
	let device = harness.core.context.device_manager.device_id()?;
	store
		.db()
		.set_deep_scope(
			&["Projects".to_string()],
			&sd_core::ops::tags::stamp::assertion_stamp(device),
		)
		.await?;
	assert_eq!(store.deep_scope().await?, scope(&["Projects"], &[]));
	assert!(list(&harness).await.is_empty(), "listing changes nothing");

	// What opening the library does.
	sd_core::ops::spaces::store::adopt_library(&harness.library, &harness.core.context).await;
	let spaces = list(&harness).await;
	assert_eq!(spaces.len(), 1, "{spaces:?}");
	let projects = &spaces[0];
	assert_eq!(projects.root, "Projects");
	assert_eq!(projects.name, "Projects");
	assert!(projects.content_indexing);
	assert_eq!(
		store.db().deep_scope().await?,
		Some(Vec::new()),
		"the old list is marked adopted"
	);
	assert_eq!(store.deep_scope().await?, scope(&["Projects"], &[]));
	sd_core::ops::spaces::store::adopt_library(&harness.library, &harness.core.context).await;
	assert_eq!(list(&harness).await, spaces, "adopting happens once");

	let archive = create(
		&harness,
		SpaceKind::Folder,
		Some(tracked.root.join("Archive")),
		None,
	)
	.await?;
	assert_eq!(
		store.deep_scope().await?,
		scope(&["Archive", "Projects"], &[])
	);
	SpaceUpdateAction::from_input(SpaceUpdateInput {
		id: archive.id,
		name: None,
		content_indexing: Some(false),
		versioning: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert_eq!(
		store.deep_scope().await?,
		scope(&["Projects"], &["Archive"]),
		"a space with content indexing off is skipped"
	);
	SpaceDeleteAction::from_input(SpaceDeleteInput { id: projects.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	assert!(
		store.deep_scope().await?.is_empty(),
		"a deleted space's folder is no longer read, and the old list stays adopted"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn a_space_with_content_indexing_off_is_not_read_inside_a_read_source() -> anyhow::Result<()>
{
	let harness = IndexingHarnessBuilder::new("spaces_reading_off")
		.disable_watcher()
		.build()
		.await?;
	let dir = harness.create_test_dir("drive").await?;
	dir.write_file("notes.md", "notes").await?;
	dir.write_file("Private/diary.md", "diary").await?;
	let tracked = dir.track().await?;
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.expect("the source has a store");
	assert_eq!(store.deep_scope().await?, scope(&[""], &[]));

	let private = SpaceCreateAction::from_input(SpaceCreateInput {
		name: None,
		kind: SpaceKind::Folder,
		path: Some(tracked.root.join("Private")),
		content_indexing: Some(false),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space;
	assert_eq!(store.deep_scope().await?, scope(&[""], &["Private"]));

	SpaceUpdateAction::from_input(SpaceUpdateInput {
		id: private.id,
		name: None,
		content_indexing: Some(true),
		versioning: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert_eq!(store.deep_scope().await?, scope(&["", "Private"], &[]));

	harness.shutdown().await?;
	Ok(())
}
