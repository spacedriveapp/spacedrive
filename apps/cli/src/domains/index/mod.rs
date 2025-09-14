pub mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::{context::Context, util::error::CliError};
use sd_core::{
    infra::job::types::JobId,
    ops::libraries::list::query::ListLibrariesQuery,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
	/// Quick scan of a path (ephemeral)
	QuickScan(QuickScanArgs),
	/// Browse a path without adding as location
	Browse(BrowseArgs),
}

pub async fn run(ctx: &Context, cmd: IndexCmd) -> Result<()> {
	match cmd {
		IndexCmd::Start(args) => {
			let library_id = if let Some(id) = args.library {
				id
			} else {
				let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> =
					execute_query!(ctx, ListLibrariesQuery::basic());
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
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
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
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
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
	}
	Ok(())
}
