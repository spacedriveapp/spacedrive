//! Validate, preview and the job over a small tree with every kind of leaf.

use std::{path::Path, time::Duration};

use sd_store::file::FileKind;

use super::{
	input::{FileMergeInput, MergeConflictPolicy},
	job::{Cursor, FolderMergeJob, MergeOutput, MergeResult},
	validate::{self, DESTINATION_MISSING, DESTINATION_NOT_DIRECTORY, NESTED, SOURCE_DETACHED},
};
use crate::{
	domain::{SdPath, SdPathBatch},
	infra::{
		action::preflight::{PreviewContext, PreviewableAction, Severity},
		job::output::JobOutput,
	},
	ops::files::{
		fixture::{Fixture, T},
		plan::{ChangeKind, ConflictKind, FsPlan, PlanBasis, ReplaceReason, SkipReason, Tally},
		planner::Planner,
	},
};

/// The plan of `input`, as the merge preview builds it.
async fn plan_of(preview: &PreviewContext, input: &FileMergeInput) -> FsPlan {
	let mut planner = Planner::new(preview, input.on_conflict, false);
	for source in &input.sources.paths {
		planner
			.pair(source, &input.destination, input.consume_sources)
			.await
			.expect("planned");
	}
	planner.finish()
}

impl Fixture {
	fn input(&self, policy: MergeConflictPolicy) -> FileMergeInput {
		FileMergeInput {
			sources: SdPathBatch {
				paths: vec![SdPath::local(&self.source)],
			},
			destination: SdPath::local(&self.destination),
			on_conflict: policy,
			consume_sources: false,
			remove_extras: false,
		}
	}

	/// The source and destination trees the plan tests read: one leaf of
	/// every kind the planner sorts.
	async fn index_tree(&self) {
		let file = FileKind::File;
		self.index(
			&self.source,
			&[
				("same.txt", file, 4, T, Some("s1"), None, None),
				("verified.txt", file, 9, T, Some("v"), Some("V"), None),
				("diff.txt", file, 3, T + 10, Some("d1"), None, None),
				("new.txt", file, 7, T, Some("n"), None, None),
				("sub", FileKind::Directory, 0, T, None, None, None),
				("sub/inner.txt", file, 2, T, Some("i"), None, None),
				("sub/.DS_Store", file, 6, T, Some("ds"), None, None),
				("deeper/f.txt", file, 5, T, Some("f"), None, None),
				("clash", file, 1, T, Some("c"), None, None),
				("link", FileKind::Symlink, 0, T, None, None, Some("a")),
			],
		)
		.await;
		self.index(
			&self.destination,
			&[
				("same.txt", file, 4, T, Some("s1"), None, None),
				("verified.txt", file, 9, T, Some("v"), Some("V"), None),
				("diff.txt", file, 5, T, Some("d2"), None, None),
				("keep.txt", file, 8, T, Some("k"), None, None),
				("sub", FileKind::Directory, 0, T, None, None, None),
				("clash", FileKind::Directory, 0, T, None, None, None),
				("link", file, 1, T, Some("l"), None, None),
			],
		)
		.await;
	}

	async fn plan(&self, policy: MergeConflictPolicy) -> FsPlan {
		plan_of(&self.preview(), &self.input(policy)).await
	}
}

fn change_at<'a>(plan: &'a FsPlan, relative: &str, destination: &Path) -> &'a ChangeKind {
	let path = destination.join(relative);
	&plan
		.changes
		.iter()
		.find(|change| change.path.path() == Some(&path))
		.unwrap_or_else(|| panic!("no change at {relative}"))
		.change
}

