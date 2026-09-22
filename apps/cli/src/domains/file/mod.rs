mod args;

use anyhow::Result;
use clap::Subcommand;
use comfy_table::presets::UTF8_BORDERS_ONLY;

use crate::format_bytes;
use crate::util::prelude::*;

use std::io::Write;

use crate::context::{Context, OutputFormat};
use sd_core::domain::SdPath;
use sd_core::infra::job::handle::JobReceipt;
use sd_core::infra::job::types::JobId;
use sd_core::infra::query::LibraryQuery;
use sd_core::ops::files::delete::{DeleteTargets, FileDeleteInput};
use sd_core::ops::paths::compare::{
	CompareBy, CompareEntry, CompareSet, CompareTotals, Comparison, PathCompareInput,
	PathCompareOutput, MAX_PAGE,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum FileCmd {
	/// Copy files
	Copy(FileCopyArgs),
	/// Get file information
	Info(FileInfoArgs),
	/// List directory contents
	List(FileListArgs),
	/// Compare what two indexed folders hold
	Compare(FileCompareArgs),
	/// Delete files, or from a folder what a comparison names
	Delete(FileDeleteArgs),
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
		FileCmd::List(args) => {
			let sort_by = match args.sort_by.to_lowercase().as_str() {
				"name" => sd_core::ops::files::query::DirectorySortBy::Name,
				"modified" => sd_core::ops::files::query::DirectorySortBy::Modified,
				"size" => sd_core::ops::files::query::DirectorySortBy::Size,
				"type" => sd_core::ops::files::query::DirectorySortBy::Type,
				_ => {
					anyhow::bail!(
						"Invalid sort option: {}. Valid options are: name, modified, size, type",
						args.sort_by
					);
				}
			};
			let directory_listing =
				list_directory(ctx, &args.path, args.limit, args.include_hidden, sort_by).await?;
			print_output!(
				ctx,
				&directory_listing,
				|listing: &sd_core::ops::files::query::DirectoryListingOutput| {
					println!("Directory: {}", args.path.display());
					println!("Found {} items:", listing.files.len());
					println!();

					// Create a table to display the results
					let mut table = comfy_table::Table::new();
					table.load_preset(UTF8_BORDERS_ONLY);
					table.set_header(vec!["Name", "Type", "Size", "Modified"]);

					for file in &listing.files {
						// Determine if this is a directory by checking if size is None
						// In Spacedrive, directories typically have size = 0 or None
						let is_directory = file.size == 0;
						let file_type = if is_directory { "Directory" } else { "File" };

						let size_str = if is_directory {
							"-".to_string()
						} else {
							format_bytes(file.size)
						};

						table.add_row(vec![
							file.name.clone(),
							file_type.to_string(),
							size_str,
							file.modified_at.format("%Y-%m-%d %H:%M:%S").to_string(),
						]);
					}

					println!("{}", table);
				}
			);
		}
		FileCmd::Compare(args) => {
			let cap = args.limit;
			compare_folders(ctx, args.into_input()?, cap).await?;
		}
		FileCmd::Delete(args) => {
			let yes = args.yes;
			delete_files(ctx, args.into_input()?, yes).await?;
		}
	}
	Ok(())
}

