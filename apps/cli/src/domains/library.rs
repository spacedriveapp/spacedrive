use anyhow::Result;
use clap::Subcommand;

use crate::context::{Context, OutputFormat};
use crate::util::output::{print_human_line, print_json};

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
	/// List libraries
	List,
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
	match cmd {
		LibraryCmd::List => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			match ctx.format {
				OutputFormat::Human => {
					if libs.is_empty() {
						print_human_line("No libraries found");
					}
					for l in libs {
						print_human_line(&format!("- {} {}", l.id, l.path.display()));
					}
				}
				OutputFormat::Json => print_json(&libs),
			}
		}
	}
	Ok(())
}
