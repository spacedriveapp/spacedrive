mod args;

use anyhow::Result;
use clap::Subcommand;
use comfy_table::presets::UTF8_BORDERS_ONLY;

use crate::format_bytes;
use crate::util::prelude::*;

use std::io::Write;

use crate::context::{Context, OutputFormat};
use sd_core::domain::SdPath;
use sd_core::infra::action::preflight::{Severity, Validation};
use sd_core::infra::job::handle::JobReceipt;
use sd_core::infra::query::LibraryQuery;
use sd_core::ops::files::copy::input::FileCopyInput;
use sd_core::ops::files::delete::{DeleteTargets, FileDeleteInput};
use sd_core::ops::files::merge::FileMergeInput;
use sd_core::ops::files::plan::{ChangeKind, FsPlan, PlanBasis, SkipReason};
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
	/// Merge folders into an existing folder, after seeing the plan
	Merge(FileMergeArgs),
}

pub async fn run(ctx: &Context, cmd: FileCmd) -> Result<()> {
	match cmd {
		FileCmd::Copy(args) => {
			let (dry_run, yes) = (args.dry_run, args.yes);
			let input = args.into_input()?;
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "))
			}
			copy_files(ctx, input, dry_run, yes).await?;
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
			let (dry_run, yes) = (args.dry_run, args.yes);
			delete_files(ctx, args.into_input()?, dry_run, yes).await?;
		}
		FileCmd::Merge(args) => {
			let (dry_run, yes) = (args.dry_run, args.yes);
			merge_folders(ctx, args.into_input()?, dry_run, yes).await?;
		}
	}
	Ok(())
}

/// Validate, show the plan, and dispatch the same input behind a y/N prompt.
/// An error finding stops here, as the daemon would refuse it anyway;
/// `--dry-run` stops after the plan.
async fn merge_folders(
	ctx: &Context,
	input: FileMergeInput,
	dry_run: bool,
	yes: bool,
) -> Result<()> {
	let human = matches!(ctx.format, OutputFormat::Human);
	let mut out = std::io::stdout();
	let validation: Validation = execute_validate!(ctx, input.clone());
	let plan: FsPlan = execute_preview!(ctx, input.clone());

	if human {
		for source in &input.sources.paths {
			writeln!(out, "Merge {}", folder_name(source))?;
		}
		writeln!(out, "Into  {}", folder_name(&input.destination))?;
		writeln!(
			out,
			"On conflict: {}{}\n",
			match input.on_conflict {
				sd_core::ops::files::merge::MergeConflictPolicy::Skip => "skip",
				sd_core::ops::files::merge::MergeConflictPolicy::Overwrite => "overwrite",
				sd_core::ops::files::merge::MergeConflictPolicy::KeepBoth => "keep both",
				sd_core::ops::files::merge::MergeConflictPolicy::KeepNewer => "keep newer",
			},
			if input.consume_sources {
				"; consuming the sources"
			} else {
				""
			}
		)?;
		print_validation(&mut out, &validation)?;
		print_plan(&mut out, &plan)?;
	}
	if let Some(stop) = stop_before_dispatch(human, &validation, &plan, dry_run)? {
		return stop;
	}

	let summary = &plan.summary;
	confirm_or_abort(
		&format!(
			"Merge {} files ({}) into {}{}?",
			summary.creates.files + summary.replaces.files,
			format_bytes(summary.bytes_needed()),
			folder_name(&input.destination),
			if input.consume_sources {
				", consuming the sources"
			} else {
				""
			}
		),
		yes,
	)?;

	let receipt: JobReceipt = execute_action!(ctx, input);
	if human {
		writeln!(out, "Dispatched merge job {}", receipt.id)?;
	} else {
		crate::util::output::print_json(&serde_json::json!({
			"validation": validation,
			"plan": plan,
			"job": receipt,
		}));
	}
	Ok(())
}

fn folder_name(path: &SdPath) -> String {
	path.path()
		.map_or_else(|| path.to_string(), |path| path.display().to_string())
}

fn refusal_summary(validation: &Validation) -> String {
	validation
		.errors()
		.map(|finding| format!("{} ({})", finding.message, finding.code))
		.collect::<Vec<_>>()
		.join("; ")
}

