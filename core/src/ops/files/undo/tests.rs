//! Undo over the journals the other jobs write: a rename goes back, a copy
//! is trashed, a trashed file returns, a replacement gives way to the
//! previous bytes, and a file that changed since is left alone.

use std::{path::Path, time::Duration};

use sd_store::file::FileKind;

use super::{
	input::FileUndoInput,
	job::UndoJob,
	preflight::{CHANGED, NO_JOURNAL},
	FileUndoAction,
};
use crate::{
	domain::{SdPath, SdPathBatch},
	infra::{
		action::preflight::{PreviewableAction, ValidatedAction},
		job::{
			journal::Effect,
			output::JobOutput,
			traits::{Job, JobHandler},
			types::JobId,
		},
	},
	ops::files::{
		copy::{
			action::FileConflictResolution,
			job::{CopyOptions, FileCopyJob},
		},
		delete::{DeleteJob, DeleteMode, DeleteTargets},
		fixture::{Fixture, T},
		merge::{FileMergeInput, FolderMergeJob, MergeConflictPolicy},
		plan::ChangeKind,
		rename::RenameJob,
	},
};

async fn run<J>(fixture: &Fixture, job: J) -> (JobId, JobOutput)
where
	J: Job + JobHandler + crate::infra::job::types::ErasedJob + crate::infra::job::traits::DynJob,
{
	let handle = fixture
		.library
		.jobs()
		.dispatch(job)
		.await
		.expect("dispatched");
	let id = handle.id();
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	(id, output)
}

async fn journal(fixture: &Fixture, job: JobId) -> Vec<Effect> {
	fixture
		.library
		.jobs()
		.database()
		.journal(job)
		.await
		.expect("journal")
		.into_iter()
		.map(|recorded| recorded.effect)
		.collect()
}

async fn undo(fixture: &Fixture, job: JobId) -> serde_json::Value {
	let input = FileUndoInput {
		job: job.0,
		effects: None,
	};
	let validation = FileUndoAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let (_, output) = run(fixture, UndoJob::new(job.0, None)).await;
	let JobOutput::Custom(value) = output else {
		panic!("not an undo output: {output:?}");
	};
	value
}

fn bytes(path: &Path) -> String {
	std::fs::read_to_string(path).expect("readable")
}

