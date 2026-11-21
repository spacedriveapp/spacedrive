//! Acceptance for agent writes (`docs/core/design/agent-writes.md`): a
//! writable grant writes against a fresh revision or content hash, a stale
//! one is refused with the current state, a create never replaces anything,
//! and `files.undo` reverses each write from its journal.
//!
//! One `Core` over temporary directories with the watcher off, so the
//! store's revision moves only when the test moves it.

#![cfg(unix)]

mod helpers;

use std::{path::Path, time::Duration};

use helpers::*;
use sd_core::{
	infra::{action::LibraryAction, job::output::JobOutput},
	ops::{
		agent_write::{
			delete, make_dir, move_path, stat, write_file, ContentEncoding, DeleteInput,
			MakeDirInput, MoveInput, PathKind, PathState, Precondition, StatInput, WriteError,
			WriteFileInput, WriteOutcome,
		},
		files::undo::UndoJob,
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			GrantCap, GrantError,
		},
	},
};
use uuid::Uuid;

struct Agent<'a> {
	harness: &'a IndexingHarness,
	token: String,
}

impl Agent<'_> {
	async fn stat(&self, path: &str) -> PathState {
		stat(
			&self.harness.core.context,
			StatInput {
				token: self.token.clone(),
				path: path.into(),
			},
		)
		.await
		.expect("stat")
	}

	async fn write(
		&self,
		path: &str,
		content: &str,
		offset: Option<u64>,
		expect: Option<Precondition>,
	) -> Result<WriteOutcome, WriteError> {
		write_file(
			&self.harness.core.context,
			WriteFileInput {
				token: self.token.clone(),
				path: path.into(),
				content: content.to_string(),
				encoding: None,
				offset,
				expect,
			},
		)
		.await
	}

	async fn make_dir(&self, path: &str) -> Result<WriteOutcome, WriteError> {
		make_dir(
			&self.harness.core.context,
			MakeDirInput {
				token: self.token.clone(),
				path: path.into(),
			},
		)
		.await
	}

	async fn move_to(
		&self,
		from: &str,
		to: &str,
		expect: Precondition,
	) -> Result<WriteOutcome, WriteError> {
		move_path(
			&self.harness.core.context,
			MoveInput {
				token: self.token.clone(),
				from: from.into(),
				to: to.into(),
				expect,
			},
		)
		.await
	}

	async fn delete(&self, path: &str, expect: Precondition) -> Result<WriteOutcome, WriteError> {
		delete(
			&self.harness.core.context,
			DeleteInput {
				token: self.token.clone(),
				path: path.into(),
				expect,
			},
		)
		.await
	}
}

fn hash(state: &PathState) -> Precondition {
	Precondition {
		revision: None,
		content_hash: state.content_hash.clone(),
	}
}

fn read(path: &Path) -> String {
	std::fs::read_to_string(path).expect("readable")
}

async fn grant(harness: &IndexingHarness, path: &Path, caps: GrantCap) -> anyhow::Result<String> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("writes".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.token)
}

async fn undo(harness: &IndexingHarness, id: Uuid) -> serde_json::Value {
	let handle = harness
		.library
		.jobs()
		.dispatch(UndoJob::new(id, None))
		.await
		.expect("dispatched");
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("not an undo output: {output:?}");
	};
	value
}

fn stale(result: &Result<WriteOutcome, WriteError>) -> &PathState {
	match result {
		Err(error @ WriteError::Stale { .. }) => error.current().expect("current"),
		other => panic!("expected a stale refusal, got {other:?}"),
	}
}

