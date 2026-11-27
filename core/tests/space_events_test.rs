//! Space events: every change inside a space is logged with who made it and
//! how, and nothing is logged for a change outside every space.
//!
//! `docs/core/design/spaces.md`. Agent writes are attributed to the grant's
//! agent, changes the watcher sees to this device as "changed outside
//! Spacedrive".

#![cfg(unix)]

mod helpers;

use std::{path::Path, time::Duration};

use helpers::*;
use sd_core::{
	domain::{Space, SpaceKind},
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	ops::{
		actors::ActorKind,
		agent_write::{write_file, Precondition, WriteFileInput},
		grants::{
			create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
			GrantCap,
		},
		spaces::{
			ChangeOrder, SpaceChange, SpaceChangesInput, SpaceChangesQuery, SpaceCreateAction,
			SpaceCreateInput,
		},
	},
};

fn session(harness: &IndexingHarness) -> SessionContext {
	let device_id = sd_core::device::get_current_device_id();
	let device_name = sd_core::device::get_current_device_slug();
	let mut session = SessionContext::device_session(device_id, device_name);
	session.current_library_id = Some(harness.library.id());
	session
}

async fn space(harness: &IndexingHarness, path: &Path) -> anyhow::Result<Space> {
	Ok(SpaceCreateAction::from_input(SpaceCreateInput {
		name: None,
		kind: SpaceKind::Folder,
		path: Some(path.to_path_buf()),
		content_indexing: None,
		protected: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space)
}

async fn changes(harness: &IndexingHarness, space: &Space) -> Vec<SpaceChange> {
	SpaceChangesQuery::from_input(SpaceChangesInput {
		id: space.id,
		order: None,
		since: None,
		before: None,
		path: None,
		limit: None,
	})
	.expect("input")
	.execute(harness.core.context.clone(), session(harness))
	.await
	.expect("changes")
	.changes
}

async fn grant(
	harness: &IndexingHarness,
	path: &Path,
	label: &str,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(GrantCap::ReadWrite),
		ttl_seconds: None,
		label: Some(label.to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

async fn write(
	harness: &IndexingHarness,
	token: &str,
	path: &str,
	content: &str,
	expect: Option<Precondition>,
) -> anyhow::Result<()> {
	write_file(
		&harness.core.context,
		WriteFileInput {
			token: token.to_string(),
			path: path.into(),
			content: content.to_string(),
			encoding: None,
			offset: None,
			expect,
		},
	)
	.await
	.map_err(|e| anyhow::anyhow!("{e:?}"))?;
	Ok(())
}

#[tokio::test]
async fn agent_writes_inside_a_space_are_logged_and_outside_are_not() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("space_events_agent")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/README.md", "hello").await?;
	home.write_file("other/README.md", "elsewhere").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	assert!(changes(&harness, &project).await.is_empty());

	let inside = grant(&harness, &tracked.root.join("project"), "builder").await?;
	let outside = grant(&harness, &tracked.root.join("other"), "elsewhere").await?;
	write(&harness, &inside.token, "notes.md", "first", None).await?;
	write(&harness, &outside.token, "notes.md", "first", None).await?;

	let logged = changes(&harness, &project).await;
	assert_eq!(
		logged.len(),
		1,
		"only the write inside the space: {logged:?}"
	);
	let created = &logged[0];
	assert_eq!(created.op, "create");
	assert_eq!(created.path, "notes.md");
	assert_eq!(created.origin, "agent_write");
	assert_eq!(created.grant_id, Some(inside.grant.id));
	assert_eq!(created.actor.kind, ActorKind::Agent);
	assert_eq!(created.actor.label, "builder");
	assert!(created.after_hash.is_some(), "a write knows what it wrote");
	assert!(
		created.undo_id.is_some(),
		"a write can be undone from its event"
	);

	write(
		&harness,
		&inside.token,
		"notes.md",
		"second",
		Some(Precondition {
			revision: None,
			content_hash: created.after_hash.clone(),
		}),
	)
	.await?;
	let logged = changes(&harness, &project).await;
	assert_eq!(logged.len(), 2, "{logged:?}");
	let modified = &logged[1];
	assert_eq!(modified.op, "modify");
	assert_eq!(
		modified.actor.id, created.actor.id,
		"one agent across its writes"
	);
	assert_eq!(modified.before_hash, created.after_hash);
	assert_ne!(modified.after_hash, created.after_hash);

	let next = SpaceChangesQuery::from_input(SpaceChangesInput {
		id: project.id,
		order: None,
		since: Some(created.seq),
		before: None,
		path: None,
		limit: None,
	})
	.expect("input")
	.execute(harness.core.context.clone(), session(&harness))
	.await?;
	assert_eq!(next.changes.len(), 1);
	assert_eq!(next.next, modified.seq);

	write(&harness, &inside.token, "drafts/idea.md", "idea", None).await?;
	let newest = |path: Option<&str>, before: Option<i64>| {
		SpaceChangesQuery::from_input(SpaceChangesInput {
			id: project.id,
			order: Some(ChangeOrder::Newest),
			since: None,
			before,
			path: path.map(str::to_string),
			limit: Some(1),
		})
		.expect("input")
		.execute(harness.core.context.clone(), session(&harness))
	};
	let feed = newest(None, None).await?;
	assert_eq!(feed.changes[0].path, "drafts/idea.md");
	let older = newest(None, Some(feed.next)).await?;
	assert!(older.changes[0].seq < feed.next, "before pages backwards");
	let last_for_notes = newest(Some("notes.md"), None).await?;
	assert_eq!(last_for_notes.changes[0].seq, modified.seq);
	let in_drafts = newest(Some("drafts"), None).await?;
	assert_eq!(in_drafts.changes[0].path, "drafts/idea.md");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn watcher_changes_inside_a_space_are_logged_as_outside_spacedrive() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("space_events_watcher")
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/README.md", "hello").await?;
	home.write_file("other/README.md", "elsewhere").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	tokio::time::sleep(Duration::from_secs(1)).await;

	tokio::fs::write(tracked.root.join("other/outside.txt"), "outside").await?;
	tokio::fs::write(tracked.root.join("project/inside.txt"), "inside").await?;

	let mut logged = Vec::new();
	for _ in 0..100 {
		logged = changes(&harness, &project).await;
		if logged.iter().any(|change| change.path == "inside.txt") {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	let seen = logged
		.iter()
		.find(|change| change.path == "inside.txt")
		.unwrap_or_else(|| panic!("the watcher's change was not logged: {logged:?}"));
	assert_eq!(seen.origin, "watcher");
	assert_eq!(seen.actor.kind, ActorKind::Device);
	assert_eq!(
		seen.actor.id,
		harness.core.context.device_manager.device_id()?
	);
	assert_eq!(seen.grant_id, None);
	assert!(
		seen.summary.contains("changed outside Spacedrive on"),
		"{}",
		seen.summary
	);
	assert!(
		logged.iter().all(|change| !change.path.contains("outside")),
		"nothing outside the space: {logged:?}"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn a_folder_copied_through_the_app_is_the_persons_change_not_an_outside_one(
) -> anyhow::Result<()> {
	use sd_core::{
		domain::addressing::{SdPath, SdPathBatch},
		ops::files::copy::{
			action::FileConflictResolution,
			job::{CopyOptions, FileCopyJob},
		},
	};

	let harness = IndexingHarnessBuilder::new("space_events_app_copy")
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/drafts/kept.md", "kept").await?;
	home.write_file("project/drafts/plan.md", "old plan")
		.await?;
	home.write_file("incoming/drafts/plan.md", "new plan")
		.await?;
	home.write_file("incoming/drafts/idea.md", "idea").await?;
	home.write_file("incoming/drafts/deep/note.md", "note")
		.await?;
	let tracked = home.track().await?;
	tokio::time::sleep(Duration::from_secs(2)).await;
	let project = space(&harness, &tracked.root.join("project")).await?;
	tokio::time::sleep(Duration::from_secs(1)).await;
	let before = changes(&harness, &project)
		.await
		.last()
		.map_or(0, |c| c.seq);

	// A folder onto a folder already there merges leaf by leaf, and the merge
	// journals its effects in a batch after it has made them.
	let copy = FileCopyJob::new(
		SdPathBatch::new(vec![SdPath::local(tracked.root.join("incoming/drafts"))]),
		SdPath::local(tracked.root.join("project")),
	)
	.with_options(CopyOptions {
		conflict_resolution: Some(FileConflictResolution::Overwrite),
		..CopyOptions::default()
	});
	harness.library.jobs().dispatch(copy).await?.wait().await?;
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/drafts/plan.md")).await?,
		"new plan"
	);
	// Long enough for the watcher to have seen and handled every change.
	tokio::time::sleep(Duration::from_secs(4)).await;

	let logged: Vec<SpaceChange> = changes(&harness, &project)
		.await
		.into_iter()
		.filter(|change| change.seq > before)
		.collect();
	assert!(
		logged.iter().any(|change| change.path == "drafts/idea.md"),
		"the copy is logged: {logged:?}"
	);
	assert!(
		logged
			.iter()
			.any(|change| change.path == "drafts/deep/note.md"),
		"{logged:?}"
	);
	let outside: Vec<_> = logged
		.iter()
		.filter(|change| change.origin != "app_op")
		.collect();
	assert!(
		outside.is_empty(),
		"every change is the person's, none outside Spacedrive: {outside:?}"
	);
	assert!(logged
		.iter()
		.all(|change| change.actor.kind == ActorKind::Person));

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn undoing_an_agent_move_is_one_event_by_the_person() -> anyhow::Result<()> {
	use sd_core::ops::{
		agent_write::{move_path, stat, MoveInput, StatInput},
		files::undo::UndoJob,
	};

	let harness = IndexingHarnessBuilder::new("space_events_undo")
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/draft.md", "draft").await?;
	home.write_file("project/done/.keep", "").await?;
	let tracked = home.track().await?;
	tokio::time::sleep(Duration::from_secs(2)).await;
	let project = space(&harness, &tracked.root.join("project")).await?;
	let agent = grant(&harness, &tracked.root.join("project"), "mover").await?;
	tokio::time::sleep(Duration::from_secs(1)).await;

	let current = stat(
		&harness.core.context,
		StatInput {
			token: agent.token.clone(),
			path: "draft.md".into(),
		},
	)
	.await
	.map_err(|e| anyhow::anyhow!("{e:?}"))?;
	let moved = move_path(
		&harness.core.context,
		MoveInput {
			token: agent.token.clone(),
			from: "draft.md".into(),
			to: "done/draft.md".into(),
			expect: Precondition {
				revision: None,
				content_hash: current.content_hash.clone(),
			},
		},
	)
	.await
	.map_err(|e| anyhow::anyhow!("{e:?}"))?;
	tokio::time::sleep(Duration::from_secs(3)).await;
	let after_move = changes(&harness, &project).await;
	let last = after_move.last().expect("the move is logged");
	assert_eq!(last.origin, "agent_write");

	harness
		.library
		.jobs()
		.dispatch(UndoJob::new(moved.undo.expect("undoable"), None))
		.await?
		.wait()
		.await?;
	assert!(tracked.root.join("project/draft.md").exists());
	tokio::time::sleep(Duration::from_secs(4)).await;

	let undone: Vec<SpaceChange> = changes(&harness, &project)
		.await
		.into_iter()
		.filter(|change| change.seq > last.seq)
		.collect();
	assert_eq!(undone.len(), 1, "one event for the undo: {undone:?}");
	assert_eq!(undone[0].op, "move");
	assert_eq!(undone[0].path, "done/draft.md");
	assert_eq!(undone[0].to_path.as_deref(), Some("draft.md"));
	assert_eq!(undone[0].origin, "app_op");
	assert_eq!(undone[0].actor.kind, ActorKind::Person);
	let me =
		sd_core::ops::actors::ActorsMeQuery::from_input(sd_core::ops::actors::ActorsMeInput {})
			.expect("input")
			.execute(harness.core.context.clone(), session(&harness))
			.await?
			.actor;
	assert_eq!(
		undone[0].actor.id, me.id,
		"actors.me names the person the app attributes to"
	);
	assert_ne!(me.label, "", "the person has a name");

	harness.shutdown().await?;
	Ok(())
}
