use anyhow::Result;
use clap::Subcommand;
use uuid::Uuid;

use crate::context::Context;
use crate::domains::index::IndexModeArg;
use sd_core::ops::{
	libraries::list::{output::LibraryInfo, query::ListLibrariesQuery},
	locations::add::action::LocationAddInput,
};

#[derive(Subcommand, Debug)]
pub enum LocationCmd {
	Add {
		path: std::path::PathBuf,
		#[arg(long)]
		name: Option<String>,
		#[arg(long, value_enum, default_value = "content")]
		mode: IndexModeArg,
	},
	List,
	Remove {
		location_id: Uuid,
	},
	Rescan {
		location_id: Uuid,
		#[arg(long, default_value_t = false)]
		force: bool,
	},
}

pub async fn run(ctx: &Context, cmd: LocationCmd) -> Result<()> {
	match cmd {
		LocationCmd::Add { path, name, mode } => {
			let result_bytes = ctx
				.core
				.action(&LocationAddInput {
					path,
					name,
					mode: sd_core::ops::indexing::job::IndexMode::from(mode),
				})
				.await?;
			let out: sd_core::ops::locations::add::output::LocationAddOutput =
				bincode::deserialize(&result_bytes)?;
			println!(
				"Added location {} -> {}",
				out.location_id,
				out.path.display()
			);
		}
		LocationCmd::List => {
			let out: sd_core::ops::locations::list::output::LocationsListOutput = ctx
				.core
				.query(&sd_core::ops::locations::list::query::LocationsListQuery { library_id })
				.await?;
			for loc in out.locations {
				println!("- {} {}", loc.id, loc.path.display());
			}
		}
		LocationCmd::Remove { location_id } => {
			let libs: Vec<LibraryInfo> = ctx.core.query(&ListLibrariesQuery::basic()).await?;
			let library_id = if libs.len() == 1 {
				libs[0].id
			} else {
				anyhow::bail!("Specify --library to remove locations when multiple libraries exist")
			};

			let result_bytes = ctx
				.core
				.action(
					&sd_core::ops::locations::remove::action::LocationRemoveInput {
						library_id,
						location_id,
					},
				)
				.await?;
			let _out: sd_core::ops::locations::remove::output::LocationRemoveOutput =
				bincode::deserialize(&result_bytes)?;
			println!("Removed location {}", location_id);
		}
		LocationCmd::Rescan { location_id, force } => {
			let result_bytes = ctx
				.core
				.action(
					&sd_core::ops::locations::rescan::action::LocationRescanInput {
						location_id,
						full_rescan: force,
					},
				)
				.await?;
			let _out: sd_core::ops::locations::rescan::output::LocationRescanOutput =
				bincode::deserialize(&result_bytes)?;
			println!("Rescan requested for {}", location_id);
		}
	}
	Ok(())
}