/// Say what will go, then delete it behind a y/N prompt unless `yes`. For a
/// comparison that is its counts and the set, as `file compare` prints them,
/// from a first page of it; the job derives the set again as it runs.
async fn delete_files(ctx: &Context, input: FileDeleteInput, yes: bool) -> Result<()> {
	let human = matches!(ctx.format, OutputFormat::Human);
	let mut out = std::io::stdout();
	let (count, from) = match &input.targets {
		DeleteTargets::Comparison { comparison } => {
			let page: PathCompareOutput = execute_query!(
				ctx,
				PathCompareInput {
					comparison: comparison.clone(),
					after: None,
					limit: 1,
				}
			);
			let totals = page.totals.unwrap_or_default();
			if human {
				print_summary(&mut out, comparison, &totals)?;
				writeln!(out)?;
			}
			(totals.count(comparison.show) as usize, " from A")
		}
		DeleteTargets::Paths { paths } => (paths.len(), ""),
	};
	if count == 0 {
		writeln!(out, "Nothing to delete")?;
		return Ok(());
	}
	let destination = if input.permanent {
		"permanently"
	} else {
		"to the trash"
	};
	confirm_or_abort(&format!("Delete {count} files{from} {destination}?"), yes)?;

	let receipt: JobReceipt = execute_action!(ctx, input);
	print_output!(ctx, &receipt, |receipt: &JobReceipt| {
		println!("Dispatched delete job {}", receipt.id);
	});
	Ok(())
}

/// Every page of a comparison, up to `cap` files. Human output prints each
/// page as it arrives, one line per file; JSON output is one document holding
/// every entry. A reader that closes the pipe early, like `head`, ends the
/// listing quietly.
async fn compare_folders(ctx: &Context, input: PathCompareInput, cap: Option<u32>) -> Result<()> {
	match walk_comparison(ctx, input, cap).await {
		Err(error)
			if error
				.downcast_ref::<std::io::Error>()
				.is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe) =>
		{
			Ok(())
		}
		result => result,
	}
}

async fn walk_comparison(
	ctx: &Context,
	mut input: PathCompareInput,
	cap: Option<u32>,
) -> Result<()> {
	let human = matches!(ctx.format, OutputFormat::Human);
	let mut out = std::io::stdout();
	let mut totals = None;
	let mut entries = Vec::new();
	let mut listed = 0;
	let next = loop {
		input.limit = cap.map_or(MAX_PAGE, |cap| (cap - listed).min(MAX_PAGE));
		let page: PathCompareOutput = execute_query!(ctx, input.clone());
		if let Some(first) = page.totals {
			if human {
				print_summary(&mut out, &input.comparison, &first)?;
			}
			totals = Some(first);
		}
		for entry in page.entries {
			if human {
				print_entry(&mut out, &entry, listed == 0)?;
			} else {
				entries.push(entry);
			}
			listed += 1;
		}
		match page.next {
			Some(cursor) if cap.is_none_or(|cap| listed < cap) => input.after = Some(cursor),
			next => break next,
		}
	};

	if !human {
		let output = PathCompareOutput {
			entries,
			next,
			totals,
		};
		writeln!(out, "{}", serde_json::to_string_pretty(&output)?)?;
		return Ok(());
	}
	if listed == 0 {
		writeln!(out, "None")?;
	} else if let Some(total) = totals
		.map(|totals| totals.count(input.comparison.show))
		.filter(|&total| listed < total)
	{
		writeln!(out, "\n{listed} of {total} listed")?;
	}
	Ok(())
}

/// Name A and B, say how their files matched, and print every set's count
/// from the comparison's first page, then the title of the listing below.
fn print_summary(
	out: &mut impl Write,
	comparison: &Comparison,
	totals: &CompareTotals,
) -> std::io::Result<()> {
	let folder = |path: &SdPath| {
		path.path()
			.map_or_else(|| path.to_string(), |path| path.display().to_string())
	};
	writeln!(out, "A  {}", folder(&comparison.a))?;
	writeln!(out, "B  {}", folder(&comparison.b))?;
	let (by, sets): (&str, &[CompareSet]) = match comparison.by {
		CompareBy::Path => (
			"path",
			&[
				CompareSet::OnlyA,
				CompareSet::OnlyB,
				CompareSet::Both,
				CompareSet::Different,
			],
		),
		CompareBy::Content => (
			"content",
			&[CompareSet::OnlyA, CompareSet::OnlyB, CompareSet::Both],
		),
	};
	writeln!(out, "Matched by {by}\n")?;

	let label_width = sets
		.iter()
		.map(|&set| set_label(set).len())
		.max()
		.unwrap_or(0);
	let count_width = sets
		.iter()
		.map(|&set| totals.count(set).to_string().len())
		.max()
		.unwrap_or(0);
	for &set in sets {
		writeln!(
			out,
			"{:<label_width$}    {:>count_width$}",
			set_label(set),
			totals.count(set)
		)?;
	}
	if totals.unhashed_a + totals.unhashed_b > 0 {
		writeln!(
			out,
			"Not hashed yet: {} in A, {} in B",
			totals.unhashed_a, totals.unhashed_b
		)?;
	}
	writeln!(out, "\n{}", set_label(comparison.show))
}

