pub mod args;

use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Table};

use crate::util::prelude::*;

use crate::{context::Context, util::error::CliError};
use sd_core::{
	infra::job::types::JobId,
	ops::{
		indexing::startup::{
			StartupIndexingDisposition, StartupIndexingInput, StartupIndexingOutput,
		},
		libraries::list::query::ListLibrariesQuery,
	},
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
	/// Quick scan of a path
	QuickScan(QuickScanArgs),
	/// Browse a path without tracking it as a source
	Browse(BrowseArgs),
	/// Show volume index status
	Status(IndexStatusArgs),
	/// Reset the volume index
	Reset,
}

pub async fn run(ctx: &Context, cmd: IndexCmd) -> Result<()> {
	match cmd {
		IndexCmd::Start(args) => {
			let library_id = if let Some(id) = args.library {
				id
			} else {
				let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
					ctx,
					sd_core::ops::libraries::list::query::ListLibrariesInput {
						include_stats: false
					}
				);
				match libs.len() {
					0 => anyhow::bail!("No libraries found; specify --library after creating one"),
					1 => libs[0].id,
					_ => anyhow::bail!("Multiple libraries found; please specify --library <UUID>"),
				}
			};

			let action_ctx = ctx.clone().with_library_id(library_id);
			if args.defaults {
				if !args.paths.is_empty() {
					anyhow::bail!("--defaults cannot be combined with paths");
				}

				let out: StartupIndexingOutput =
					execute_action!(&action_ctx, StartupIndexingInput { force: true });
				print_output!(ctx, &out, |output: &StartupIndexingOutput| {
					match output.disposition {
						StartupIndexingDisposition::Started => {
							println!("Default filesystem discovery started")
						}
						StartupIndexingDisposition::AlreadyStarted => {
							println!("Default filesystem discovery is already running")
						}
						StartupIndexingDisposition::Disabled => {
							println!("Default filesystem discovery is disabled")
						}
					}
				});
			} else {
				let input = args.to_input(library_id)?;
				if let Err(errors) = input.validate() {
					anyhow::bail!(errors.join("; "));
				}

				let out: JobId = execute_action!(&action_ctx, input);
				print_output!(ctx, out, |_| {
					println!("Indexing request submitted");
				});
			}
		}
		IndexCmd::QuickScan(args) => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
				ctx,
				sd_core::ops::libraries::list::query::ListLibrariesInput {
					include_stats: false
				}
			);
			let library_id = match libs.len() {
				1 => libs[0].id,
				_ => {
					anyhow::bail!("Specify --library for quick-scan when multiple libraries exist")
				}
			};

			let input = args.to_input(library_id)?;
			let out: JobId = execute_action!(ctx, input);
			print_output!(ctx, out, |_| {
				println!("Quick scan request submitted");
			});
		}
		IndexCmd::Browse(args) => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
				ctx,
				sd_core::ops::libraries::list::query::ListLibrariesInput {
					include_stats: false
				}
			);
			let library_id = match libs.len() {
				1 => libs[0].id,
				_ => anyhow::bail!("Specify --library for browse when multiple libraries exist"),
			};

			let input = args.to_input(library_id)?;
			let out: JobId = execute_action!(ctx, input);
			print_output!(ctx, out, |_| {
				println!("Browse request submitted");
			});
		}
		IndexCmd::Status(args) => {
			let input = args.to_input();
			let out: sd_core::ops::core::index_status::IndexStatus =
				execute_core_query!(ctx, input);

			print_output!(
				ctx,
				&out,
				|status: &sd_core::ops::core::index_status::IndexStatus| {
					println!();
					println!("╔══════════════════════════════════════════════════════════════╗");
					println!("║           VOLUME INDEX                                       ║");
					println!("╠══════════════════════════════════════════════════════════════╣");
					println!(
						"║ Indexed Paths: {:3}    In Progress: {:3}                       ║",
						status.indexed_paths_count, status.indexing_in_progress_count
					);
					println!("╚══════════════════════════════════════════════════════════════╝");

					// Show unified index stats
					let stats = &status.index_stats;
					println!();
					let mut stats_table = Table::new();
					stats_table.load_preset(UTF8_BORDERS_ONLY);
					stats_table.set_header(vec![
						Cell::new("SHARED INDEX STATS").add_attribute(Attribute::Bold),
						Cell::new(""),
					]);

					stats_table.add_row(vec![
						"Total entries (shared arena)",
						&stats.total_entries.to_string(),
					]);
					stats_table.add_row(vec![
						"Path index count",
						&stats.path_index_count.to_string(),
					]);
					stats_table.add_row(vec![
						"Unique names (shared)",
						&stats.unique_names.to_string(),
					]);
					stats_table.add_row(vec![
						"Interned strings (shared)",
						&stats.interned_strings.to_string(),
					]);
					stats_table.add_row(vec!["Content kinds", &stats.content_kinds.to_string()]);
					stats_table.add_row(vec!["UUID count (lazy)", &stats.uuid_count.to_string()]);
					stats_table.add_row(vec![
						"Memory usage",
						&format_bytes(stats.memory_bytes as u64),
					]);
					stats_table.add_row(vec![
						"Total file size",
						&format_bytes(stats.total_file_bytes),
					]);
					stats_table.add_row(vec!["Cache age", &format!("{:.1}s", stats.age_seconds)]);
					stats_table.add_row(vec!["Idle time", &format!("{:.1}s", stats.idle_seconds)]);

					println!("{}", stats_table);

					// Show detailed memory breakdown if available
					if let Some(ref breakdown) = stats.memory_breakdown {
						println!();
						let mut breakdown_table = Table::new();
						breakdown_table.load_preset(UTF8_BORDERS_ONLY);
						breakdown_table.set_header(vec![
							Cell::new("MEMORY BREAKDOWN (DETAILED)").add_attribute(Attribute::Bold),
							Cell::new("Overhead"),
							Cell::new("Entries"),
							Cell::new("Total"),
						]);

						breakdown_table.add_row(vec![
							"Arena",
							"-",
							"-",
							&format_bytes(breakdown.arena as u64),
						]);
						breakdown_table.add_row(vec![
							"Cache (string interning)",
							"-",
							"-",
							&format_bytes(breakdown.cache as u64),
						]);
						breakdown_table.add_row(vec![
							"Registry (name search)",
							"-",
							"-",
							&format_bytes(breakdown.registry as u64),
						]);
						breakdown_table.add_row(vec![
							"path_index HashMap",
							&format_bytes(breakdown.path_index_overhead as u64),
							&format_bytes(breakdown.path_index_entries as u64),
							&format_bytes(
								(breakdown.path_index_overhead + breakdown.path_index_entries)
									as u64,
							),
						]);
						breakdown_table.add_row(vec![
							"entry_uuids HashMap",
							&format_bytes(breakdown.entry_uuids_overhead as u64),
							&format_bytes(breakdown.entry_uuids_entries as u64),
							&format_bytes(
								(breakdown.entry_uuids_overhead + breakdown.entry_uuids_entries)
									as u64,
							),
						]);
						breakdown_table.add_row(vec![
							"content_kinds HashMap",
							&format_bytes(breakdown.content_kinds_overhead as u64),
							&format_bytes(breakdown.content_kinds_entries as u64),
							&format_bytes(
								(breakdown.content_kinds_overhead + breakdown.content_kinds_entries)
									as u64,
							),
						]);

						let total = breakdown.arena
							+ breakdown.cache + breakdown.registry
							+ breakdown.path_index_overhead
							+ breakdown.path_index_entries
							+ breakdown.entry_uuids_overhead
							+ breakdown.entry_uuids_entries
							+ breakdown.content_kinds_overhead
							+ breakdown.content_kinds_entries;

						breakdown_table.add_row(vec![
							Cell::new("TOTAL").add_attribute(Attribute::Bold),
							Cell::new(""),
							Cell::new(""),
							Cell::new(&format_bytes(total as u64)).add_attribute(Attribute::Bold),
						]);

						println!("{}", breakdown_table);
						println!();
						println!("Note: 'Overhead' = HashMap control bytes (~1 byte/capacity)");
						println!("      'Entries' = Actual key+value data (len × entry_size + heap strings)");
					}

					// Show indexed paths
					if status.indexed_paths.is_empty() && status.paths_in_progress.is_empty() {
						println!("\n  No paths indexed yet.");
					} else {
						// Paths in progress
						if !status.paths_in_progress.is_empty() {
							println!();
							let mut progress_table = Table::new();
							progress_table.load_preset(UTF8_BORDERS_ONLY);
							progress_table
								.set_header(vec![Cell::new("INDEXING IN PROGRESS")
									.add_attribute(Attribute::Bold)]);
							for path in &status.paths_in_progress {
								progress_table.add_row(vec![format!("● {}", path.display())]);
							}
							println!("{}", progress_table);
						}

						// Indexed paths
						if !status.indexed_paths.is_empty() {
							println!();
							let mut paths_table = Table::new();
							paths_table.load_preset(UTF8_BORDERS_ONLY);
							paths_table.set_header(vec![
								Cell::new("INDEXED PATHS").add_attribute(Attribute::Bold),
								Cell::new("Children"),
							]);
							for info in &status.indexed_paths {
								paths_table.add_row(vec![
									format!("○ {}", info.path.display()),
									info.child_count.to_string(),
								]);
							}
							println!("{}", paths_table);
						}
					}

					// Watched roots. Printed even when empty, because empty is the
					// interesting answer: a restored index that nothing is watching
					// looks identical from the outside to a broken watcher.
					println!();
					let mut watched_table = Table::new();
					watched_table.load_preset(UTF8_BORDERS_ONLY);
					watched_table.set_header(vec![
						Cell::new("WATCHED ROOTS").add_attribute(Attribute::Bold)
					]);
					if status.watched_paths.is_empty() {
						watched_table.add_row(vec!["none — nothing is armed for events"]);
					} else {
						for path in &status.watched_paths {
							watched_table.add_row(vec![format!("● {}", path.display())]);
						}
					}
					for refusal in &status.refused_watches {
						watched_table.add_row(vec![format!(
							"○ {} (refused: {})",
							refusal.path.display(),
							refusal.reason
						)]);
					}
					println!("{}", watched_table);
					println!();
				}
			);
		}
		IndexCmd::Reset => {
			let input = sd_core::ops::core::index_status::IndexResetInput { confirm: true };
			let out: sd_core::ops::core::index_status::IndexResetOutput =
				execute_action!(ctx, input);

			print_output!(
				ctx,
				&out,
				|result: &sd_core::ops::core::index_status::IndexResetOutput| {
					println!();
					println!("╔══════════════════════════════════════════════════════════════╗");
					println!("║           VOLUME INDEX RESET                                 ║");
					println!("╠══════════════════════════════════════════════════════════════╣");
					println!("║ Cleared {} paths {:45} ║", result.cleared_paths, "");
					println!("╚══════════════════════════════════════════════════════════════╝");
					println!();
				}
			);
		}
	}
	Ok(())
}

fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_index = 0;

	while size >= 1024.0 && unit_index < UNITS.len() - 1 {
		size /= 1024.0;
		unit_index += 1;
	}

	if unit_index == 0 {
		format!("{} {}", bytes, UNITS[unit_index])
	} else {
		format!("{:.1} {}", size, UNITS[unit_index])
	}
}
