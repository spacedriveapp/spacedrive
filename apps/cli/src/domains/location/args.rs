use clap::Args;
use std::path::PathBuf;
use uuid::Uuid;

use sd_core::ops::{
	indexing::job::IndexMode,
	locations::{
		add::action::LocationAddInput, remove::action::LocationRemoveInput,
		rescan::action::LocationRescanInput,
	},
};

use crate::domains::index::args::IndexModeArg;

#[derive(Args, Debug)]
pub struct LocationAddArgs {
	pub path: PathBuf,
	#[arg(long)]
	pub name: Option<String>,
	#[arg(long, value_enum, default_value = "content")]
	pub mode: IndexModeArg,
}

impl From<LocationAddArgs> for LocationAddInput {
	fn from(args: LocationAddArgs) -> Self {
		Self {
			path: args.path,
			name: args.name,
			mode: IndexMode::from(args.mode),
		}
	}
}

#[derive(Args, Debug)]
pub struct LocationRemoveArgs {
	pub location_id: Uuid,
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl From<LocationRemoveArgs> for LocationRemoveInput {
	fn from(args: LocationRemoveArgs) -> Self {
		Self {
			location_id: args.location_id,
		}
	}
}

#[derive(Args, Debug)]
pub struct LocationRescanArgs {
	pub location_id: Uuid,
	#[arg(long, default_value_t = false)]
	pub force: bool,
}

impl From<LocationRescanArgs> for LocationRescanInput {
	fn from(args: LocationRescanArgs) -> Self {
		Self {
			location_id: args.location_id,
			full_rescan: args.force,
		}
	}
}

