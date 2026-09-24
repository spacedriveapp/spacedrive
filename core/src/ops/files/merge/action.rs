//! Folder merge action handler

use std::sync::Arc;

use super::input::FileMergeInput;
use super::job::FolderMergeJob;
use crate::{
	context::CoreContext,
	infra::action::{
		error::ActionError,
		preflight::{PreviewContext, PreviewableAction, ValidatedAction, Validation},
		LibraryAction,
	},
	ops::files::{plan::FsPlan, planner::Planner},
};

pub struct FileMergeAction {
	input: FileMergeInput,
}

impl LibraryAction for FileMergeAction {
	type Input = FileMergeInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		input.validate().map_err(|errors| errors.join("; "))?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(FolderMergeJob::new(self.input))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.merge"
	}
}

impl ValidatedAction for FileMergeAction {
	async fn validate(
		input: &FileMergeInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		super::validate::validate(input, ctx).await
	}
}

impl PreviewableAction for FileMergeAction {
	type Plan = FsPlan;

	async fn preview(input: FileMergeInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		let mut planner = Planner::new(ctx, input.on_conflict, false);
		for source in &input.sources.paths {
			planner
				.pair(source, &input.destination, input.consume_sources)
				.await?;
		}
		Ok(ctx.plans().retain(planner.finish()))
	}
}

crate::register_library_action!(FileMergeAction, "files.merge");
crate::register_validate!(FileMergeAction, "files.merge");
crate::register_preview!(FileMergeAction, "files.merge");
