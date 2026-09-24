//! Whether a rename would run, and what would exist after.
//!
//! Both actions resolve their set the way the job does, so the findings
//! name exactly what the job would refuse: a name the filesystem does not
//! write, a file already at the new name, a name the directory cannot tell
//! from an existing one, and two targets wanting one name. The plan is one
//! move per changed name, at the new name, so the listing shows every new
//! name before anything moves, and a conflict where two targets collide.

use super::{
	action::{FileRenameAction, FileRenameBatchAction},
	input::{FileRenameBatchInput, FileRenameInput},
	resolve::{resolve, RenameSet, Resolved, UNCHANGED},
};
use crate::{
	domain::SdPath,
	infra::action::{
		error::ActionError,
		preflight::{
			ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction, Validation,
		},
	},
	ops::files::plan::{
		ChangeKind, ConflictKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange,
		StoreRevision,
	},
};

impl ValidatedAction for FileRenameAction {
	async fn validate(
		input: &FileRenameInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		validate(
			&RenameSet::Named(vec![(input.target.clone(), input.new_name.clone())]),
			ctx,
		)
		.await
	}
}

impl PreviewableAction for FileRenameAction {
	type Plan = FsPlan;

	async fn preview(input: FileRenameInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		preview(&RenameSet::Named(vec![(input.target, input.new_name)]), ctx).await
	}
}

impl ValidatedAction for FileRenameBatchAction {
	async fn validate(
		input: &FileRenameBatchInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		validate(&set_of(input), ctx).await
	}
}

impl PreviewableAction for FileRenameBatchAction {
	type Plan = FsPlan;

	async fn preview(
		input: FileRenameBatchInput,
		ctx: &PreviewContext,
	) -> Result<FsPlan, ActionError> {
		preview(&set_of(&input), ctx).await
	}
}

crate::register_validate!(FileRenameAction, "files.rename");
crate::register_preview!(FileRenameAction, "files.rename");
crate::register_validate!(FileRenameBatchAction, "files.rename_batch");
crate::register_preview!(FileRenameBatchAction, "files.rename_batch");

fn set_of(input: &FileRenameBatchInput) -> RenameSet {
	RenameSet::Ruled {
		targets: input.targets.clone(),
		rules: input.rules.clone(),
	}
}

async fn validate(set: &RenameSet, ctx: &PreviewContext) -> Result<Validation, ActionError> {
	let Resolved {
		renames,
		mut findings,
		unchanged,
	} = resolve(ctx.volumes(), ctx.index(), set).await;
	if unchanged > 0 {
		findings.push(Finding::info(
			UNCHANGED,
			format!(
				"{unchanged} of {} names {} unchanged",
				unchanged + renames.len() as u64,
				if unchanged == 1 { "is" } else { "are" }
			),
		));
	}
	Ok(Validation {
		findings,
		facts: ExecutionFacts {
			executes_on: ctx.executes_on(),
			strategy: Some("rename".to_string()),
			estimated_files: Some(renames.iter().filter(|rename| !rename.refused).count() as u64),
			estimated_bytes: Some(0),
			free_space_after: None,
		},
	})
}

