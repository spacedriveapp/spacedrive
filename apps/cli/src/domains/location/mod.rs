mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::locations::{
	add::{action::LocationAddInput, output::LocationAddOutput},
	list::{output::LocationsListOutput, query::LocationsListQueryInput},
	remove::output::LocationRemoveOutput,
	rescan::output::LocationRescanOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum LocationCmd {
	/// Add a new location to the library
	Add(LocationAddArgs),
	/// List all locations in the library
	List,
	/// Remove a location from the library
	Remove(LocationRemoveArgs),
	/// Rescan a location
	Rescan(LocationRescanArgs),
}

pub async fn run(ctx: &Context, cmd: LocationCmd) -> Result<()> {
	match cmd {
		LocationCmd::Add(args) => {
			let out: LocationAddOutput = execute_action!(ctx, LocationAddInput::from(args));
			print_output!(ctx, &out, |o: &LocationAddOutput| {
				println!("Added location {} -> {}", o.location_id, o.path.display());
			});
		}
		LocationCmd::List => {
			let out: sd_core::ops::locations::list::output::LocationsListOutput =
				execute_query!(ctx, LocationsListQueryInput {});
			print_output!(ctx, &out, |o: &LocationsListOutput| {
				if o.locations.is_empty() {
					println!("No locations found");
					return;
				}
				for loc in &o.locations {
					println!("- {} {}", loc.id, loc.path.display());
				}
			});
		}
		LocationCmd::Remove(args) => {
			confirm_or_abort(
				&format!(
					"This will remove location {} from the library. Continue?",
					args.location_id
				),
				args.yes,
			)?;
			let input: sd_core::ops::locations::remove::action::LocationRemoveInput = args.into();
			let out: LocationRemoveOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LocationRemoveOutput| {
				println!("Removed location {}", o.location_id);
			});
		}
		LocationCmd::Rescan(args) => {
			let input: sd_core::ops::locations::rescan::action::LocationRescanInput = args.into();
			let out: LocationRescanOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LocationRescanOutput| {
				println!("Rescan requested for {}", o.location_id);
			});
		}
	}
	Ok(())
}
