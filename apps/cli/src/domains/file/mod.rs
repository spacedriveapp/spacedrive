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
	/// Get file information
	Info(FileInfoArgs),
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
		FileCmd::Info(args) => {
			let file_info = get_file_info(ctx, &args.path).await?;
			print_output!(ctx, &file_info, |info: &Option<sd_core::domain::File>| {
				match info {
					Some(file) => {
						println!("{}", serde_json::to_string_pretty(file).unwrap());
					}
					None => {
						println!("File not found or not indexed in Spacedrive");
					}
				}
			});
		}
	}
	Ok(())
}

/// Run file copy with confirmation handling
async fn run_copy_with_confirmation(
	ctx: &Context,
	mut input: sd_core::ops::files::copy::input::FileCopyInput,
) -> Result<JobId> {
	use crate::util::confirm::prompt_for_choice;
	use sd_core::infra::action::LibraryAction;
	use sd_core::ops::files::copy::action::FileCopyAction;

	// Build the action from input for validation purposes
	let action = FileCopyAction::from_input(input.clone())
		.map_err(|e| anyhow::anyhow!("Failed to build action: {}", e))?;

	// Use the action's validation method to check for conflicts
	// For CLI validation, we'll use a simplified approach since we don't have full library context
	// In a production system, you'd want to pass the actual library context

	// Simple conflict detection - check if destination exists and overwrite is not enabled
	if !input.overwrite {
		let has_conflict = check_for_simple_conflicts(&action).await?;
		if has_conflict {
			use sd_core::infra::action::ConfirmationRequest;

			let request = ConfirmationRequest {
				message: "Destination file(s) already exist. What would you like to do?"
					.to_string(),
				choices: vec![
					"Overwrite the existing file(s)".to_string(),
					"Rename the new file(s) (e.g., file.txt -> file (1).txt)".to_string(),
					"Abort this copy operation".to_string(),
				],
			};

			let choice_index = prompt_for_choice(request)?;

			// Apply the user's choice to the input
			match choice_index {
				0 => {
					// Overwrite: set conflict resolution in input
					use sd_core::ops::files::copy::action::FileConflictResolution;
					input.on_conflict = Some(FileConflictResolution::Overwrite);
				}
				1 => {
					// Auto-rename: set conflict resolution in input
					use sd_core::ops::files::copy::action::FileConflictResolution;
					input.on_conflict = Some(FileConflictResolution::AutoModifyName);
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

	// Execute the action using the input
	let job_id: JobId = execute_action!(ctx, input);
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
		SdPath::Content { .. } => {
			// Content paths cannot be destinations for copy operations
			return Ok(false);
		}
	};

	// Resolve the actual destination file path using the same logic as the core copy job
	let final_dest_path = resolve_final_destination_path(action, dest_path)?;

	// Check if the resolved destination file exists
	Ok(tokio::fs::metadata(&final_dest_path).await.is_ok())
}

/// Resolve the final destination path using the same logic as the core copy job
/// This handles the case where destination is a directory vs a file path
fn resolve_final_destination_path(
	action: &sd_core::ops::files::copy::action::FileCopyAction,
	dest_path: &std::path::PathBuf,
) -> Result<std::path::PathBuf> {
	use sd_core::domain::addressing::SdPath;

	if action.sources.paths.len() > 1 {
		// Multiple sources: destination must be a directory
		if let Some(first_source) = action.sources.paths.first() {
			if let SdPath::Physical {
				path: source_path, ..
			} = first_source
			{
				if let Some(filename) = source_path.file_name() {
					return Ok(dest_path.join(filename));
				}
			}
		}
		// Fallback
		return Ok(dest_path.clone());
	} else {
		// Single source: check if destination is a directory
		if dest_path.is_dir() {
			// Destination is a directory, join with source filename
			if let Some(source) = action.sources.paths.first() {
				if let SdPath::Physical {
					path: source_path, ..
				} = source
				{
					if let Some(filename) = source_path.file_name() {
						return Ok(dest_path.join(filename));
					}
				}
			}
			// Fallback
			return Ok(dest_path.clone());
		} else {
			// Destination is a file path, use as-is
			return Ok(dest_path.clone());
		}
	}
}

/// Get file information using the FileByPathQuery
async fn get_file_info(
	ctx: &Context,
	path: &std::path::Path,
) -> Result<Option<sd_core::domain::File>> {
	use sd_core::ops::files::query::FileByPathQuery;

	// Create the query with the local path
	let query = FileByPathQuery::new(path.to_path_buf());

	// Execute the query using the core client
	let json_response = ctx.core.query(&query, ctx.library_id).await?;
	let result: Option<sd_core::domain::File> = serde_json::from_value(json_response)?;

	Ok(result)
}