/// Copy or move with preflight: validate, show the plan, dispatch the same
/// input behind a y/N prompt. An error finding stops here; `--dry-run`
/// stops after the plan.
async fn copy_files(ctx: &Context, input: FileCopyInput, dry_run: bool, yes: bool) -> Result<()> {
	let human = matches!(ctx.format, OutputFormat::Human);
	let mut out = std::io::stdout();
	let verb = if input.move_files { "Move" } else { "Copy" };
	let validation: Validation = execute_validate!(ctx, input.clone());
	let plan: FsPlan = execute_preview!(ctx, input.clone());

	if human {
		for source in &input.sources.paths {
			writeln!(out, "{verb} {}", folder_name(source))?;
		}
		writeln!(out, "To   {}\n", folder_name(&input.destination))?;
		print_validation(&mut out, &validation)?;
		print_plan(&mut out, &plan)?;
	}
	if let Some(stop) = stop_before_dispatch(human, &validation, &plan, dry_run)? {
		return stop;
	}

	let summary = &plan.summary;
	confirm_or_abort(
		&format!(
			"{verb} {} files ({}) to {}?",
			summary.creates.files + summary.replaces.files + summary.moves.files,
			format_bytes(summary.bytes_needed()),
			folder_name(&input.destination),
		),
		yes,
	)?;

	let receipt: JobReceipt = execute_action!(ctx, input);
	if human {
		writeln!(out, "Dispatched {} job {}", verb.to_lowercase(), receipt.id)?;
	} else {
		crate::util::output::print_json(&serde_json::json!({
			"validation": validation,
			"plan": plan,
			"job": receipt,
		}));
	}
	Ok(())
}

/// Whether a preflight command stops after showing the plan: on a
/// refusal, with the findings as the error, or at `--dry-run`. JSON output
/// carries both answers either way.
fn stop_before_dispatch(
	human: bool,
	validation: &Validation,
	plan: &FsPlan,
	dry_run: bool,
) -> Result<Option<Result<()>>> {
	if !validation.refuses() && !dry_run {
		return Ok(None);
	}
	if !human {
		crate::util::output::print_json(&serde_json::json!({
			"validation": validation,
			"plan": plan,
		}));
	}
	if validation.refuses() {
		anyhow::bail!("refused: {}", refusal_summary(validation));
	}
	Ok(Some(Ok(())))
}

/// The findings, each with its severity and code, and the facts of the
/// execution.
fn print_validation(out: &mut impl Write, validation: &Validation) -> std::io::Result<()> {
	for finding in &validation.findings {
		let severity = match finding.severity {
			Severity::Error => "error",
			Severity::Warning => "warning",
			Severity::Info => "info",
		};
		let at = finding
			.path
			.as_ref()
			.map(|path| format!("  ({})", folder_name(path)))
			.unwrap_or_default();
		writeln!(
			out,
			"{severity:<8} {}: {}{at}",
			finding.code, finding.message
		)?;
	}
	let facts = &validation.facts;
	let mut about = vec![format!("runs on {}", facts.executes_on)];
	if let Some(strategy) = &facts.strategy {
		about.push(format!("via {strategy}"));
	}
	if let (Some(files), Some(bytes)) = (facts.estimated_files, facts.estimated_bytes) {
		about.push(format!("{files} files, {} estimated", format_bytes(bytes)));
	}
	if let Some(after) = facts.free_space_after {
		about.push(format!(
			"{} free after",
			if after < 0 {
				format!("-{}", format_bytes(after.unsigned_abs()))
			} else {
				format_bytes(after as u64)
			}
		));
	}
	writeln!(out, "Facts: {}\n", about.join("; "))
}