/// The plan sorts every kind of leaf: creates, a merged directory, both
/// tiers of duplicate, junk, a collision under the policy, and the two
/// conflicts nothing resolves, with complete counts and the stores it read.
#[tokio::test]
async fn a_plan_sorts_every_kind_of_leaf() {
	let fixture = Fixture::new().await;
	fixture.index_tree().await;
	let plan = fixture.plan(MergeConflictPolicy::Skip).await;
	let dst = &fixture.destination;

	let summary = &plan.summary;
	assert_eq!(
		summary.creates,
		Tally {
			files: 3,
			bytes: 14
		}
	);
	assert_eq!(summary.directories_created, 1);
	assert_eq!(summary.merged_into, 1);
	assert_eq!(
		summary.skips.duplicate_candidates,
		Tally { files: 1, bytes: 4 }
	);
	assert_eq!(
		summary.skips.duplicates_confirmed,
		Tally { files: 1, bytes: 9 }
	);
	assert_eq!(summary.skips.junk, 1);
	assert_eq!(summary.skips.policy, Tally { files: 1, bytes: 3 });
	assert_eq!(summary.collisions, 1);
	assert_eq!(summary.conflicts, 2);
	assert_eq!(summary.replaces, Tally::default());
	assert_eq!(plan.changes.len(), 11);
	assert!(!plan.truncated);

	assert_eq!(
		change_at(&plan, "new.txt", dst),
		&ChangeKind::Create { size: 7 }
	);
	assert_eq!(
		change_at(&plan, "deeper", dst),
		&ChangeKind::CreateDirectory
	);
	assert_eq!(change_at(&plan, "sub", dst), &ChangeKind::MergeInto);
	assert_eq!(
		change_at(&plan, "same.txt", dst),
		&ChangeKind::Skip {
			reason: SkipReason::DuplicateCandidate
		}
	);
	assert_eq!(
		change_at(&plan, "verified.txt", dst),
		&ChangeKind::Skip {
			reason: SkipReason::DuplicateConfirmed
		}
	);
	assert_eq!(
		change_at(&plan, "sub/.DS_Store", dst),
		&ChangeKind::Skip {
			reason: SkipReason::Junk
		}
	);
	assert_eq!(
		change_at(&plan, "diff.txt", dst),
		&ChangeKind::Skip {
			reason: SkipReason::Policy
		}
	);
	assert_eq!(
		change_at(&plan, "clash", dst),
		&ChangeKind::Conflict {
			kind: ConflictKind::FileVsDirectory
		}
	);
	assert_eq!(
		change_at(&plan, "link", dst),
		&ChangeKind::Conflict {
			kind: ConflictKind::LinkVsFile
		}
	);
	assert!(matches!(
		plan.changes[0].change,
		ChangeKind::Conflict { .. }
	));
	assert!(!plan
		.changes
		.iter()
		.any(|change| change.path.path() == Some(&dst.join("keep.txt"))));

	let PlanBasis::Index { revisions } = &plan.basis else {
		panic!("not read from the index: {:?}", plan.basis);
	};
	assert_eq!(revisions.len(), 2, "one revision per store read");
	assert!(revisions.iter().all(|revision| revision.revision > 0));
}

/// Each policy resolves the one collision its own way, and only the
/// collision changes.
#[tokio::test]
async fn the_policy_decides_a_collision() {
	let fixture = Fixture::new().await;
	fixture.index_tree().await;
	let dst = &fixture.destination;

	let overwrite = fixture.plan(MergeConflictPolicy::Overwrite).await;
	assert_eq!(
		change_at(&overwrite, "diff.txt", dst),
		&ChangeKind::Replace {
			existing_size: 5,
			incoming_size: 3,
			reason: ReplaceReason::Overwrite
		}
	);
	assert_eq!(overwrite.summary.replaces, Tally { files: 1, bytes: 3 });
	assert_eq!(overwrite.summary.skips.policy, Tally::default());

	let newer = fixture.plan(MergeConflictPolicy::KeepNewer).await;
	assert_eq!(
		change_at(&newer, "diff.txt", dst),
		&ChangeKind::Replace {
			existing_size: 5,
			incoming_size: 3,
			reason: ReplaceReason::Newer
		}
	);

	let both = fixture.plan(MergeConflictPolicy::KeepBoth).await;
	assert_eq!(
		change_at(&both, "diff (1).txt", dst),
		&ChangeKind::Create { size: 3 }
	);
	assert_eq!(
		both.summary.creates,
		Tally {
			files: 4,
			bytes: 17
		}
	);
	assert_eq!(both.summary.collisions, 1);
}

/// Two sources wanting one place is a conflict the plan cannot settle.
#[tokio::test]
async fn two_sources_claiming_one_place_conflict() {
	let fixture = Fixture::new().await;
	fixture.index_tree().await;
	fixture
		.index(
			&fixture.other,
			&[("new.txt", FileKind::File, 3, T, Some("n2"), None, None)],
		)
		.await;
	let mut input = fixture.input(MergeConflictPolicy::Skip);
	input.sources.paths.push(SdPath::local(&fixture.other));
	let plan = plan_of(&fixture.preview(), &input).await;
	assert_eq!(plan.summary.conflicts, 3);
	assert!(plan.changes.iter().any(|change| change.change
		== ChangeKind::Conflict {
			kind: ConflictKind::Sources
		}));
}

