//! Whether a job can be undone, and what undoing it would leave.
//!
//! Validation refuses a job with no journal or one still running, counts
//! what cannot be reversed, and names each effect whose subject changed
//! since. The plan is the reverse as an `FsPlan`: a delete for each
//! creation, a move back for each move and each trashing with a known
//! location, and a replacement's previous bytes moved back over it.

use super::{
	action::FileUndoAction,
	input::FileUndoInput,
	reverse::{reversals, Left, Step},
};
use crate::{
	domain::SdPath,
	infra::{
		action::{
			error::ActionError,
			preflight::{
				ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction,
				Validation,
			},
		},
		job::{journal::Recorded, types::JobId},
	},
	ops::files::plan::{
		ChangeKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange, ReplaceReason,
	},
};

pub const NO_JOURNAL: &str = "undo.no_journal";
pub const RUNNING: &str = "undo.running";
pub const CHANGED: &str = "undo.changed";
pub const OCCUPIED: &str = "undo.occupied";
pub const IRREVERSIBLE: &str = "undo.irreversible";
pub const NOTHING: &str = "undo.nothing";

impl ValidatedAction for FileUndoAction {
	async fn validate(
		input: &FileUndoInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		let mut findings = Vec::new();
		let mut facts = ExecutionFacts {
			executes_on: ctx.executes_on(),
			strategy: Some("undo".to_string()),
			..Default::default()
		};
		let jobs = ctx.library().jobs();
		if let Some(info) = jobs
			.get_job_info(input.job)
			.await
			.map_err(|error| ActionError::Internal(error.to_string()))?
		{
			if !info.status.is_terminal() {
				findings.push(Finding::error(
					RUNNING,
					"the job is still running; undo it once it has finished",
				));
			}
		}
		let journal = journal_of(ctx, input.job).await?;
		if journal.is_empty() {
			findings.push(Finding::error(
				NO_JOURNAL,
				"the job recorded nothing that can be undone",
			));
			return Ok(Validation { findings, facts });
		}

		let mut reversible = 0u64;
		let mut irreversible = 0u64;
		for reversal in reversals(&journal, input.effects.as_deref()).await {
			match reversal.outcome {
				Ok(_) => reversible += 1,
				Err(Left::Irreversible) => irreversible += 1,
				Err(Left::Changed) => {
					let effect = journal
						.iter()
						.find(|recorded| recorded.sequence == reversal.sequence)
						.map(|recorded| &recorded.effect);
					let mut finding = Finding::warning(
						CHANGED,
						"a file changed since the job ran; it is left as it is",
					);
					if let Some(path) = effect.and_then(|effect| effect.result()) {
						finding = finding.at(SdPath::local(path));
					}
					findings.push(finding);
				}
				Err(Left::Occupied) => {
					let effect = journal
						.iter()
						.find(|recorded| recorded.sequence == reversal.sequence)
						.map(|recorded| &recorded.effect);
					let mut finding = Finding::warning(
						OCCUPIED,
						"something now sits where a file would go back to; it is left as it is",
					);
					if let Some(path) = effect.and_then(|effect| match effect {
						crate::infra::job::journal::Effect::Moved { from, .. }
						| crate::infra::job::journal::Effect::Trashed { from, .. } => Some(from),
						_ => None,
					}) {
						finding = finding.at(SdPath::local(path));
					}
					findings.push(finding);
				}
			}
		}
		if irreversible > 0 {
			findings.push(Finding::info(
				IRREVERSIBLE,
				format!(
					"{irreversible} of the job's effects cannot be reversed: permanent removals, or replacements and trashings whose previous bytes were not kept"
				),
			));
		}
		if reversible == 0 {
			findings.push(Finding::error(
				NOTHING,
				"nothing the job did can be reversed now",
			));
		}
		facts.estimated_files = Some(reversible);
		Ok(Validation { findings, facts })
	}
}

impl PreviewableAction for FileUndoAction {
	type Plan = FsPlan;

	async fn preview(input: FileUndoInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let journal = journal_of(ctx, input.job).await?;
		let device = crate::device::get_current_device_slug();
		let at = |path: &std::path::Path| SdPath::Physical {
			device_slug: device.clone(),
			path: path.to_path_buf(),
		};
		let mut summary = FsPlanSummary::default();
		let mut changes = PlanChanges::default();
		for reversal in reversals(&journal, input.effects.as_deref()).await {
			let Ok(step) = reversal.outcome else {
				continue;
			};
			let (path, change, bytes) = match step {
				Step::Trash { path, size, .. } => {
					(at(&path), ChangeKind::Delete { last_copy: false }, size)
				}
				Step::MoveBack { from, to, size } => {
					(at(&to), ChangeKind::Move { from: at(&from) }, size)
				}
				Step::Restore {
					path,
					previous,
					size,
				} => {
					let incoming_size = tokio::fs::symlink_metadata(&previous)
						.await
						.map(|meta| meta.len())
						.unwrap_or(0);
					(
						at(&path),
						ChangeKind::Replace {
							existing_size: size,
							incoming_size,
							reason: ReplaceReason::Overwrite,
						},
						incoming_size,
					)
				}
				Step::Attributes { path, to } => {
					(at(&path), ChangeKind::SetAttributes { attributes: to }, 0)
				}
			};
			summary.count(&change, bytes);
			changes.push(PlannedChange { path, change });
		}
		let (changes, truncated) = changes.finish();
		Ok(ctx.plans().retain(FsPlan {
			handle: None,
			basis: PlanBasis::Journal { job: input.job },
			roots: Vec::new(),
			summary,
			changes,
			truncated,
		}))
	}
}

crate::register_validate!(FileUndoAction, "files.undo");
crate::register_preview!(FileUndoAction, "files.undo");

async fn journal_of(ctx: &PreviewContext, job: uuid::Uuid) -> Result<Vec<Recorded>, ActionError> {
	ctx.library()
		.jobs()
		.database()
		.journal(JobId(job))
		.await
		.map_err(|error| ActionError::Internal(error.to_string()))
}
