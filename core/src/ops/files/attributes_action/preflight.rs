use super::{
	action::{FileSetAttributesAction, FileSetAttributesInput},
	fs,
};
use crate::{
	infra::action::{
		error::ActionError,
		preflight::{
			ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction, Validation,
		},
	},
	ops::files::plan::{ChangeKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange},
};

pub const REMOTE_ROOT: &str = "attributes.remote_root";
pub const MISSING: &str = "attributes.missing";
pub const UNSUPPORTED: &str = "attributes.unsupported";
pub const UNCHANGED: &str = "attributes.unchanged";

impl ValidatedAction for FileSetAttributesAction {
	async fn validate(
		input: &FileSetAttributesInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let mut findings = Vec::new();
		let mut changing = 0;
		let mut unchanged = 0;
		for path in &input.paths {
			let Some(local) = path.as_local_path() else {
				findings.push(
					Finding::error(REMOTE_ROOT, "a file is on another device").at(path.clone()),
				);
				continue;
			};
			let Ok(current) = fs::read(local).await else {
				findings.push(Finding::error(MISSING, "a file is not there").at(path.clone()));
				continue;
			};
			if let Some(volume) = ctx.volumes().volume_for_path(local).await {
				if let Err(reason) = fs::supported(&volume.file_system, &input.attributes) {
					findings.push(Finding::error(UNSUPPORTED, reason).at(path.clone()));
					continue;
				}
			}
			if fs::differs(&current, &input.attributes) {
				changing += 1;
			} else {
				unchanged += 1;
			}
		}
		if unchanged > 0 {
			findings.push(Finding::info(
				UNCHANGED,
				format!("{unchanged} files already carry these attributes"),
			));
		}
		Ok(Validation {
			findings,
			facts: ExecutionFacts {
				executes_on: ctx.executes_on(),
				strategy: Some("attributes".to_string()),
				estimated_files: Some(changing),
				estimated_bytes: Some(0),
				free_space_after: None,
			},
		})
	}
}

impl PreviewableAction for FileSetAttributesAction {
	type Plan = FsPlan;

	async fn preview(
		input: FileSetAttributesInput,
		ctx: &PreviewContext,
	) -> Result<FsPlan, ActionError> {
		let mut summary = FsPlanSummary::default();
		let mut changes = PlanChanges::default();
		for path in &input.paths {
			let Some(local) = path.as_local_path() else {
				continue;
			};
			let Ok(current) = fs::read(local).await else {
				continue;
			};
			if !fs::differs(&current, &input.attributes) {
				continue;
			}
			let change = ChangeKind::SetAttributes {
				attributes: fs::changing(&current, &input.attributes),
			};
			summary.count(&change, 0);
			changes.push(PlannedChange {
				path: path.clone(),
				change,
			});
		}
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

crate::register_validate!(FileSetAttributesAction, "files.set_attributes");
crate::register_preview!(FileSetAttributesAction, "files.set_attributes");
