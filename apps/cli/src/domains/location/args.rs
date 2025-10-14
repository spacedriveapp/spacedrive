use clap::Args;
use std::path::PathBuf;
use uuid::Uuid;

use sd_core::{
	domain::addressing::SdPath,
	ops::{
		indexing::job::IndexMode,
		locations::{
			add::action::LocationAddInput, remove::action::LocationRemoveInput,
			rescan::action::LocationRescanInput,
		},
	},
};

use crate::domains::index::args::IndexModeArg;

#[derive(Args, Debug)]
pub struct LocationAddArgs {
	/// Path to add (local filesystem path or cloud path)
	/// If not provided, enters interactive mode
	pub path: Option<String>,

	/// Cloud volume fingerprint (if adding a cloud location)
	#[arg(long)]
	pub cloud: Option<String>,

	/// Display name for the location
	#[arg(long)]
	pub name: Option<String>,

	/// Indexing mode
	#[arg(long, value_enum)]
	pub mode: Option<IndexModeArg>,
}

impl LocationAddArgs {
	/// Build an SdPath from the args (non-interactive mode)
	pub fn build_sd_path(&self) -> anyhow::Result<SdPath> {
		let path_str = self.path.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Path is required in non-interactive mode"))?;

		if let Some(volume_fingerprint_str) = &self.cloud {
			// Cloud path
			let volume_fingerprint = sd_core::volume::VolumeFingerprint(volume_fingerprint_str.clone());
			Ok(SdPath::cloud(volume_fingerprint, path_str.clone()))
		} else {
			// Local path
			let path_buf = PathBuf::from(path_str);
			Ok(SdPath::local(path_buf))
		}
	}

	/// Check if interactive mode should be triggered
	pub fn is_interactive(&self) -> bool {
		self.path.is_none()
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