/// A rename journals a move, and undoing it moves the file back.
#[tokio::test]
async fn undoing_a_rename_moves_the_file_back() {
	let fixture = Fixture::new().await;
	let tree = [("a.txt", FileKind::File, 4, T, Some("a"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let a = fixture.source.join("a.txt");

	let (job, _) = run(
		&fixture,
		RenameJob::named(SdPath::local(&a), "b.txt".to_string()),
	)
	.await;
	assert!(matches!(
		journal(&fixture, job).await.as_slice(),
		[Effect::Moved { .. }]
	));

	let plan = FileUndoAction::preview(
		FileUndoInput {
			job: job.0,
			effects: None,
		},
		&fixture.preview(),
	)
	.await
	.expect("planned");
	assert_eq!(plan.summary.moves.files, 1);
	assert!(matches!(plan.changes[0].change, ChangeKind::Move { .. }));

	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 1);
	assert!(a.exists());
	assert!(!fixture.source.join("b.txt").exists());

	// Undoing the undo renames it again.
	let undo_job = fixture
		.library
		.jobs()
		.list_jobs(None)
		.await
		.expect("jobs")
		.into_iter()
		.find(|info| info.name == "undo")
		.expect("the undo job");
	undo(&fixture, JobId(undo_job.id)).await;
	assert!(fixture.source.join("b.txt").exists());
}

/// A copy journals a creation, which undo trashes, and a file that
/// changed since the copy is left alone with a warning.
#[tokio::test]
async fn undoing_a_copy_trashes_the_copy_unless_it_changed() {
	let fixture = Fixture::new().await;
	let tree = [
		("a.txt", FileKind::File, 4, T, Some("a"), None, None),
		("c.txt", FileKind::File, 2, T, Some("c"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	fixture.index(&fixture.destination, &[]).await;

	let (job, _) = run(
		&fixture,
		FileCopyJob::new(
			SdPathBatch::new(vec![
				SdPath::local(fixture.source.join("a.txt")),
				SdPath::local(fixture.source.join("c.txt")),
			]),
			SdPath::local(&fixture.destination),
		),
	)
	.await;
	let effects = journal(&fixture, job).await;
	assert_eq!(effects.len(), 2, "{effects:?}");
	assert!(effects
		.iter()
		.all(|effect| matches!(effect, Effect::Created { .. })));

	// One copy changes afterward.
	std::fs::write(fixture.destination.join("c.txt"), b"changed bytes").expect("write");
	let input = FileUndoInput {
		job: job.0,
		effects: None,
	};
	let validation = FileUndoAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(validation
		.findings
		.iter()
		.any(|finding| finding.code == CHANGED));
	assert_eq!(validation.facts.estimated_files, Some(1));

	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 1);
	assert_eq!(output["left"].as_array().map(Vec::len), Some(1));
	assert!(!fixture.destination.join("a.txt").exists());
	assert_eq!(
		bytes(&fixture.destination.join("c.txt")),
		"changed bytes",
		"the changed copy stays"
	);
	assert!(fixture.source.join("a.txt").exists(), "the original stays");
}

/// A trashed file journals where it went, and undo puts it back.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn undoing_a_delete_restores_from_the_trash() {
	let fixture = Fixture::new().await;
	let tree = [("gone.txt", FileKind::File, 4, T, Some("g"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let gone = fixture.source.join("gone.txt");

	let (job, _) = run(
		&fixture,
		DeleteJob::new(
			DeleteTargets::Paths {
				paths: vec![SdPath::local(&gone)],
			},
			DeleteMode::Trash,
		),
	)
	.await;
	let effects = journal(&fixture, job).await;
	let [Effect::Trashed {
		to: Some(location), ..
	}] = effects.as_slice()
	else {
		panic!("not a trashing with a location: {effects:?}");
	};
	assert!(location.exists());
	assert!(!gone.exists());

	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 1);
	assert_eq!(bytes(&gone), "xxxx");
}

/// A permanent delete cannot be undone, and validation says so.
#[tokio::test]
async fn a_permanent_delete_has_nothing_to_undo() {
	let fixture = Fixture::new().await;
	let tree = [("gone.txt", FileKind::File, 4, T, Some("g"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;

	let (job, _) = run(
		&fixture,
		DeleteJob::new(
			DeleteTargets::Paths {
				paths: vec![SdPath::local(fixture.source.join("gone.txt"))],
			},
			DeleteMode::Permanent,
		),
	)
	.await;
	assert!(matches!(
		journal(&fixture, job).await.as_slice(),
		[Effect::Removed { .. }]
	));
	let validation = FileUndoAction::validate(
		&FileUndoInput {
			job: job.0,
			effects: None,
		},
		&fixture.preview(),
	)
	.await
	.expect("validated");
	assert!(validation.refuses());

	let validation = FileUndoAction::validate(
		&FileUndoInput {
			job: uuid::Uuid::new_v4(),
			effects: None,
		},
		&fixture.preview(),
	)
	.await
	.expect("validated");
	assert!(validation
		.errors()
		.any(|finding| finding.code == NO_JOURNAL));
}

/// A merge that overwrites keeps the previous bytes in the trash, and undo
/// puts them back byte for byte; the copied files are trashed.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn undoing_a_merge_restores_replaced_files() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let incoming = [
		("same.txt", file, 4, T, Some("s"), None, None),
		("new.txt", file, 3, T, Some("n"), None, None),
	];
	fixture.materialize(&fixture.source, &incoming);
	fixture.index(&fixture.source, &incoming).await;
	std::fs::write(fixture.destination.join("same.txt"), b"old bytes").expect("file");
	fixture
		.index(
			&fixture.destination,
			&[("same.txt", file, 9, T - 1000, Some("o"), None, None)],
		)
		.await;

	let (job, _) = run(
		&fixture,
		FolderMergeJob::new(FileMergeInput {
			sources: SdPathBatch::new(vec![SdPath::local(&fixture.source)]),
			destination: SdPath::local(&fixture.destination),
			on_conflict: MergeConflictPolicy::Overwrite,
			consume_sources: false,
			remove_extras: false,
		}),
	)
	.await;
	assert_eq!(bytes(&fixture.destination.join("same.txt")), "xxxx");
	let effects = journal(&fixture, job).await;
	assert!(effects.iter().any(|effect| matches!(
		effect,
		Effect::Replaced {
			previous: Some(_),
			..
		}
	)));
	assert!(effects
		.iter()
		.any(|effect| matches!(effect, Effect::Created { .. })));

	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 2, "{output}");
	assert_eq!(bytes(&fixture.destination.join("same.txt")), "old bytes");
	assert!(!fixture.destination.join("new.txt").exists());
}

/// A copy that overwrites honors the policy: skipped without overwrite,
/// replaced with the previous bytes kept when overwriting.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_copy_skips_or_replaces_as_the_policy_says() {
	let fixture = Fixture::new().await;
	let tree = [("a.txt", FileKind::File, 4, T, Some("a"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	std::fs::write(fixture.destination.join("a.txt"), b"old").expect("file");
	fixture
		.index(
			&fixture.destination,
			&[("a.txt", FileKind::File, 3, T, Some("o"), None, None)],
		)
		.await;

	let sources = SdPathBatch::new(vec![SdPath::local(fixture.source.join("a.txt"))]);
	let (job, _) = run(
		&fixture,
		FileCopyJob::new(sources.clone(), SdPath::local(&fixture.destination)),
	)
	.await;
	assert_eq!(bytes(&fixture.destination.join("a.txt")), "old", "skipped");
	assert!(journal(&fixture, job).await.is_empty());

	let (job, _) = run(
		&fixture,
		FileCopyJob::new(sources, SdPath::local(&fixture.destination)).with_options(CopyOptions {
			overwrite: true,
			conflict_resolution: Some(FileConflictResolution::Overwrite),
			..Default::default()
		}),
	)
	.await;
	assert_eq!(
		bytes(&fixture.destination.join("a.txt")),
		"xxxx",
		"replaced"
	);
	let effects = journal(&fixture, job).await;
	assert!(
		matches!(
			effects.as_slice(),
			[Effect::Replaced {
				previous: Some(_),
				..
			}]
		),
		"{effects:?}"
	);
	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 1);
	assert_eq!(bytes(&fixture.destination.join("a.txt")), "old");
}
