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