/// Validation passes a good merge with its facts, refuses nesting, a missing
/// or file destination, and a detached source, whose preview still answers
/// from the store.
#[tokio::test]
async fn validation_refuses_what_cannot_run_and_previews_a_detached_source() {
	let fixture = Fixture::new().await;
	fixture.index_tree().await;
	let preview = fixture.preview();
	let codes = |validation: &crate::infra::action::preflight::Validation| {
		validation
			.findings
			.iter()
			.filter(|finding| finding.severity == Severity::Error)
			.map(|finding| finding.code.clone())
			.collect::<Vec<_>>()
	};

	let good = validate::validate(&fixture.input(MergeConflictPolicy::Skip), &preview)
		.await
		.expect("validated");
	assert!(!good.refuses(), "{:?}", good.findings);
	assert_eq!(
		good.facts.executes_on,
		crate::device::get_current_device_slug()
	);
	assert!(good.facts.strategy.is_some());

	let mut nested = fixture.input(MergeConflictPolicy::Skip);
	nested.destination = SdPath::local(&fixture.source);
	let nested = validate::validate(&nested, &preview)
		.await
		.expect("validated");
	assert_eq!(codes(&nested), [NESTED]);

	let mut missing = fixture.input(MergeConflictPolicy::Skip);
	missing.destination = SdPath::local(fixture.destination.join("nope"));
	let missing = validate::validate(&missing, &preview)
		.await
		.expect("validated");
	assert_eq!(codes(&missing), [DESTINATION_MISSING]);

	let file = fixture.destination.join("keep.txt");
	std::fs::write(&file, b"k").expect("file");
	let mut onto_file = fixture.input(MergeConflictPolicy::Skip);
	onto_file.destination = SdPath::local(&file);
	let onto_file = validate::validate(&onto_file, &preview)
		.await
		.expect("validated");
	assert_eq!(codes(&onto_file), [DESTINATION_NOT_DIRECTORY]);

	std::fs::remove_dir_all(&fixture.source).expect("detached");
	let detached = validate::validate(&fixture.input(MergeConflictPolicy::Skip), &preview)
		.await
		.expect("validated");
	assert_eq!(codes(&detached), [SOURCE_DETACHED]);
	let plan = fixture.plan(MergeConflictPolicy::Skip).await;
	assert_eq!(
		plan.summary.creates,
		Tally {
			files: 3,
			bytes: 14
		}
	);
}

/// A merge that does not fit warns with the numbers; one that fits says
/// nothing.
#[test]
fn a_full_disk_is_a_warning_with_numbers() {
	let finding = validate::space_finding(10, 5).expect("a warning");
	assert_eq!(finding.severity, Severity::Warning);
	assert_eq!(finding.code, validate::SPACE);
	assert!(finding.message.contains("10 bytes"));
	assert!(validate::space_finding(5, 10).is_none());
}

/// Real folders for the job: every kind of leaf again, on disk.
fn populate(source: &Path, destination: &Path) {
	std::fs::write(source.join("same.txt"), b"hello").unwrap();
	std::fs::write(destination.join("same.txt"), b"hello").unwrap();
	std::fs::write(source.join("diff.txt"), b"aaa").unwrap();
	std::fs::write(destination.join("diff.txt"), b"bbbb").unwrap();
	std::fs::write(source.join("new.txt"), b"fresh").unwrap();
	std::fs::create_dir_all(source.join("sub")).unwrap();
	std::fs::write(source.join("sub/inner.txt"), b"in").unwrap();
	std::fs::write(source.join("sub/.DS_Store"), b"junk").unwrap();
	std::fs::write(source.join("clash"), b"c").unwrap();
	std::fs::create_dir_all(destination.join("clash")).unwrap();
}

async fn run(fixture: &Fixture, job: FolderMergeJob) -> MergeOutput {
	let handle = fixture
		.library
		.jobs()
		.dispatch(job)
		.await
		.expect("dispatched");
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("not a merge output: {output:?}");
	};
	serde_json::from_value(value).expect("a merge output")
}

fn result_at<'a>(output: &'a MergeOutput, path: &str) -> &'a MergeResult {
	&output
		.outcomes
		.iter()
		.find(|outcome| outcome.path == path)
		.unwrap_or_else(|| panic!("no outcome at {path}"))
		.result
}

