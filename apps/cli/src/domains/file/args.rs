use clap::Args;
use std::path::PathBuf;

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::files::copy::input::{CopyMethod, FileCopyInput},
};

#[derive(Args, Debug, Clone)]
pub struct FileCopyArgs {
	/// Source files or directories to copy (one or more)
	pub sources: Vec<PathBuf>,

	/// Destination path
	#[arg(long)]
	pub destination: PathBuf,

	/// Overwrite existing files
	#[arg(long, default_value_t = false)]
	pub overwrite: bool,

	/// Verify checksums during copy
	#[arg(long, default_value_t = false)]
	pub verify_checksum: bool,

	/// Preserve file timestamps
	#[arg(long, default_value_t = true)]
	pub preserve_timestamps: bool,

	/// Delete source files after copy (move)
	#[arg(long, default_value_t = false)]
	pub move_files: bool,

	/// Copy method to use
	#[arg(long, default_value_t = CopyMethod::Auto)]
	pub method: CopyMethod,
}

impl From<FileCopyArgs> for FileCopyInput {
	fn from(args: FileCopyArgs) -> Self {
		let sources = args
			.sources
			.iter()
			.map(|p| SdPath::local(p.clone()))
			.collect::<Vec<_>>();
		let destination = SdPath::local(args.destination);
		Self {
			sources: SdPathBatch { paths: sources },
			destination,
			overwrite: args.overwrite,
			verify_checksum: args.verify_checksum,
			preserve_timestamps: args.preserve_timestamps,
			move_files: args.move_files,
			copy_method: args.method,
			on_conflict: None,
		}
	}
}