async fn preview(set: &RenameSet, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
	let resolved = resolve(ctx.volumes(), ctx.index(), set).await;
	let device = crate::device::get_current_device_slug();
	let mut summary = FsPlanSummary::default();
	let mut changes = PlanChanges::default();
	let mut revisions: Vec<StoreRevision> = Vec::new();

	for target in set.targets() {
		for reach in ctx.reach(target).await {
			if revisions
				.iter()
				.any(|known| known.source == reach.source.id)
			{
				continue;
			}
			if let Some(db) = ctx.index().read_store(reach.source.id).await {
				if let Ok(revision) = db.revision().await {
					revisions.push(StoreRevision {
						source: reach.source.id,
						revision: revision.value,
					});
				}
			}
		}
	}

	for rename in resolved.renames {
		let change = if rename.collides {
			ChangeKind::Conflict {
				kind: ConflictKind::Sources,
			}
		} else {
			ChangeKind::Move {
				from: SdPath::Physical {
					device_slug: device.clone(),
					path: rename.from,
				},
			}
		};
		summary.count(&change, rename.size);
		changes.push(PlannedChange {
			path: SdPath::Physical {
				device_slug: device.clone(),
				path: rename.to,
			},
			change,
		});
	}

	let (changes, truncated) = changes.finish();
	Ok(ctx.plans().retain(FsPlan {
		handle: None,
		basis: PlanBasis::Index { revisions },
		roots: Vec::new(),
		summary,
		changes,
		truncated,
	}))
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;
	use std::time::Duration;

	use sd_store::file::FileKind;

	use super::*;
	use crate::{
		infra::job::journal::Effect,
		ops::files::{
			fixture::{Fixture, T},
			rename::{
				job::RenameJob,
				resolve::{CASE_COLLISION, CASE_ONLY, COLLISION, EXISTS, ILLEGAL_NAME},
				rules::{CaseRule, ExtensionCase, RenameRule},
			},
		},
	};

	fn change_at(plan: &FsPlan, path: &std::path::Path) -> Option<ChangeKind> {
		plan.changes
			.iter()
			.find(|change| change.path.path().map(PathBuf::as_path) == Some(path))
			.map(|change| change.change.clone())
	}

	async fn run(fixture: &Fixture, job: RenameJob) -> crate::infra::job::output::JobOutput {
		let handle = fixture
			.library
			.jobs()
			.dispatch(job)
			.await
			.expect("dispatched");
		tokio::time::timeout(Duration::from_secs(30), handle.wait())
			.await
			.expect("in time")
			.expect("completed")
	}

	/// A rename to a name already there is refused, to a name the directory
	/// folds onto another file is refused, and to its own name in another
	/// case is a case-only rename the plan shows as one move.
	#[tokio::test]
	async fn a_single_rename_answers_every_code() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let tree = [
			("a.txt", file, 4, T, Some("a"), None, None),
			("b.txt", file, 3, T, Some("b"), None, None),
		];
		fixture.materialize(&fixture.source, &tree);
		fixture.index(&fixture.source, &tree).await;
		let a = SdPath::local(fixture.source.join("a.txt"));
		let insensitive = tokio::fs::symlink_metadata(fixture.source.join("A.TXT"))
			.await
			.is_ok();

		let taken = FileRenameInput::new(a.clone(), "b.txt");
		let validation = FileRenameAction::validate(&taken, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation.errors().any(|finding| finding.code == EXISTS));

		let illegal = FileRenameInput::new(a.clone(), "a/b.txt");
		let validation = FileRenameAction::validate(&illegal, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation
			.errors()
			.any(|finding| finding.code == ILLEGAL_NAME));

		let cased = FileRenameInput::new(a.clone(), "A.txt");
		let validation = FileRenameAction::validate(&cased, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		assert_eq!(
			validation
				.findings
				.iter()
				.any(|finding| finding.code == CASE_ONLY),
			insensitive
		);
		let plan = FileRenameAction::preview(cased.clone(), &fixture.preview())
			.await
			.expect("planned");
		assert_eq!(
			change_at(&plan, &fixture.source.join("A.txt")),
			Some(ChangeKind::Move { from: a.clone() })
		);
		assert_eq!(plan.summary.moves.files, 1);

		if insensitive {
			let folded = FileRenameInput::new(a.clone(), "B.TXT");
			let validation = FileRenameAction::validate(&folded, &fixture.preview())
				.await
				.expect("validated");
			assert!(validation
				.errors()
				.any(|finding| finding.code == CASE_COLLISION));
		}

		run(&fixture, RenameJob::named(a, "A.txt".to_string())).await;
		assert!(fixture.source.join("A.txt").exists());
		let listed: Vec<String> = std::fs::read_dir(&fixture.source)
			.expect("dir")
			.map(|entry| {
				entry
					.expect("entry")
					.file_name()
					.to_string_lossy()
					.into_owned()
			})
			.collect();
		assert!(listed.contains(&"A.txt".to_string()), "{listed:?}");
	}

	/// Rules preview each new name, two targets wanting one name are a
	/// conflict, and the job renames a chain and a cycle without
	/// overwriting, journaling each move.
	#[tokio::test]
	async fn a_batch_previews_collisions_and_renames_chains_and_cycles() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let tree = [
			("IMG_1.JPG", file, 4, T, Some("1"), None, None),
			("IMG_2.JPG", file, 3, T, Some("2"), None, None),
			("IMG_3.JPG", file, 2, T, Some("3"), None, None),
		];
		fixture.materialize(&fixture.source, &tree);
		fixture.index(&fixture.source, &tree).await;
		let targets: Vec<SdPath> = ["IMG_1.JPG", "IMG_2.JPG", "IMG_3.JPG"]
			.iter()
			.map(|name| SdPath::local(fixture.source.join(name)))
			.collect();

		// Every target wants the same name.
		let collision = FileRenameBatchInput {
			targets: targets.clone(),
			rules: vec![RenameRule::Template {
				pattern: "same{ext}".into(),
			}],
		};
		let validation = FileRenameBatchAction::validate(&collision, &fixture.preview())
			.await
			.expect("validated");
		assert_eq!(
			validation
				.errors()
				.filter(|finding| finding.code == COLLISION)
				.count(),
			2
		);
		let plan = FileRenameBatchAction::preview(collision, &fixture.preview())
			.await
			.expect("planned");
		assert_eq!(plan.summary.conflicts, 2);
		assert_eq!(plan.summary.moves.files, 1);

		// A chain: 1 -> 2 -> 3 -> 4, each new name held by the next target
		// until it moves, then lowercased extensions.
		let chain = FileRenameBatchInput {
			targets: targets.clone(),
			rules: vec![
				RenameRule::Sequence {
					pattern: "IMG_{n}".into(),
					start: 2,
					step: 1,
				},
				RenameRule::Case {
					stem: CaseRule::Upper,
					extension: ExtensionCase::Lower,
				},
			],
		};
		let validation = FileRenameBatchAction::validate(&chain, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		let plan = FileRenameBatchAction::preview(chain.clone(), &fixture.preview())
			.await
			.expect("planned");
		assert_eq!(
			change_at(&plan, &fixture.source.join("IMG_2.jpg")),
			Some(ChangeKind::Move {
				from: targets[0].clone()
			})
		);
		assert_eq!(plan.summary.moves.files, 3);

		let output = run(&fixture, RenameJob::ruled(chain.targets, chain.rules)).await;
		let crate::infra::job::output::JobOutput::Custom(value) = output else {
			panic!("not a rename output: {output:?}");
		};
		assert_eq!(value["renamed"], 3);
		assert_eq!(value["failed"].as_array().map(Vec::len), Some(0));
		assert_eq!(
			std::fs::read_to_string(fixture.source.join("IMG_2.jpg")).expect("moved"),
			"xxxx"
		);
		assert_eq!(
			std::fs::read_to_string(fixture.source.join("IMG_4.jpg")).expect("moved"),
			"xx"
		);
		assert!(!fixture.source.join("IMG_1.JPG").exists());

		// A cycle: 2 <-> 3 swap names.
		let swap = RenameJob::named_many(vec![
			(
				SdPath::local(fixture.source.join("IMG_2.jpg")),
				"IMG_3.jpg".to_string(),
			),
			(
				SdPath::local(fixture.source.join("IMG_3.jpg")),
				"IMG_2.jpg".to_string(),
			),
		]);
		let handle = fixture
			.library
			.jobs()
			.dispatch(swap)
			.await
			.expect("dispatched");
		let job_id = handle.id();
		tokio::time::timeout(Duration::from_secs(30), handle.wait())
			.await
			.expect("in time")
			.expect("completed");
		assert_eq!(
			std::fs::read_to_string(fixture.source.join("IMG_3.jpg")).expect("swapped"),
			"xxxx"
		);
		assert_eq!(
			std::fs::read_to_string(fixture.source.join("IMG_2.jpg")).expect("swapped"),
			"xxx"
		);
		let parked: Vec<_> = std::fs::read_dir(&fixture.source)
			.expect("dir")
			.filter_map(|entry| entry.ok())
			.filter(|entry| entry.file_name().to_string_lossy().contains("renaming"))
			.collect();
		assert!(parked.is_empty(), "no temporary name is left behind");

		let journal = fixture
			.library
			.jobs()
			.database()
			.journal(job_id)
			.await
			.expect("journal");
		let moves: Vec<(PathBuf, PathBuf)> = journal
			.iter()
			.filter_map(|recorded| match &recorded.effect {
				Effect::Moved { from, to, .. } => Some((from.clone(), to.clone())),
				_ => None,
			})
			.collect();
		assert_eq!(moves.len(), 2);
		assert!(moves.iter().all(|(from, _)| !from
			.file_name()
			.unwrap()
			.to_string_lossy()
			.contains("renaming")));
	}
}