/// The job copies what is missing, proves a duplicate by reading it,
/// leaves a collision and a conflict alone under `Skip`, never copies junk,
/// and reports each leaf.
#[tokio::test]
async fn the_job_settles_every_leaf_against_the_live_tree() {
	let fixture = Fixture::new().await;
	populate(&fixture.source, &fixture.destination);
	let output = run(
		&fixture,
		FolderMergeJob::new(fixture.input(MergeConflictPolicy::Skip)),
	)
	.await;

	assert_eq!(
		result_at(&output, "new.txt"),
		&MergeResult::Copied { bytes: 5 }
	);
	assert_eq!(result_at(&output, "sub"), &MergeResult::CreatedDirectory);
	assert_eq!(
		result_at(&output, "sub/inner.txt"),
		&MergeResult::Copied { bytes: 2 }
	);
	assert_eq!(
		result_at(&output, "same.txt"),
		&MergeResult::Skipped {
			reason: SkipReason::DuplicateConfirmed
		}
	);
	assert_eq!(
		result_at(&output, "diff.txt"),
		&MergeResult::Skipped {
			reason: SkipReason::Policy
		}
	);
	assert_eq!(
		result_at(&output, "sub/.DS_Store"),
		&MergeResult::Skipped {
			reason: SkipReason::Junk
		}
	);
	assert_eq!(
		result_at(&output, "clash"),
		&MergeResult::Conflict {
			kind: ConflictKind::FileVsDirectory
		}
	);
	assert_eq!(output.copied, 2);
	assert_eq!(output.conflicts, 1);
	assert_eq!(output.diverged, 0, "no plan, nothing to diverge from");

	let dst = &fixture.destination;
	assert_eq!(std::fs::read(dst.join("new.txt")).unwrap(), b"fresh");
	assert_eq!(std::fs::read(dst.join("sub/inner.txt")).unwrap(), b"in");
	assert_eq!(std::fs::read(dst.join("diff.txt")).unwrap(), b"bbbb");
	assert!(!dst.join("sub/.DS_Store").exists());
	assert!(dst.join("clash").is_dir());
	assert!(fixture.source.join("new.txt").exists(), "nothing consumed");
}

/// Overwrite replaces the collision, keep-both writes it beside the
/// existing file, and a consuming merge leaves the source holding exactly
/// what was not settled.
#[tokio::test]
async fn policies_apply_and_a_consuming_merge_prunes_the_source() {
	let fixture = Fixture::new().await;
	populate(&fixture.source, &fixture.destination);
	let mut input = fixture.input(MergeConflictPolicy::KeepBoth);
	input.consume_sources = true;
	let output = run(&fixture, FolderMergeJob::new(input)).await;

	let dst = &fixture.destination;
	assert_eq!(std::fs::read(dst.join("diff.txt")).unwrap(), b"bbbb");
	assert_eq!(std::fs::read(dst.join("diff (1).txt")).unwrap(), b"aaa");
	assert_eq!(
		result_at(&output, "diff (1).txt"),
		&MergeResult::Copied { bytes: 3 },
		"recorded where it was written"
	);
	assert_eq!(output.consumed, 5, "same, diff, new, inner, junk");
	assert_eq!(output.pruned_directories, 1);

	let src = &fixture.source;
	assert!(!src.join("new.txt").exists());
	assert!(!src.join("same.txt").exists());
	assert!(!src.join("diff.txt").exists());
	assert!(!src.join("sub").exists(), "emptied and pruned");
	assert!(src.join("clash").exists(), "a conflict stays");

	let fixture = Fixture::new().await;
	populate(&fixture.source, &fixture.destination);
	let output = run(
		&fixture,
		FolderMergeJob::new(fixture.input(MergeConflictPolicy::Overwrite)),
	)
	.await;
	assert_eq!(
		result_at(&output, "diff.txt"),
		&MergeResult::Replaced { bytes: 3 }
	);
	assert_eq!(
		std::fs::read(fixture.destination.join("diff.txt")).unwrap(),
		b"aaa"
	);
}

/// A resumed job walks past its cursor without repeating what it settled.
#[tokio::test]
async fn a_resumed_job_continues_past_its_cursor() {
	let fixture = Fixture::new().await;
	populate(&fixture.source, &fixture.destination);
	let mut job = FolderMergeJob::new(fixture.input(MergeConflictPolicy::Skip));
	job.cursor = Some(Cursor {
		source: 0,
		path: "new.txt".to_string(),
	});
	let output = run(&fixture, job).await;

	let settled: Vec<&str> = output
		.outcomes
		.iter()
		.map(|outcome| outcome.path.as_str())
		.collect();
	assert_eq!(
		settled,
		["same.txt", "sub", "sub/.DS_Store", "sub/inner.txt"]
	);
	assert!(!fixture.destination.join("new.txt").exists());
}

