use anyhow::Result;
use clap::Subcommand;
use uuid::Uuid;

use crate::context::Context;
use crate::util::confirm::confirm_or_abort;
use crate::domains::index::IndexModeArg;

#[derive(Subcommand, Debug)]
pub enum LocationCmd {
	Add { path: std::path::PathBuf, #[arg(long)] name: Option<String>, #[arg(long, value_enum, default_value = "content")] mode: IndexModeArg },
	List,
	Remove { location_id: Uuid, #[arg(long, short = 'y', default_value_t = false)] yes: bool },
	Rescan { location_id: Uuid, #[arg(long, default_value_t = false)] force: bool },
}

pub async fn run(ctx: &Context, cmd: LocationCmd) -> Result<()> {
	match cmd {
		LocationCmd::Add { path, name, mode } => {
			let out: sd_core::ops::locations::add::output::LocationAddOutput = ctx.core.action(&sd_core::ops::locations::add::action::LocationAddInput { path, name, mode: sd_core::ops::indexing::job::IndexMode::from(mode) }).await?;
			println!("Added location {} -> {}", out.id, out.path.display());
		}
		LocationCmd::List => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library to list locations when multiple libraries exist") };
			let out: sd_core::ops::locations::list::output::LocationsListOutput = ctx.core.query(&sd_core::ops::locations::list::query::LocationsListQuery { library_id }).await?;
			for loc in out.locations { println!("- {} {}", loc.id, loc.path.display()); }
		}
		LocationCmd::Remove { location_id, yes } => {
			confirm_or_abort(&format!("This will remove location {} from the library. Continue?", location_id), yes)?;
			let _out: sd_core::ops::locations::remove::output::LocationRemoveOutput = ctx.core.action(&sd_core::ops::locations::remove::action::LocationRemoveInput { location_id }).await?;
			println!("Removed location {}", location_id);
		}
		LocationCmd::Rescan { location_id, force: _ } => {
			let _out: sd_core::ops::locations::rescan::output::LocationRescanOutput = ctx.core.action(&sd_core::ops::locations::rescan::action::LocationRescanInput { location_id }).await?;
			println!("Rescan requested for {}", location_id);
		}
	}
	Ok(())
}

