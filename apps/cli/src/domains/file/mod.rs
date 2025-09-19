mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::infra::job::types::JobId;

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum FileCmd {
	/// Copy files
	Copy(FileCopyArgs),
}

pub async fn run(ctx: &Context, cmd: FileCmd) -> Result<()> {
	match cmd {
		FileCmd::Copy(args) => {
			let input: sd_core::ops::files::copy::input::FileCopyInput = args.into();
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "))
			}
			
			// Handle confirmation for file copy operations
			let job_id: JobId = run_copy_with_confirmation(ctx, input).await?;
			print_output!(ctx, &job_id, |id: &JobId| {
				println!("Dispatched copy job {}", id);
			});
		}
	}
	Ok(())
}

/// Run file copy with confirmation handling
async fn run_copy_with_confirmation(
	ctx: &Context,
	input: sd_core::ops::files::copy::input::FileCopyInput,
) -> Result<JobId> {
	use sd_core::infra::action::LibraryAction;
	use sd_core::ops::files::copy::action::FileCopyAction;
	use crate::util::confirm::prompt_for_choice;
	
	// Build the action from input
	let mut action = FileCopyAction::from_input(input)
		.map_err(|e| anyhow::anyhow!("Failed to build action: {}", e))?;
	
	// Simple conflict detection - check if destination exists and overwrite is not enabled
	if !action.options.overwrite {
		let has_conflict = check_for_simple_conflicts(&action).await?;
		if has_conflict {
			use sd_core::infra::action::ConfirmationRequest;
			
			let request = ConfirmationRequest {
				message: "Destination file(s) already exist. What would you like to do?".to_string(),
				choices: vec![
					"Overwrite the existing file(s)".to_string(),
					"Rename the new file(s) (e.g., file.txt -> file (1).txt)".to_string(),
					"Abort this copy operation".to_string(),
				],
			};
			
			let choice_index = prompt_for_choice(request)?;
			
			// Handle the user's choice
			match choice_index {
				0 => {
					// Overwrite: enable overwrite in options
					action.options.overwrite = true;
				}
				1 => {
					// Auto-rename: this would be handled by the job itself
					// For now, we'll just proceed (the job should handle naming conflicts)
				}
				2 => {
					// Abort
					anyhow::bail!("Operation aborted by user");
				}
				_ => {
					anyhow::bail!("Invalid choice selected");
				}
			}
		}
	}
	
	// Execute the action normally
	let job_id: JobId = execute_action!(ctx, action);
	Ok(job_id)
}

/// Simple conflict detection for CLI
async fn check_for_simple_conflicts(
	action: &sd_core::ops::files::copy::action::FileCopyAction,
) -> Result<bool> {
	use sd_core::domain::addressing::SdPath;
	
	// Extract the physical path from the destination SdPath
	let dest_path = match &action.destination {
		SdPath::Physical { path, .. } => path,
		SdPath::Virtual { .. } => {
			// Virtual paths would need different conflict resolution
			return Ok(false);
		}
	};

	// Check if destination exists
	Ok(tokio::fs::metadata(dest_path).await.is_ok())
}