/// How a set is named in the counts and over its listing.
fn set_label(set: CompareSet) -> &'static str {
	match set {
		CompareSet::OnlyA => "Only in A",
		CompareSet::OnlyB => "Only in B",
		CompareSet::Both => "In both",
		CompareSet::Different => "Different",
	}
}

/// One file per line: each side's size and modification time in fixed
/// columns, then the path, so a long path never pushes the columns out of
/// line. The first file brings a header: column names when files have one
/// side, since the listing's title already names the folder, and A and B when
/// they have both.
fn print_entry(out: &mut impl Write, entry: &CompareEntry, header: bool) -> std::io::Result<()> {
	// Sizes right-align so their units line up; a time prints as 16
	// characters of `%Y-%m-%d %H:%M`.
	const SIZE: usize = 9;
	const MODIFIED: usize = 16;
	const SIDE: usize = SIZE + 2 + MODIFIED;

	let sides: Vec<(&str, &sd_core::domain::File)> = [("A", &entry.a), ("B", &entry.b)]
		.into_iter()
		.filter_map(|(label, file)| file.as_ref().map(|file| (label, file)))
		.collect();
	if header {
		let columns: Vec<String> = match sides.as_slice() {
			[_] => vec![format!("{:>SIZE$}  {:<MODIFIED$}", "Size", "Modified")],
			_ => sides
				.iter()
				.map(|(label, _)| format!("{label:<SIDE$}"))
				.collect(),
		};
		writeln!(out, "{}   Path", columns.join("   "))?;
	}
	let cells: Vec<String> = sides
		.iter()
		.map(|(_, file)| {
			format!(
				"{:>SIZE$}  {}",
				format_bytes(file.size),
				file.modified_at.format("%Y-%m-%d %H:%M")
			)
		})
		.collect();
	writeln!(out, "{}   {}", cells.join("   "), entry.path)
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
				metadata: None,
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
		SdPath::Cloud { .. } => {
			// Cloud paths are not yet supported for copy operations
			return Ok(false);
		}
		SdPath::Content { .. } => {
			// Content paths cannot be destinations for copy operations
			return Ok(false);
		}
		SdPath::Sidecar { .. } => {
			// Sidecar paths cannot be destinations for copy operations
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

/// List directory contents using the DirectoryListingQuery
async fn list_directory(
	ctx: &Context,
	path: &std::path::Path,
	limit: Option<u32>,
	include_hidden: bool,
	sort_by: sd_core::ops::files::query::DirectorySortBy,
) -> Result<sd_core::ops::files::query::DirectoryListingOutput> {
	use sd_core::domain::addressing::SdPath;
	use sd_core::ops::files::query::DirectoryListingQuery;

	// Create the SdPath for the directory
	let sd_path = SdPath::local(path.to_path_buf());

	// Create the query input
	let input = sd_core::ops::files::query::DirectoryListingInput {
		path: sd_path,
		limit,
		include_hidden: Some(include_hidden),
		sort_by,
		folders_first: None,
	};

	// Execute the query using the core client
	let json_response = ctx.core.query(&input, ctx.library_id).await?;
	let result: sd_core::ops::files::query::DirectoryListingOutput =
		serde_json::from_value(json_response)?;

	Ok(result)
}
