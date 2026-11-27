//! Space versioning: a space with versioning on keeps the bytes of every
//! change, lists them per file, restores them through the write path, and
//! keeps what a snapshot pins past retention. A space with it off writes
//! nothing.
//!
//! `docs/core/design/spaces.md` and `docs/core/space-versioning.mdx`.

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
			versioning, ChangeOrder, SpaceChangesInput, SpaceChangesQuery, SpaceCreateAction,
			SpaceCreateInput, SpaceHistoryInput, SpaceHistoryOutput, SpaceHistoryPurgeAction,
			SpaceHistoryPurgeInput, SpaceHistoryQuery, SpaceRestoreAction, SpaceRestoreInput,
			SpaceSnapshotCreateAction, SpaceSnapshotCreateInput, SpaceSnapshotDeleteAction,
			SpaceSnapshotDeleteInput, SpaceSnapshotListInput, SpaceSnapshotListQuery,
			SpaceUpdateAction, SpaceUpdateInput,
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

async fn set_versioning(
	harness: &IndexingHarness,
	space: &Space,
	on: bool,
) -> anyhow::Result<Space> {
	Ok(SpaceUpdateAction::from_input(SpaceUpdateInput {
		id: space.id,
		name: None,
		content_indexing: None,
		versioning: Some(on),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space)
}

async fn history(
	harness: &IndexingHarness,
	space: &Space,
	path: Option<&str>,
) -> SpaceHistoryOutput {
	SpaceHistoryQuery::from_input(SpaceHistoryInput {
		id: space.id,
		path: path.map(str::to_string),
		before: None,
		limit: None,
	})
	.expect("input")
	.execute(harness.core.context.clone(), session(harness))
	.await
	.expect("history")
}

/// Poll the history until `done` holds, for captures that run in the
/// background.
async fn history_until(
	harness: &IndexingHarness,
	space: &Space,
	path: Option<&str>,
	done: impl Fn(&SpaceHistoryOutput) -> bool,
) -> SpaceHistoryOutput {
	let mut seen = history(harness, space, path).await;
	for _ in 0..100 {
		if done(&seen) {
			return seen;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
		seen = history(harness, space, path).await;
	}
	panic!("the history never got there: {seen:?}");
}

async fn grant(harness: &IndexingHarness, path: &Path) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(GrantCap::ReadWrite),
		ttl_seconds: None,
		label: Some("builder".to_string()),
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
	expect: Option<String>,
) -> anyhow::Result<Option<String>> {
	let outcome = write_file(
		&harness.core.context,
		WriteFileInput {
			token: token.to_string(),
			path: path.into(),
			content: content.to_string(),
			encoding: None,
			offset: None,
			expect: expect.map(|hash| Precondition {
				revision: None,
				content_hash: Some(hash),
			}),
		},
	)
	.await
	.map_err(|e| anyhow::anyhow!("{e:?}"))?;
	Ok(outcome.state.content_hash)
}

async fn restore(
	harness: &IndexingHarness,
	space: &Space,
	version: Option<i64>,
	snapshot: Option<&str>,
	path: Option<&str>,
) -> anyhow::Result<sd_core::ops::spaces::SpaceRestoreOutput> {
	Ok(SpaceRestoreAction::from_input(SpaceRestoreInput {
		id: space.id,
		version,
		snapshot: snapshot.map(str::to_string),
		path: path.map(str::to_string),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

fn space_dir(harness: &IndexingHarness, space: &Space) -> std::path::PathBuf {
	versioning::space_dir(&harness.core.context, space.source_id, space.id).expect("store dir")
}

#[tokio::test]
async fn a_space_with_versioning_off_stores_nothing() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("versioning_off")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/README.md", "hello").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	assert!(!project.versioning, "off by default");

	let token = grant(&harness, &tracked.root.join("project")).await?.token;
	let first = write(&harness, &token, "notes.md", "one", None).await?;
	write(&harness, &token, "notes.md", "two", first).await?;

	let seen = history(&harness, &project, None).await;
	assert!(!seen.versioning);
	assert!(seen.versions.is_empty());
	let dir = space_dir(&harness, &project);
	assert!(
		!dir.join(sd_store::SPACE_VERSIONS_FILE).exists()
			&& !dir.join(sd_store::BLOBS_DIR).exists(),
		"no versions or blobs for a space with versioning off"
	);
	assert!(
		restore(&harness, &project, Some(1), None, None)
			.await
			.is_err(),
		"nothing to restore"
	);
	Ok(())
}

#[tokio::test]
async fn edits_keep_versions_that_restore_through_the_write_path() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("versioning_edits")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/README.md", "hello").await?;
	home.write_file("other/README.md", "elsewhere").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	let project = set_versioning(&harness, &project, true).await?;
	assert!(project.versioning);

	let baseline = history_until(&harness, &project, Some("README.md"), |h| {
		!h.versions.is_empty()
	})
	.await;
	assert_eq!(baseline.versions[0].origin, versioning::BASELINE);
	assert_eq!(baseline.versions[0].actor.kind, ActorKind::Person);

	let token = grant(&harness, &tracked.root.join("project")).await?.token;
	let first = write(&harness, &token, "notes.md", "first draft", None).await?;
	let second = write(&harness, &token, "notes.md", "second draft", first.clone()).await?;
	let outside = grant(&harness, &tracked.root.join("other")).await?.token;
	write(&harness, &outside, "notes.md", "not versioned", None).await?;

	let notes = history(&harness, &project, Some("notes.md")).await;
	assert_eq!(notes.versions.len(), 2, "{notes:?}");
	assert_eq!(notes.versions[0].hash, second);
	assert_eq!(notes.versions[1].hash, first);
	assert_eq!(notes.versions[0].origin, "agent_write");
	assert_eq!(notes.versions[0].actor.label, "builder");
	assert!(
		notes.versions[0].change_seq.is_some(),
		"a version points at its change"
	);
	assert!(
		history(&harness, &project, None)
			.await
			.versions
			.iter()
			.all(|v| v.path != "other/notes.md"),
		"nothing outside the space"
	);

	let restored = restore(&harness, &project, Some(notes.versions[1].id), None, None).await?;
	assert_eq!(restored.restored.len(), 1);
	assert!(
		restored.restored[0].undo.is_some(),
		"a restore can be undone"
	);
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/notes.md")).await?,
		"first draft"
	);
	let feed = SpaceChangesQuery::from_input(SpaceChangesInput {
		id: project.id,
		order: Some(ChangeOrder::Newest),
		since: None,
		before: None,
		path: Some("notes.md".into()),
		limit: Some(1),
	})
	.expect("input")
	.execute(harness.core.context.clone(), session(&harness))
	.await?;
	let change = &feed.changes[0];
	assert_eq!(change.origin, "restore");
	assert_eq!(change.op, "modify");
	assert_eq!(change.actor.kind, ActorKind::Person);
	assert_eq!(change.before_hash, second);
	assert_eq!(change.after_hash, first);

	let after = history(&harness, &project, Some("notes.md")).await;
	assert_eq!(after.versions.len(), 3, "the restore is a version too");
	assert_eq!(after.versions[0].origin, "restore");
	assert_eq!(after.versions[0].hash, first);

	tokio::fs::remove_file(tracked.root.join("project/notes.md")).await?;
	restore(
		&harness,
		&project,
		Some(notes.versions[0].id),
		None,
		Some("restored/notes.md"),
	)
	.await?;
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/restored/notes.md")).await?,
		"second draft",
		"a version can be written to another path"
	);
	assert!(
		restore(
			&harness,
			&project,
			Some(notes.versions[0].id),
			None,
			Some("../escape.md")
		)
		.await
		.is_err(),
		"a restore stays inside the space"
	);
	assert!(!tracked.root.join("escape.md").exists());

	// Turning versioning off stops capture and keeps what was stored.
	let kept = history(&harness, &project, None).await.versions.len();
	let off = set_versioning(&harness, &project, false).await?;
	assert!(!off.versioning);
	write(&harness, &token, "after-off.md", "not captured", None).await?;
	let seen = history(&harness, &off, None).await;
	assert!(!seen.versioning);
	assert_eq!(seen.versions.len(), kept, "nothing captured while off");
	restore(&harness, &off, Some(notes.versions[0].id), None, None).await?;
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/notes.md")).await?,
		"second draft",
		"kept versions still restore"
	);

	// A file deleted while versioning was off is recorded gone once it is
	// turned back on, so a snapshot does not bring it back.
	tokio::fs::remove_file(tracked.root.join("project/notes.md")).await?;
	let on = set_versioning(&harness, &off, true).await?;
	history_until(&harness, &on, Some("notes.md"), |h| {
		h.versions.first().is_some_and(|v| v.deleted)
	})
	.await;
	let versions = versioning::open(&harness.core.context, on.source_id, on.id)
		.await
		.expect("versions");
	assert!(
		versions.manifest(None, Some("notes.md")).await?.is_empty(),
		"the baseline records the delete"
	);
	drop(versions);
	let off = set_versioning(&harness, &on, false).await?;

	// Purging is the explicit step that deletes them.
	let purged = SpaceHistoryPurgeAction::from_input(SpaceHistoryPurgeInput { id: off.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	assert!(purged.purged);
	assert!(history(&harness, &off, None).await.versions.is_empty());
	assert!(
		!space_dir(&harness, &off).join(sd_store::BLOBS_DIR).exists(),
		"a purge deletes the stored versions"
	);
	Ok(())
}

#[tokio::test]
async fn a_snapshot_keeps_its_versions_past_retention() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("versioning_snapshots")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/a.md", "a1").await?;
	home.write_file("project/b.md", "b1").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	let project = set_versioning(&harness, &project, true).await?;
	history_until(&harness, &project, None, |h| h.versions.len() == 2).await;

	let snapshot = SpaceSnapshotCreateAction::from_input(SpaceSnapshotCreateInput {
		id: project.id,
		name: "before the agent".into(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.snapshot;
	assert_eq!(snapshot.files, 2);
	assert_eq!(snapshot.created_by.kind, ActorKind::Person);

	let token = grant(&harness, &tracked.root.join("project")).await?.token;
	let a = history(&harness, &project, Some("a.md")).await.versions[0]
		.hash
		.clone();
	let a2 = write(&harness, &token, "a.md", "a2", a).await?;
	write(&harness, &token, "a.md", "a3", a2).await?;
	let b = history(&harness, &project, Some("b.md")).await.versions[0]
		.hash
		.clone();
	write(&harness, &token, "b.md", "b2", b).await?;

	// Age everything: a prune with a cutoff in the future drops every version
	// retention does not keep.
	let versions = versioning::open(&harness.core.context, project.source_id, project.id)
		.await
		.expect("versions");
	let future = (chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339();
	let pruned = versions.prune(&future).await?;
	assert_eq!(
		pruned.versions, 1,
		"only a2, neither pinned nor current: {pruned:?}"
	);
	let a_left = history(&harness, &project, Some("a.md")).await;
	assert_eq!(
		a_left.versions.len(),
		2,
		"the current version and the pinned one"
	);

	let restored = restore(&harness, &project, None, Some("before the agent"), None).await?;
	assert_eq!(restored.restored.len(), 2);
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/a.md")).await?,
		"a1"
	);
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/b.md")).await?,
		"b1"
	);
	let again = restore(
		&harness,
		&project,
		None,
		Some(&snapshot.id.to_string()),
		None,
	)
	.await?;
	assert!(again.restored.is_empty());
	assert_eq!(
		again.unchanged, 2,
		"a second restore finds nothing to change"
	);

	// An edit not captured yet is compared on disk, not by its last version.
	tokio::fs::write(tracked.root.join("project/a.md"), "outside").await?;
	let fresh = restore(&harness, &project, None, Some("before the agent"), None).await?;
	assert_eq!((fresh.restored.len(), fresh.unchanged), (1, 1));
	assert_eq!(
		tokio::fs::read_to_string(tracked.root.join("project/a.md")).await?,
		"a1"
	);

	let list = SpaceSnapshotListQuery::from_input(SpaceSnapshotListInput { id: project.id })
		.expect("input")
		.execute(harness.core.context.clone(), session(&harness))
		.await?;
	assert_eq!(list.snapshots.len(), 1);
	let deleted = SpaceSnapshotDeleteAction::from_input(SpaceSnapshotDeleteInput {
		id: project.id,
		snapshot: snapshot.id,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert!(deleted.deleted);
	let pruned = versions.prune(&future).await?;
	assert!(
		pruned.versions > 0 && pruned.chunks > 0,
		"unpinned, they age out: {pruned:?}"
	);
	Ok(())
}

#[tokio::test]
async fn only_a_grant_on_the_whole_space_makes_or_drops_snapshots() -> anyhow::Result<()> {
	use serde_json::{json, Value};

	let harness = IndexingHarnessBuilder::new("versioning_snapshot_scope")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/a.md", "a1").await?;
	home.write_file("project/sub/b.md", "b1").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	let project = set_versioning(&harness, &project, true).await?;
	history_until(&harness, &project, None, |h| h.versions.len() == 2).await;

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = sd_core::service::agent_mcp::serve(
		harness.core.context.clone(),
		listener,
		sd_core::service::agent_mcp::Limits::default(),
	)?;
	let url = format!("http://{bound}/mcp");
	let http = reqwest::Client::new();
	let call = |token: String, tool: &'static str, arguments: Value| {
		let (http, url) = (http.clone(), url.clone());
		async move {
			let message = |method: &str, params: Value| {
				json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string()
			};
			let post = |body: String, session: Option<String>| {
				let mut request = http
					.post(&url)
					.bearer_auth(&token)
					.header("accept", "application/json, text/event-stream")
					.header("content-type", "application/json")
					.body(body);
				if let Some(session) = session {
					request = request.header("mcp-session-id", session);
				}
				request.send()
			};
			let init = post(
				message(
					"initialize",
					json!({
						"protocolVersion": "2025-06-18",
						"capabilities": {},
						"clientInfo": { "name": "versioning", "version": "0" },
					}),
				),
				None,
			)
			.await
			.expect("initialize");
			let session = init
				.headers()
				.get("mcp-session-id")
				.map(|session| session.to_str().unwrap().to_string());
			let reply = post(
				message(
					"tools/call",
					json!({ "name": tool, "arguments": arguments }),
				),
				session,
			)
			.await
			.expect("call");
			let body: Value = serde_json::from_slice(&reply.bytes().await.expect("body")).unwrap();
			body["result"].clone()
		}
	};

	let whole = grant(&harness, &tracked.root.join("project")).await?.token;
	let part = grant(&harness, &tracked.root.join("project/sub"))
		.await?
		.token;
	let refused = call(
		part.clone(),
		"snapshot",
		json!({ "action": "create", "name": "mine" }),
	)
	.await;
	assert_eq!(refused["isError"], json!(true), "{refused}");
	let made = call(
		whole.clone(),
		"snapshot",
		json!({ "action": "create", "name": "kept" }),
	)
	.await;
	assert_eq!(made["isError"], json!(false), "{made}");
	let listed = call(part.clone(), "snapshot", json!({ "action": "list" })).await;
	assert_eq!(listed["isError"], json!(false), "{listed}");
	let dropped = call(
		part,
		"snapshot",
		json!({ "action": "delete", "name": "kept" }),
	)
	.await;
	assert_eq!(dropped["isError"], json!(true), "{dropped}");
	let list = SpaceSnapshotListQuery::from_input(SpaceSnapshotListInput { id: project.id })
		.expect("input")
		.execute(harness.core.context.clone(), session(&harness))
		.await?;
	assert_eq!(
		list.snapshots.len(),
		1,
		"the person's snapshot is still there"
	);
	let dropped = call(
		whole,
		"snapshot",
		json!({ "action": "delete", "name": "kept" }),
	)
	.await;
	assert_eq!(dropped["isError"], json!(false), "{dropped}");
	Ok(())
}

#[tokio::test]
async fn a_change_the_watcher_sees_is_captured_once_it_settles() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("versioning_watcher")
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	home.write_file("project/README.md", "hello").await?;
	let tracked = home.track().await?;
	let project = space(&harness, &tracked.root.join("project")).await?;
	let project = set_versioning(&harness, &project, true).await?;
	history_until(&harness, &project, Some("README.md"), |h| {
		!h.versions.is_empty()
	})
	.await;
	tokio::time::sleep(Duration::from_secs(1)).await;

	let file = tracked.root.join("project/README.md");
	for text in ["edit one", "edit two", "edit three"] {
		tokio::fs::write(&file, text).await?;
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	let final_hash = blake3::hash(b"edit three").to_hex().to_string();
	let seen = history_until(&harness, &project, Some("README.md"), |h| {
		h.versions.first().and_then(|v| v.hash.as_deref()) == Some(final_hash.as_str())
	})
	.await;
	let captured = &seen.versions[0];
	assert_eq!(captured.origin, "watcher");
	assert_eq!(captured.actor.kind, ActorKind::Device);
	assert_eq!(
		seen.versions.len(),
		2,
		"a burst of saves is one version once the file goes quiet: {seen:?}"
	);

	tokio::fs::write(tracked.root.join("project/new.md"), "made outside").await?;
	history_until(&harness, &project, Some("new.md"), |h| {
		!h.versions.is_empty()
	})
	.await;
	tokio::fs::remove_file(tracked.root.join("project/new.md")).await?;
	let gone = history_until(&harness, &project, Some("new.md"), |h| {
		h.versions.first().is_some_and(|v| v.deleted)
	})
	.await;
	assert_eq!(gone.versions.len(), 2);
	Ok(())
}

/// The cost of versioning on a 1 GiB folder: 10,000 agent edits of 4 KiB at
/// random offsets, into one space with versioning off and one with it on.
/// Prints write latency and the bytes the versions take. Run with
/// `cargo test -p sd-core --test space_versioning_test -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "writes 2 GiB and takes minutes"]
async fn bench_versioning_overhead_on_a_gigabyte_folder() -> anyhow::Result<()> {
	use base64::Engine;

	const SMALL: usize = 512;
	const LARGE: usize = 32;
	const EDITS: usize = 10_000;
	const EDIT_BYTES: usize = 4096;

	let harness = IndexingHarnessBuilder::new("versioning_bench")
		.disable_watcher()
		.build()
		.await?;
	let home = harness.create_test_dir("home").await?;
	// Every replaced file goes to the trash. Point the platform trash at a
	// folder of this run, so emptying it between writes touches nothing else.
	let trash_home = harness.create_test_dir("data").await?;
	std::env::set_var("XDG_DATA_HOME", trash_home.path());
	let mut state = 0x9e37_79b9_7f4a_7c15u64;
	let mut next = move || {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		state
	};
	let mut files = Vec::new();
	for (count, size) in [(SMALL, 1usize << 20), (LARGE, 16usize << 20)] {
		for i in 0..count {
			let name = format!("{size}/file-{i}.bin");
			files.push((name, size));
		}
	}
	for side in ["off", "on"] {
		for (name, size) in &files {
			let path = home.path().join(side).join(name);
			std::fs::create_dir_all(path.parent().unwrap())?;
			let bytes: Vec<u8> = (0..*size / 8).flat_map(|_| next().to_le_bytes()).collect();
			std::fs::write(path, bytes)?;
		}
	}
	let tracked = home.track().await?;
	let off = space(&harness, &tracked.root.join("off")).await?;
	let on = space(&harness, &tracked.root.join("on")).await?;

	let started = std::time::Instant::now();
	let on = set_versioning(&harness, &on, true).await?;
	let versions = versioning::open(&harness.core.context, on.source_id, on.id)
		.await
		.expect("versions");
	while versions.manifest(None, None).await?.len() < files.len() {
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
	let baseline_secs = started.elapsed().as_secs_f64();
	let dir_size = |dir: &Path| -> u64 {
		let mut total = 0;
		let mut pending = vec![dir.to_path_buf()];
		while let Some(dir) = pending.pop() {
			for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
				let meta = entry.metadata().unwrap();
				if meta.is_dir() {
					pending.push(entry.path());
				} else {
					total += meta.len();
				}
			}
		}
		total
	};
	let space_bytes = dir_size(&tracked.root.join("on"));
	let baseline_bytes = dir_size(&space_dir(&harness, &on));

	let picks: Vec<(usize, u64, Vec<u8>)> = (0..EDITS)
		.map(|_| {
			let file = next() as usize % files.len();
			let offset = next() % (files[file].1 - EDIT_BYTES) as u64;
			let bytes: Vec<u8> = (0..EDIT_BYTES / 8)
				.flat_map(|_| next().to_le_bytes())
				.collect();
			(file, offset, bytes)
		})
		.collect();
	let mut report = Vec::new();
	for space in [&off, &on] {
		let token = grant(&harness, space.path.as_ref().unwrap()).await?.token;
		let mut hashes = vec![None::<String>; files.len()];
		let mut latencies = Vec::with_capacity(EDITS);
		for (done, (file, offset, bytes)) in picks.iter().enumerate() {
			// Empty this run's trash between timed writes so 10,000 edits fit
			// on the disk.
			if done % 250 == 0 {
				let _ = std::fs::remove_dir_all(trash_home.path().join("Trash/files"));
				let _ = std::fs::remove_dir_all(trash_home.path().join("Trash/info"));
			}
			let (name, _) = &files[*file];
			let expect = match &hashes[*file] {
				Some(hash) => hash.clone(),
				None => sd_core::ops::agent_write::stat(
					&harness.core.context,
					sd_core::ops::agent_write::StatInput {
						token: token.clone(),
						path: name.into(),
					},
				)
				.await
				.map_err(|e| anyhow::anyhow!("{e:?}"))?
				.content_hash
				.expect("hash"),
			};
			let began = std::time::Instant::now();
			let outcome = write_file(
				&harness.core.context,
				WriteFileInput {
					token: token.clone(),
					path: name.into(),
					content: base64::engine::general_purpose::STANDARD.encode(bytes),
					encoding: Some(sd_core::ops::agent_write::ContentEncoding::Base64),
					offset: Some(*offset),
					expect: Some(Precondition {
						revision: None,
						content_hash: Some(expect),
					}),
				},
			)
			.await
			.map_err(|e| anyhow::anyhow!("{e:?}"))?;
			latencies.push(began.elapsed().as_secs_f64() * 1000.0);
			hashes[*file] = outcome.state.content_hash;
		}
		latencies.sort_by(f64::total_cmp);
		let at = |q: f64| latencies[((latencies.len() - 1) as f64 * q) as usize];
		let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
		report.push((space.versioning, mean, at(0.5), at(0.95), at(0.99)));
	}
	let after_bytes = dir_size(&space_dir(&harness, &on));
	let rows = versions.history(None, None, u32::MAX).await?.len();

	let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
	tracing::info!("bench done");
	eprintln!(
		"space: {:.0} MiB in {} files",
		mib(space_bytes),
		files.len()
	);
	eprintln!(
		"baseline capture: {:.1} s, {:.0} MiB stored ({:.2}x the space)",
		baseline_secs,
		mib(baseline_bytes),
		baseline_bytes as f64 / space_bytes as f64
	);
	eprintln!(
		"after {EDITS} edits of {EDIT_BYTES} B: {:.0} MiB stored, {:.0} MiB for the edits ({:.1} KiB per edit), {rows} version rows",
		mib(after_bytes),
		mib(after_bytes - baseline_bytes),
		(after_bytes - baseline_bytes) as f64 / 1024.0 / EDITS as f64
	);
	for (versioning, mean, p50, p95, p99) in report {
		eprintln!(
			"versioning {}: write mean {mean:.1} ms, p50 {p50:.1} ms, p95 {p95:.1} ms, p99 {p99:.1} ms",
			if versioning { "on " } else { "off" }
		);
	}
	Ok(())
}
