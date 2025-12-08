pub mod args;

use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Table};

use crate::util::prelude::*;

use crate::{context::Context, util::error::CliError};
use sd_core::{infra::job::types::JobId, ops::libraries::list::query::ListLibrariesQuery};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
	/// Quick scan of a path (ephemeral)
	QuickScan(QuickScanArgs),
	/// Browse a path without adding as location
	Browse(BrowseArgs),
	/// Verify index integrity for a path
	Verify(IndexVerifyArgs),
	/// Show ephemeral index cache status
	EphemeralCache(EphemeralCacheArgs),
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

			let input = args.to_input(library_id)?;
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}

			let out: JobId = execute_action!(ctx, input);
			print_output!(ctx, out, |_| {
				println!("Indexing request submitted");
			});
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
		IndexCmd::Verify(args) => {
			let input = args.to_input();
			let out: sd_core::ops::indexing::verify::output::IndexVerifyOutput =
				execute_action!(ctx, input);

			print_output!(
				ctx,
				&out,
				|result: &sd_core::ops::indexing::verify::output::IndexVerifyOutput| {
					println!("\n╔══════════════════════════════════════════════════════════════╗");
					println!("║          INDEX INTEGRITY VERIFICATION REPORT                ║");
					println!("╠══════════════════════════════════════════════════════════════╣");
					println!(
						"║ Path: {:60} ║",
						result
							.path
							.display()
							.to_string()
							.chars()
							.take(60)
							.collect::<String>()
					);
					println!("║ Duration: {:.2}s {:49} ║", result.duration_secs, "");
					println!("╠══════════════════════════════════════════════════════════════╣");

					let report = &result.report;

					println!(
						"║ Filesystem: {} files, {} directories {:23} ║",
						report.filesystem_file_count, report.filesystem_dir_count, ""
					);
					println!(
						"║ Database:   {} files, {} directories {:23} ║",
						report.database_file_count, report.database_dir_count, ""
					);
					println!("╠══════════════════════════════════════════════════════════════╣");

					if result.is_valid {
						println!("║ STATUS: VALID - Index matches filesystem perfectly!      ║");
					} else {
						println!(
							"║ STATUS: DIVERGED - {} issues found {:24} ║",
							report.total_issues(),
							""
						);
						println!(
							"╠══════════════════════════════════════════════════════════════╣"
						);

						if !report.missing_from_index.is_empty() {
							println!(
								"║ ️  Missing from index: {} {:33} ║",
								report.missing_from_index.len(),
								""
							);
							if args.detailed {
								for diff in report.missing_from_index.iter().take(5) {
									let path_str = diff.path.display().to_string();
									if path_str.len() <= 58 {
										println!("║   - {:58} ║", path_str);
									} else {
										println!(
											"║   - ...{:55} ║",
											&path_str[path_str.len().saturating_sub(55)..]
										);
									}
								}
								if report.missing_from_index.len() > 5 {
									println!(
										"║   ... and {} more {:40} ║",
										report.missing_from_index.len() - 5,
										""
									);
								}
							}
						}

						if !report.stale_in_index.is_empty() {
							println!(
								"║ ️  Stale in index: {} {:36} ║",
								report.stale_in_index.len(),
								""
							);
							if args.detailed {
								for diff in report.stale_in_index.iter().take(5) {
									let path_str = diff.path.display().to_string();
									if path_str.len() <= 58 {
										println!("║   - {:58} ║", path_str);
									} else {
										println!(
											"║   - ...{:55} ║",
											&path_str[path_str.len().saturating_sub(55)..]
										);
									}
								}
								if report.stale_in_index.len() > 5 {
									println!(
										"║   ... and {} more {:40} ║",
										report.stale_in_index.len() - 5,
										""
									);
								}
							}
						}

						if !report.metadata_mismatches.is_empty() {
							println!(
								"║ ️  Metadata mismatches: {} {:31} ║",
								report.metadata_mismatches.len(),
								""
							);
							if args.detailed {
								for diff in &report.metadata_mismatches {
									println!(
										"║   - {:?}: {:?} -> {:?} {:20} ║",
										diff.issue_type,
										diff.expected.as_deref().unwrap_or("?"),
										diff.actual.as_deref().unwrap_or("?"),
										""
									);
								}
							}
						}

						if !report.hierarchy_errors.is_empty() {
							println!(
								"║ Hierarchy errors: {} {:34} ║",
								report.hierarchy_errors.len(),
								""
							);
						}
					}

					println!("╠══════════════════════════════════════════════════════════════╣");
					println!(
						"║ {}{:59} ║",
						if result.is_valid { "" } else { "" },
						report.summary.chars().take(59).collect::<String>()
					);
					println!("╚══════════════════════════════════════════════════════════════╝\n");
				}
			);
		}
		IndexCmd::EphemeralCache(args) => {
			let input = args.to_input();
			let out: sd_core::ops::core::ephemeral_status::EphemeralCacheStatus =
				execute_core_query!(ctx, input);

			print_output!(
				ctx,
				&out,
				|status: &sd_core::ops::core::ephemeral_status::EphemeralCacheStatus| {
					println!();
					println!("╔══════════════════════════════════════════════════════════════╗");
					println!("║           UNIFIED EPHEMERAL INDEX CACHE                      ║");
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
					stats_table.add_row(vec![
						"Content kinds",
						&stats.content_kinds.to_string(),
					]);
					stats_table.add_row(vec![
						"Memory usage",
						&format_bytes(stats.memory_bytes as u64),
					]);
					stats_table.add_row(vec!["Cache age", &format!("{:.1}s", stats.age_seconds)]);
					stats_table
						.add_row(vec!["Idle time", &format!("{:.1}s", stats.idle_seconds)]);

					println!("{}", stats_table);

					// Show indexed paths
					if status.indexed_paths.is_empty() && status.paths_in_progress.is_empty() {
						println!("\n  No paths indexed yet.");
					} else {
						// Paths in progress
						if !status.paths_in_progress.is_empty() {
							println!();
							let mut progress_table = Table::new();
							progress_table.load_preset(UTF8_BORDERS_ONLY);
							progress_table.set_header(vec![
								Cell::new("INDEXING IN PROGRESS").add_attribute(Attribute::Bold),
							]);
							for path in &status.paths_in_progress {
								progress_table.add_row(vec![format!(
									"● {}",
									path.display()
								)]);
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
