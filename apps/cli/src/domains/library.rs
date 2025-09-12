use anyhow::Result;
use clap::Subcommand;

use crate::context::{Context, OutputFormat};
use crate::util::output::print_json;
use sd_core::ops::libraries::create;
use sd_core::ops::libraries::session::set_current;

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
	/// Create a new library
	Create { name: String },
	/// Switch to a different library
	Switch { id: uuid::Uuid },
	/// List libraries
	List,
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
	match cmd {
		LibraryCmd::Create { name } => {
			let input = create::LibraryCreateInput::new(name);
			let bytes = ctx.core.action(&input).await?;
			let out: create::LibraryCreateOutput = bincode::deserialize(&bytes)?;
			println!("Created library {} with ID {} at {}", out.name, out.library_id, out.path.display());
		}
		LibraryCmd::Switch { id } => {
			let input = set_current::SetCurrentLibraryInput { library_id: id };
			let bytes = ctx.core.action(&input).await?;
			let out: set_current::SetCurrentLibraryOutput = bincode::deserialize(&bytes)?;
			if out.success {
				println!("Switched to library {}", id);
			} else {
				println!("Failed to switch to library {}", id);
			}
		}
		LibraryCmd::List => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			match ctx.format {
				OutputFormat::Human => {
					if libs.is_empty() {
						println!("No libraries found");
					}
					for l in libs {
						println!("- {} {}", l.id, l.path.display());
					}
				}
				OutputFormat::Json => print_json(&libs),
			}
		}
	}
	Ok(())
}
