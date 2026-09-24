//! Whether organize or flatten would run, and what the folder would hold.

use super::{
	action::{FileFlattenAction, FileOrganizeAction},
	input::{FileFlattenInput, FileOrganizeInput},
	plan::{flatten, organize, PlanError, Rearrangement, Stays},
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
		SkipReason,
	},
};

pub const REMOTE_ROOT: &str = "organize.remote_root";
pub const MISSING: &str = "organize.missing";
pub const UNTRACKED: &str = "organize.untracked";
pub const CONFLICTS: &str = "organize.conflicts";
pub const NOTHING: &str = "organize.nothing";

impl ValidatedAction for FileOrganizeAction {
	async fn validate(
		input: &FileOrganizeInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let planned = check(&input.scope, ctx).await?;
		let plan = match planned {
			Ok(()) => Some(organize(ctx.volumes(), ctx.index(), input).await),
			Err(finding) => return Ok(refused(ctx, finding)),
		};
		Ok(validation(ctx, plan))
	}
}

impl PreviewableAction for FileOrganizeAction {
	type Plan = FsPlan;

	async fn preview(
		input: FileOrganizeInput,
		ctx: &PreviewContext,
	) -> Result<FsPlan, ActionError> {
		let rearrangement = organize(ctx.volumes(), ctx.index(), &input)
			.await
			.map_err(failed)?;
		Ok(ctx.plans().retain(plan_of(rearrangement)))
	}
}

impl ValidatedAction for FileFlattenAction {
	async fn validate(
		input: &FileFlattenInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let planned = check(&input.scope, ctx).await?;
		let plan = match planned {
			Ok(()) => Some(flatten(ctx.volumes(), ctx.index(), input).await),
			Err(finding) => return Ok(refused(ctx, finding)),
		};
		Ok(validation(ctx, plan))
	}
}

impl PreviewableAction for FileFlattenAction {
	type Plan = FsPlan;

	async fn preview(input: FileFlattenInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let rearrangement = flatten(ctx.volumes(), ctx.index(), &input)
			.await
			.map_err(failed)?;
		Ok(ctx.plans().retain(plan_of(rearrangement)))
	}
}

crate::register_validate!(FileOrganizeAction, "files.organize");
crate::register_preview!(FileOrganizeAction, "files.organize");
crate::register_validate!(FileFlattenAction, "files.flatten");
crate::register_preview!(FileFlattenAction, "files.flatten");

/// The scope is on this device, there, a folder, and tracked.
async fn check(scope: &SdPath, ctx: &PreviewContext) -> Result<Result<(), Finding>, ActionError> {
	let Some(local) = scope.as_local_path() else {
		return Ok(Err(Finding::error(
			REMOTE_ROOT,
			"the folder is on another device; run this there with --device",
		)
		.at(scope.clone())));
	};
	match tokio::fs::metadata(local).await {
		Ok(meta) if meta.is_dir() => {}
		_ => {
			return Ok(Err(
				Finding::error(MISSING, "the folder is not there").at(scope.clone())
			))
		}
	}
	if ctx.reach(scope).await.is_empty() {
		return Ok(Err(Finding::error(
			UNTRACKED,
			"the folder is outside every tracked source; the plan needs its index",
		)
		.at(scope.clone())));
	}
	Ok(Ok(()))
}

fn refused(ctx: &PreviewContext, finding: Finding) -> Validation {
	Validation {
		findings: vec![finding],
		facts: ExecutionFacts {
			executes_on: ctx.executes_on(),
			strategy: Some("rename".to_string()),
			..Default::default()
		},
	}
}

fn validation(
	ctx: &PreviewContext,
	planned: Option<Result<Rearrangement, PlanError>>,
) -> Validation {
	let mut findings = Vec::new();
	let mut facts = ExecutionFacts {
		executes_on: ctx.executes_on(),
		strategy: Some("rename".to_string()),
		..Default::default()
	};
	match planned {
		Some(Ok(rearrangement)) => {
			let conflicts = rearrangement
				.left
				.iter()
				.filter(|(_, _, stays)| *stays == Stays::Conflict)
				.count();
			if conflicts > 0 {
				findings.push(Finding::warning(
					CONFLICTS,
					format!("{conflicts} files want a place another file holds; they stay where they are"),
				));
			}
			if rearrangement.moves.is_empty() {
				findings.push(Finding::info(
					NOTHING,
					format!(
						"nothing moves: {} files are already where the rule puts them",
						rearrangement.in_place
					),
				));
			}
			facts.estimated_files = Some(rearrangement.moves.len() as u64);
			facts.estimated_bytes = Some(0);
		}
		Some(Err(error)) => findings.push(Finding::error(UNTRACKED, error.to_string())),
		None => {}
	}
	Validation { findings, facts }
}

fn plan_of(rearrangement: Rearrangement) -> FsPlan {
	let device = crate::device::get_current_device_slug();
	let at = |path: std::path::PathBuf| SdPath::Physical {
		device_slug: device.clone(),
		path,
	};
	let mut summary = FsPlanSummary::default();
	let mut changes = PlanChanges::default();
	for directory in rearrangement.directories {
		summary.count(&ChangeKind::CreateDirectory, 0);
		changes.push(PlannedChange {
			path: at(directory),
			change: ChangeKind::CreateDirectory,
		});
	}
	for planned in rearrangement.moves {
		let change = ChangeKind::Move {
			from: at(planned.from),
		};
		summary.count(&change, planned.size);
		changes.push(PlannedChange {
			path: at(planned.to),
			change,
		});
	}
	for (from, wanted, stays) in rearrangement.left {
		let (path, change) = match stays {
			Stays::Conflict => (
				wanted,
				ChangeKind::Conflict {
					kind: ConflictKind::Sources,
				},
			),
			Stays::Policy => (
				from,
				ChangeKind::Skip {
					reason: SkipReason::Policy,
				},
			),
		};
		summary.count(&change, 0);
		changes.push(PlannedChange {
			path: at(path),
			change,
		});
	}
	let (changes, truncated) = changes.finish();
	FsPlan {
		handle: None,
		basis: PlanBasis::Index {
			revisions: rearrangement.revisions,
		},
		roots: Vec::new(),
		summary,
		changes,
		truncated,
	}
}

fn failed(error: PlanError) -> ActionError {
	match error {
		PlanError::Remote(_) | PlanError::Untracked(_) => {
			ActionError::InvalidInput(error.to_string())
		}
		PlanError::Read(error) => ActionError::Internal(error.to_string()),
	}
}