/// Where the filesystem changed since the plan, the outcome says so.
#[tokio::test]
async fn an_outcome_reports_divergence_from_the_plan() {
	let fixture = Fixture::new().await;
	populate(&fixture.source, &fixture.destination);
	// The index knows the source's new.txt and nothing at the destination,
	// so the plan says create; on disk the destination already has one.
	fixture
		.index(
			&fixture.source,
			&[("new.txt", FileKind::File, 5, T, Some("n"), None, None)],
		)
		.await;
	fixture.index(&fixture.destination, &[]).await;
	std::fs::write(fixture.destination.join("new.txt"), b"older").unwrap();
	let output = run(
		&fixture,
		FolderMergeJob::new(fixture.input(MergeConflictPolicy::Skip)),
	)
	.await;

	let outcome = output
		.outcomes
		.iter()
		.find(|outcome| outcome.path == "new.txt")
		.expect("new.txt");
	assert_eq!(outcome.planned, Some(ChangeKind::Create { size: 5 }));
	assert_eq!(
		outcome.result,
		MergeResult::Skipped {
			reason: SkipReason::Policy
		}
	);
	assert!(outcome.diverged);
	assert!(output.diverged >= 1);
}

/// A mirror plans a delete for each file only the destination holds, flags
/// the last copies, and the job removes those files and nothing else.
#[tokio::test]
async fn a_mirror_removes_what_no_source_holds() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let source = [
		("same.txt", file, 4, T, Some("s"), None, None),
		("new.txt", file, 3, T, Some("n"), None, None),
	];
	let destination = [
		("same.txt", file, 4, T, Some("s"), None, None),
		("extra.txt", file, 5, T, Some("e"), None, None),
		("held.txt", file, 2, T, Some("h"), None, None),
		("old/gone.txt", file, 1, T, Some("g"), None, None),
	];
	// held.txt's bytes exist elsewhere in the library; extra.txt's do not.
	let elsewhere = [("copy.txt", file, 2, T, Some("h"), None, None)];
	fixture.materialize(&fixture.source, &source);
	fixture.index(&fixture.source, &source).await;
	fixture.materialize(&fixture.destination, &destination);
	fixture.index(&fixture.destination, &destination).await;
	fixture.index(&fixture.other, &elsewhere).await;

	let mut input = fixture.input(MergeConflictPolicy::Overwrite);
	input.remove_extras = true;
	let validation = validate::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	let warning = validation
		.findings
		.iter()
		.find(|finding| finding.code == validate::LAST_COPIES)
		.expect("the last copies are counted");
	assert_eq!(warning.severity, Severity::Warning);
	assert!(
		warning.message.starts_with("2 of the 3 files"),
		"{}",
		warning.message
	);

	let plan = super::action::FileMergeAction::preview(input.clone(), &fixture.preview())
		.await
		.expect("planned");
	let delete_at = |name: &str| {
		plan.changes
			.iter()
			.find(|change| {
				change.path.path().map(std::path::PathBuf::as_path)
					== Some(fixture.destination.join(name).as_path())
			})
			.map(|change| change.change.clone())
	};
	assert_eq!(
		delete_at("extra.txt"),
		Some(ChangeKind::Delete { last_copy: true })
	);
	assert_eq!(
		delete_at("held.txt"),
		Some(ChangeKind::Delete { last_copy: false })
	);
	assert_eq!(
		delete_at("old/gone.txt"),
		Some(ChangeKind::Delete { last_copy: true })
	);
	assert_eq!(
		delete_at("same.txt"),
		Some(ChangeKind::Skip {
			reason: SkipReason::DuplicateCandidate
		})
	);
	assert_eq!(plan.summary.deletes.files, 3);
	assert_eq!(plan.summary.creates.files, 1);

	let handle = fixture
		.library
		.jobs()
		.dispatch(FolderMergeJob::new(input))
		.await
		.expect("dispatched");
	let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	let JobOutput::Custom(value) = output else {
		panic!("not a merge output: {output:?}");
	};
	assert_eq!(value["removed_extras"], 3);
	assert!(fixture.destination.join("same.txt").exists());
	assert!(fixture.destination.join("new.txt").exists());
	assert!(!fixture.destination.join("extra.txt").exists());
	assert!(!fixture.destination.join("held.txt").exists());
	assert!(
		!fixture.destination.join("old").exists(),
		"the emptied folder is pruned"
	);
	assert!(
		fixture.source.join("new.txt").exists(),
		"the source is untouched"
	);
}
