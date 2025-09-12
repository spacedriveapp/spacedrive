use anyhow::Result;
use clap::Subcommand;

use crate::context::{Context, OutputFormat};
use crate::util::confirm::confirm_or_abort;

#[derive(clap::Parser, Debug, Clone)]
pub struct FileCopyArgs {
	/// Source files or directories to copy (one or more)
	pub sources: Vec<std::path::PathBuf>,

	/// Destination path
	#[arg(long)]
	pub destination: std::path::PathBuf,

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

	/// Assume yes to all confirmations (dangerous operations)
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileCopyArgs {
	pub fn to_input(&self) -> sd_core::ops::files::copy::input::FileCopyInput {
		use sd_core::ops::files::copy::input::{CopyMethod, FileCopyInput};
		FileCopyInput {
			library_id: None,
			sources: self.sources.clone(),
			destination: self.destination.clone(),
			overwrite: self.overwrite,
			verify_checksum: self.verify_checksum,
			preserve_timestamps: self.preserve_timestamps,
			move_files: self.move_files,
			copy_method: CopyMethod::Auto,
		}
	}
}

#[derive(Subcommand, Debug)]
pub enum FileCmd {
	/// Copy files
	Copy(FileCopyArgs),
}

pub async fn run(ctx: &Context, cmd: FileCmd) -> Result<()> {
	match cmd {
		FileCmd::Copy(args) => {
			let input = args.to_input();
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "))
			}
			if args.move_files {
				confirm_or_abort("This will MOVE files (delete sources after copy). Continue?", args.yes)?;
			}
			let _bytes = ctx.core.action(&input).await?;
			println!("Copy request submitted");
		}
	}
	Ok(())
}
