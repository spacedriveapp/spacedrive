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
	/// Path to add (local filesystem path or service-based cloud URI)
	/// Examples:
	///   - /Users/james/Documents (local path)
	///   - s3://my-bucket/photos (S3 cloud path)
	///   - gdrive://My Drive/photos (Google Drive path)
	/// If not provided, enters interactive mode
	pub path: Option<String>,

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
		let path_str = self
			.path
			.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Path is required in non-interactive mode"))?;

		// Use SdPath::from_uri() to parse service-based paths or local paths
		SdPath::from_uri(path_str).map_err(|e| anyhow::anyhow!("Invalid path: {}", e))
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
