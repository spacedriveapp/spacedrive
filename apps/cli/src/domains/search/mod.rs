mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::search::{grep::GrepOutput, output::FileSearchOutput, query::FileSearchQuery};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum SearchCmd {
	/// Search for files
	Files(FileSearchArgs),
	/// Search file contents under a path, live, with no index
	Grep(GrepArgs),
}

pub async fn run(ctx: &Context, cmd: SearchCmd) -> Result<()> {
	match cmd {
		SearchCmd::Grep(args) => {
			let path = args.path.clone().unwrap_or_else(|| ".".into());
			let path = crate::domains::file::requested_path(ctx, &path)?;
			let input = args.into_input(path);
			let out: GrepOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |o: &GrepOutput| {
				let context = o
					.matches
					.iter()
					.any(|m| !m.before.is_empty() || !m.after.is_empty());
				for (i, m) in o.matches.iter().enumerate() {
					if context && i > 0 {
						println!("--");
					}
					let first = m.line_number.saturating_sub(m.before.len() as u64);
					for (n, line) in m.before.iter().enumerate() {
						println!("{}-{}-{}", m.path, first + n as u64, line);
					}
					println!("{}:{}:{}", m.path, m.line_number, m.line);
					for (n, line) in m.after.iter().enumerate() {
						println!("{}-{}-{}", m.path, m.line_number + 1 + n as u64, line);
					}
				}
				eprintln!(
					"{} matches, {} files searched, {} skipped (binary or over the size cap)",
					o.matches.len(),
					o.files_scanned,
					o.files_skipped
				);
				if let Some(cursor) = &o.next_cursor {
					eprintln!("More matches: --cursor {cursor}");
				}
			});
		}
		SearchCmd::Files(mut args) => {
			if let Some(scope) = args.sd_path.as_mut().filter(|scope| !scope.contains("://")) {
				let path = crate::domains::file::requested_path(ctx, std::path::Path::new(scope))?;
				*scope = path.to_string_lossy().into_owned();
			}
			let input: sd_core::ops::search::input::FileSearchInput = args.into();
			let mut out: FileSearchOutput = execute_query!(ctx, input);
			// The index answers in the volume's spelling. On this machine a
			// result reads the way its source was tracked, through the firmlink.
			if ctx.core.device().is_none() {
				for result in &mut out.results {
					if let sd_core::domain::SdPath::Physical { path, .. } = &mut result.file.sd_path
					{
						*path = sd_fs_watcher::short_spelling(path);
					}
				}
			}
			print_output!(ctx, &out, |o: &FileSearchOutput| {
				if o.results.is_empty() {
					println!("No files found");
					return;
				}

				println!("Found {} files ({} total)", o.results.len(), o.total_found);
				println!("Search ID: {}", o.search_id);
				println!("Execution time: {}ms", o.execution_time_ms);
				println!();

				for (i, result) in o.results.iter().enumerate() {
					println!(
						"{}. {} (score: {:.2})",
						i + 1,
						result.file.name,
						result.score
					);

					if let Some(extension) = result.file.extension.as_deref() {
						println!("   Extension: {}", extension);
					}

					println!("   Size: {} bytes", result.file.size);
					println!(
						"   Modified: {}",
						result.file.modified_at.format("%Y-%m-%d %H:%M:%S")
					);
					println!("   Path: {}", result.file.sd_path.display());

					if !result.highlights.is_empty() {
						println!("   Highlights:");
						for highlight in &result.highlights {
							println!("     {}: {}", highlight.field, highlight.text);
						}
					}

					if let Some(content) = &result.matched_content {
						println!("   Matched content: {}", content);
					}

					println!();
				}

				// Show facets if available
				if !o.facets.file_types.is_empty() {
					println!("File types:");
					for (file_type, count) in &o.facets.file_types {
						println!("  {}: {}", file_type, count);
					}
					println!();
				}

				if !o.suggestions.is_empty() {
					println!("Suggestions:");
					for suggestion in &o.suggestions {
						println!("  {}", suggestion);
					}
				}
			});
		}
	}
	Ok(())
}