/// The plan's counts, then the conflicts and every collision the policy
/// resolved, since those are what a person reads a plan for.
fn print_plan(out: &mut impl Write, plan: &FsPlan) -> std::io::Result<()> {
	let PlanBasis::Index { revisions } = &plan.basis;
	writeln!(out, "Plan, from the index of {} stores:", revisions.len())?;
	let summary = &plan.summary;
	let tally = |files: u64, bytes: u64| format!("{files:>7} files  {:>9}", format_bytes(bytes));
	writeln!(
		out,
		"  Create          {}",
		tally(summary.creates.files, summary.creates.bytes)
	)?;
	writeln!(
		out,
		"  Replace         {}",
		tally(summary.replaces.files, summary.replaces.bytes)
	)?;
	writeln!(
		out,
		"  Skip duplicates {}  ({} confirmed)",
		tally(
			summary.skips.duplicate_candidates.files + summary.skips.duplicates_confirmed.files,
			summary.skips.duplicate_candidates.bytes + summary.skips.duplicates_confirmed.bytes
		),
		summary.skips.duplicates_confirmed.files
	)?;
	writeln!(
		out,
		"  Skip by policy  {}",
		tally(summary.skips.policy.files, summary.skips.policy.bytes)
	)?;
	writeln!(
		out,
		"  Move            {}",
		tally(summary.moves.files, summary.moves.bytes)
	)?;
	writeln!(
		out,
		"  Delete          {}",
		tally(summary.deletes.files, summary.deletes.bytes)
	)?;
	writeln!(out, "  New folders     {:>7}", summary.directories_created)?;
	writeln!(out, "  Merged folders  {:>7}", summary.merged_into)?;
	writeln!(out, "  Junk            {:>7}", summary.skips.junk)?;
	writeln!(out, "  Collisions      {:>7}", summary.collisions)?;
	writeln!(out, "  Conflicts       {:>7}", summary.conflicts)?;

	let notable: Vec<_> = plan
		.changes
		.iter()
		.filter(|change| {
			matches!(
				change.change,
				ChangeKind::Conflict { .. }
					| ChangeKind::Replace { .. }
					| ChangeKind::Skip {
						reason: SkipReason::Policy
					} | ChangeKind::Delete { last_copy: true }
			)
		})
		.collect();
	if !notable.is_empty() {
		writeln!(out, "\nConflicts, collisions and last copies:")?;
		for change in notable {
			let what = match &change.change {
				ChangeKind::Conflict { kind } => format!("conflict {kind:?}"),
				ChangeKind::Replace { reason, .. } => format!("replace ({reason:?})"),
				ChangeKind::Skip { .. } => "skip by policy".to_string(),
				ChangeKind::Delete { .. } => "delete, last copy anywhere".to_string(),
				_ => unreachable!(),
			};
			writeln!(out, "  {what:<28} {}", folder_name(&change.path))?;
		}
	}
	if plan.truncated {
		writeln!(
			out,
			"  (the list stops at the cap; the counts above are complete)"
		)?;
	}
	writeln!(out)
}

/// Delete with preflight: the findings, with the warning only an index can
/// give of which files are the last copy of their bytes, then the plan, then
/// a y/N prompt. A comparison target prints the comparison's counts first.
async fn delete_files(
	ctx: &Context,
	input: FileDeleteInput,
	dry_run: bool,
	yes: bool,
) -> Result<()> {
	let human = matches!(ctx.format, OutputFormat::Human);
	let mut out = std::io::stdout();
	if let (true, DeleteTargets::Comparison { comparison }) = (human, &input.targets) {
		let page: PathCompareOutput = execute_query!(
			ctx,
			PathCompareInput {
				comparison: comparison.clone(),
				after: None,
				limit: 1,
			}
		);
		print_summary(&mut out, comparison, &page.totals.unwrap_or_default())?;
		writeln!(out)?;
	}

	let validation: Validation = execute_validate!(ctx, input.clone());
	let plan: FsPlan = execute_preview!(ctx, input.clone());
	if human {
		print_validation(&mut out, &validation)?;
		print_plan(&mut out, &plan)?;
	}
	if let Some(stop) = stop_before_dispatch(human, &validation, &plan, dry_run)? {
		return stop;
	}

	let count = plan.summary.deletes.files;
	if count == 0 {
		writeln!(out, "Nothing to delete")?;
		return Ok(());
	}
	let from = match &input.targets {
		DeleteTargets::Comparison { .. } => " from A",
		DeleteTargets::Paths { .. } => "",
	};
	let destination = if input.permanent {
		"permanently"
	} else {
		"to the trash"
	};
	confirm_or_abort(
		&format!(
			"Delete {count} files ({}){from} {destination}?",
			format_bytes(plan.summary.deletes.bytes)
		),
		yes,
	)?;

	let receipt: JobReceipt = execute_action!(ctx, input);
	if human {
		writeln!(out, "Dispatched delete job {}", receipt.id)?;
	} else {
		crate::util::output::print_json(&serde_json::json!({
			"validation": validation,
			"plan": plan,
			"job": receipt,
		}));
	}
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
		overlay: None,
	};

	// Execute the query using the core client
	let json_response = ctx.core.query(&input, ctx.library_id).await?;
	let result: sd_core::ops::files::query::DirectoryListingOutput =
		serde_json::from_value(json_response)?;

	Ok(result)
}
