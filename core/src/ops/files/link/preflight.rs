use super::action::{FileLinkAction, FileLinkInput, LinkKind};
use crate::{
	infra::action::{
		error::ActionError,
		preflight::{
			ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction, Validation,
		},
	},
	ops::files::plan::{ChangeKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange},
};

pub const REMOTE_ROOT: &str = "link.remote_root";
pub const EXISTS: &str = "link.exists";
pub const TARGET_MISSING: &str = "link.target_missing";
pub const PARENT_MISSING: &str = "link.parent_missing";
pub const CROSS_VOLUME: &str = "link.cross_volume";
pub const DIRECTORY: &str = "link.directory";

/// Whether a hard link may be made, from what is known of the two sides.
pub fn hard_link_allowed(same_volume: bool, target_is_dir: bool) -> Result<(), &'static str> {
	if target_is_dir {
		return Err(DIRECTORY);
	}
	if !same_volume {
		return Err(CROSS_VOLUME);
	}
	Ok(())
}

impl ValidatedAction for FileLinkAction {
	async fn validate(
		input: &FileLinkInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let mut findings = Vec::new();
		let facts = ExecutionFacts {
			executes_on: ctx.executes_on(),
			strategy: Some(
				match input.kind {
					LinkKind::Symlink => "symlink",
					LinkKind::Hardlink => "hardlink",
				}
				.to_string(),
			),
			estimated_files: Some(1),
			estimated_bytes: Some(0),
			free_space_after: None,
		};
		let (Some(at), Some(target)) = (input.at.as_local_path(), input.target.as_local_path())
		else {
			findings.push(Finding::error(
				REMOTE_ROOT,
				"the link and its target must be on this device",
			));
			return Ok(Validation { findings, facts });
		};
		if tokio::fs::symlink_metadata(at).await.is_ok() {
			findings.push(
				Finding::error(EXISTS, "something is already at the link's place")
					.at(input.at.clone()),
			);
		}
		if !at.parent().is_some_and(|parent| parent.is_dir()) {
			findings.push(
				Finding::error(
					PARENT_MISSING,
					"the folder the link would sit in does not exist",
				)
				.at(input.at.clone()),
			);
		}
		match tokio::fs::metadata(target).await {
			Ok(meta) => {
				if input.kind == LinkKind::Hardlink {
					let theirs = ctx.volumes().volume_for_path(target).await;
					let ours = match at.parent() {
						Some(parent) => ctx.volumes().volume_for_path(parent).await,
						None => None,
					};
					let same_volume = match (theirs, ours) {
						(Some(theirs), Some(ours)) => theirs.fingerprint == ours.fingerprint,
						_ => true,
					};
					if let Err(code) = hard_link_allowed(same_volume, meta.is_dir()) {
						findings.push(
							Finding::error(
								code,
								match code {
									DIRECTORY => "a hard link cannot point at a directory",
									_ => "a hard link cannot cross volumes; make a symlink",
								},
							)
							.at(input.target.clone()),
						);
					}
				}
			}
			Err(_) => findings.push(
				Finding::error(TARGET_MISSING, "the target is not there").at(input.target.clone()),
			),
		}
		Ok(Validation { findings, facts })
	}
}

impl PreviewableAction for FileLinkAction {
	type Plan = FsPlan;

	async fn preview(input: FileLinkInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let mut summary = FsPlanSummary::default();
		let mut changes = PlanChanges::default();
		let change = ChangeKind::Create { size: 0 };
		summary.count(&change, 0);
		changes.push(PlannedChange {
			path: input.at.clone(),
			change,
		});
		let (changes, truncated) = changes.finish();
		Ok(ctx.plans().retain(FsPlan {
			handle: None,
			basis: PlanBasis::Index {
				revisions: Vec::new(),
			},
			roots: Vec::new(),
			summary,
			changes,
			truncated,
		}))
	}
}

crate::register_validate!(FileLinkAction, "files.link");
crate::register_preview!(FileLinkAction, "files.link");