#[tokio::test]
async fn agent_writes_are_fresh_keep_both_and_undoable() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_writes_acceptance")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/notes.md", "first").await?;
	tracked.write_file("project/AGENTS.md", "rules").await?;
	tracked.write_file("outside.txt", "outside").await?;
	tracked
		.write_file("project/vendor/lib/AGENTS.md", "vendored rules")
		.await?;
	tracked
		.write_file("project/repo/.git/config", "[core]")
		.await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");
	let agent = Agent {
		token: grant(&harness, &project, GrantCap::ReadWrite).await?,
		harness: &harness,
	};

	// Stat reports what a precondition names.
	let notes = agent.stat("notes.md").await;
	assert_eq!(notes.kind, Some(PathKind::File));
	assert_eq!(
		notes.content_hash.as_deref(),
		Some(blake3::hash(b"first").to_hex().as_str())
	);
	assert!(notes.revision.is_some());
	assert_eq!(agent.stat("missing.md").await.kind, None);

	// A write against the hash it read lands, and its result is the new state.
	let written = agent
		.write("notes.md", "second", None, Some(hash(&notes)))
		.await?;
	assert_eq!(read(&project.join("notes.md")), "second");
	assert!(!written.conflict);
	assert_eq!(
		written.state.content_hash.as_deref(),
		Some(blake3::hash(b"second").to_hex().as_str())
	);
	let overwrite = written.undo.expect("journaled");

	// The hash read before that write is stale now. The refusal carries the
	// current hash and revision, and the file is untouched.
	let refused = agent
		.write("notes.md", "lost", None, Some(hash(&notes)))
		.await;
	let current = stale(&refused);
	assert_eq!(current.content_hash, written.state.content_hash);
	assert!(current.revision.is_some());
	assert!(refused.unwrap_err().to_string().contains("stale"));
	assert_eq!(read(&project.join("notes.md")), "second");

	// The store revision works the same way: the current one lands, any other
	// is stale. Both fields named must hold. The revision counts every change
	// the store ingests, so wait for the tracking walk's follow-up passes.
	let revision = settled_revision(&agent).await;
	let by_revision = Precondition {
		revision: Some(revision.clone()),
		content_hash: None,
	};
	let third = agent
		.write("notes.md", "third", None, Some(by_revision.clone()))
		.await;
	assert!(third.is_ok(), "{third:?} read at {revision}");
	let old_revision = Precondition {
		revision: Some(format!("{revision}0")),
		content_hash: None,
	};
	stale(
		&agent
			.write("notes.md", "lost", None, Some(old_revision.clone()))
			.await,
	);
	let mixed = Precondition {
		revision: old_revision.revision.clone(),
		content_hash: agent.stat("notes.md").await.content_hash,
	};
	stale(&agent.write("notes.md", "lost", None, Some(mixed)).await);
	assert_eq!(read(&project.join("notes.md")), "third");

	// Changing an existing path needs a precondition.
	assert!(matches!(
		agent
			.write("notes.md", "lost", None, Some(Precondition::default()))
			.await,
		Err(WriteError::PreconditionRequired(_))
	));

	// A byte range lands over the bytes it names and keeps the rest.
	let now = agent.stat("notes.md").await;
	agent
		.write("notes.md", "IR", Some(1), Some(hash(&now)))
		.await?;
	assert_eq!(read(&project.join("notes.md")), "tIRrd");
	let now = agent.stat("notes.md").await;
	agent
		.write("notes.md", "!", Some(5), Some(hash(&now)))
		.await?;
	assert_eq!(read(&project.join("notes.md")), "tIRrd!");
	assert!(matches!(
		agent
			.write(
				"notes.md",
				"x",
				Some(99),
				Some(hash(&agent.stat("notes.md").await))
			)
			.await,
		Err(WriteError::Invalid(_))
	));
	assert!(matches!(
		agent.write("notes.md", "x", Some(0), None).await,
		Err(WriteError::Invalid(_))
	));

	// A create never replaces: a taken name keeps both files.
	let created = agent.write("drafts/plan.md", "plan", None, None).await?;
	assert!(!created.conflict);
	assert_eq!(created.state.path, "drafts/plan.md");
	let collided = agent
		.write("drafts/plan.md", "other plan", None, None)
		.await?;
	assert!(collided.conflict);
	assert_eq!(collided.requested, "drafts/plan.md");
	assert_eq!(collided.state.path, "drafts/plan (agent conflict 1).md");
	assert_eq!(read(&project.join("drafts/plan.md")), "plan");
	assert_eq!(
		read(&project.join("drafts/plan (agent conflict 1).md")),
		"other plan"
	);
	let again = agent
		.write("drafts/plan.md", "third plan", None, None)
		.await?;
	assert_eq!(again.state.path, "drafts/plan (agent conflict 2).md");

	// Binary content arrives as base64.
	let binary = write_file(
		&harness.core.context,
		WriteFileInput {
			token: agent.token.clone(),
			path: "bytes.bin".into(),
			content: "AAEC/w==".to_string(),
			encoding: Some(ContentEncoding::Base64),
			offset: None,
			expect: None,
		},
	)
	.await?;
	assert_eq!(
		std::fs::read(project.join("bytes.bin"))?,
		vec![0, 1, 2, 255]
	);
	assert_eq!(binary.state.size, Some(4));

	// Directories: made with their parents, unchanged when already there, and
	// a file in the way is kept.
	let made = agent.make_dir("a/b/c").await?;
	assert!(project.join("a/b/c").is_dir());
	assert!(made.undo.is_some());
	let unchanged = agent.make_dir("a/b/c").await?;
	assert!(unchanged.undo.is_none());
	assert!(!unchanged.conflict);
	let beside = agent.make_dir("bytes.bin").await?;
	assert!(beside.conflict);
	assert!(project.join("bytes.bin (agent conflict 1)").is_dir());

	// Moves check what was read at the source and keep a taken destination.
	let plan = agent.stat("drafts/plan.md").await;
	stale(
		&agent
			.move_to("drafts/plan.md", "a/plan.md", hash(&notes))
			.await,
	);
	let moved = agent
		.move_to("drafts/plan.md", "a/plan.md", hash(&plan))
		.await?;
	assert!(!moved.conflict);
	assert_eq!(read(&project.join("a/plan.md")), "plan");
	assert!(!project.join("drafts/plan.md").exists());
	let other = agent.stat("drafts/plan (agent conflict 1).md").await;
	let kept = agent
		.move_to(
			"drafts/plan (agent conflict 1).md",
			"a/plan.md",
			hash(&other),
		)
		.await?;
	assert!(kept.conflict);
	assert_eq!(kept.state.path, "a/plan (agent conflict 1).md");
	assert_eq!(read(&project.join("a/plan.md")), "plan");
	// A directory's precondition is the hash of its listing.
	let dir = agent.stat("a/b").await;
	agent.move_to("a/b", "moved-b", hash(&dir)).await?;
	assert!(project.join("moved-b/c").is_dir());
	assert!(matches!(
		agent
			.move_to(
				"moved-b",
				"moved-b/c/inner",
				hash(&agent.stat("moved-b").await)
			)
			.await,
		Err(WriteError::Invalid(_))
	));

	// Deletes go to the trash, against a fresh precondition only.
	stale(&agent.delete("a/plan.md", hash(&notes)).await);
	assert!(project.join("a/plan.md").exists());
	let deleted = agent
		.delete("a/plan.md", hash(&agent.stat("a/plan.md").await))
		.await?;
	assert!(!project.join("a/plan.md").exists());
	assert_eq!(deleted.state.kind, None);

	// Undo reverses each write from its journal.
	undo(&harness, deleted.undo.expect("journaled")).await;
	assert_eq!(read(&project.join("a/plan.md")), "plan");
	undo(&harness, moved.undo.expect("journaled")).await;
	assert_eq!(read(&project.join("drafts/plan.md")), "plan");
	assert!(!project.join("a/plan.md").exists());
	undo(&harness, again.undo.expect("journaled")).await;
	assert!(!project.join("drafts/plan (agent conflict 2).md").exists());
	assert_eq!(read(&project.join("drafts/plan.md")), "plan");
	let solo = agent.make_dir("solo/inner").await?;
	undo(&harness, solo.undo.expect("journaled")).await;
	assert!(!project.join("solo").exists());

	// An overwrite that later writes built on is left alone, and reverses
	// once the file is back to what that write left.
	let result = undo(&harness, overwrite).await;
	assert_eq!(result["reversed"], 0, "{result}");
	assert_eq!(read(&project.join("notes.md")), "tIRrd!");
	let fresh = agent.write("fresh.md", "v1", None, None).await?;
	let v1 = agent.stat("fresh.md").await;
	let v2 = agent.write("fresh.md", "v2", None, Some(hash(&v1))).await?;
	assert_eq!(read(&project.join("fresh.md")), "v2");
	undo(&harness, v2.undo.expect("journaled")).await;
	assert_eq!(read(&project.join("fresh.md")), "v1");
	undo(&harness, fresh.undo.expect("journaled")).await;
	assert!(!project.join("fresh.md").exists());

	// The grant still decides: instruction files, git internals, escapes and
	// read-only grants are refused before anything is touched.
	let agents_md = agent.stat("AGENTS.md").await;
	for result in [
		agent
			.write("AGENTS.md", "new rules", None, Some(hash(&agents_md)))
			.await,
		agent.write("docs/CLAUDE.md", "rules", None, None).await,
		agent.write(".git/config", "[core]", None, None).await,
		agent
			.move_to("notes.md", "AGENTS.md", hash(&agent.stat("notes.md").await))
			.await,
		agent.delete("AGENTS.md", hash(&agents_md)).await,
		agent.make_dir(".git").await,
	] {
		assert!(
			matches!(result, Err(WriteError::Grant(GrantError::Protected(_)))),
			"{result:?}"
		);
	}
	assert_eq!(read(&project.join("AGENTS.md")), "rules");
	assert!(!project.join(".git").exists());

	// Authorizing a directory does not authorize what is in it: a directory
	// holding a protected path is neither trashed nor moved.
	for result in [
		agent
			.delete("vendor", hash(&agent.stat("vendor").await))
			.await,
		agent
			.move_to("repo", "elsewhere", hash(&agent.stat("repo").await))
			.await,
	] {
		assert!(
			matches!(result, Err(WriteError::Grant(GrantError::Protected(_)))),
			"{result:?}"
		);
	}
	assert_eq!(
		read(&project.join("vendor/lib/AGENTS.md")),
		"vendored rules"
	);
	assert_eq!(read(&project.join("repo/.git/config")), "[core]");

	// A read-only file is replaced and keeps its mode.
	use std::os::unix::fs::PermissionsExt;
	agent.write("locked.txt", "v1", None, None).await?;
	std::fs::set_permissions(
		project.join("locked.txt"),
		std::fs::Permissions::from_mode(0o444),
	)?;
	let locked = agent.stat("locked.txt").await;
	agent
		.write("locked.txt", "v2", None, Some(hash(&locked)))
		.await?;
	assert_eq!(read(&project.join("locked.txt")), "v2");
	assert_eq!(
		std::fs::metadata(project.join("locked.txt"))?
			.permissions()
			.mode() & 0o777,
		0o444
	);
	assert!(matches!(
		agent.write("../outside.txt", "x", None, None).await,
		Err(WriteError::Grant(GrantError::Escape(_)))
	));
	assert_eq!(read(&project.join("../outside.txt")), "outside");
	assert!(matches!(
		agent.write("", "x", None, None).await,
		Err(WriteError::Invalid(_))
	));
	let reader = Agent {
		token: grant(&harness, &project, GrantCap::Read).await?,
		harness: &harness,
	};
	assert!(matches!(
		reader.write("new.md", "x", None, None).await,
		Err(WriteError::Grant(GrantError::ReadOnly))
	));
	assert!(!project.join("new.md").exists());

	// No staging file is left behind anywhere in the grant.
	let leftovers: Vec<_> = walk(&project)
		.into_iter()
		.filter(|path| path.to_string_lossy().ends_with(".sdpart"))
		.collect();
	assert!(leftovers.is_empty(), "{leftovers:?}");

	harness.shutdown().await?;
	Ok(())
}

async fn settled_revision(agent: &Agent<'_>) -> String {
	let mut last = agent.stat("notes.md").await.revision;
	for _ in 0..50 {
		tokio::time::sleep(Duration::from_millis(300)).await;
		let now = agent.stat("notes.md").await.revision;
		if now == last {
			break;
		}
		last = now;
	}
	last.expect("the tracked source has a store")
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
	let mut out = Vec::new();
	for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
		let path = entry.path();
		if path.is_dir() {
			out.extend(walk(&path));
		}
		out.push(path);
	}
	out
}
