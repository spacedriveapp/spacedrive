//! Generate thumbnails using the same path-scoped operation as the explorer.

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use sd_core::{
	domain::SdPath, infra::job::handle::JobReceipt, ops::thumbs::generate::ThumbnailGenerateInput,
	service::thumbs::ThumbnailGenerationMode,
};

use crate::{context::Context, util::prelude::*};

#[derive(Debug, Subcommand)]
pub enum ThumbsCmd {
	/// Generate thumbnails for a file or the indexed files in a folder
	Generate(GenerateArgs),
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
	pub path: String,
	/// Include indexed subdirectories
	#[arg(long, short)]
	pub recursive: bool,
	#[arg(long, value_enum, default_value = "stale")]
	pub mode: Mode,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Mode {
	Missing,
	Stale,
	Force,
}

pub async fn run(ctx: &Context, command: ThumbsCmd) -> Result<()> {
	let ThumbsCmd::Generate(args) = command;
	let scope = if args.path.contains("://") {
		SdPath::from_uri(&args.path)?
	} else {
		let path = std::path::PathBuf::from(&args.path);
		if ctx.core.device().is_some() && !path.is_absolute() {
			anyhow::bail!("Use an absolute path when targeting another device");
		}
		let path = if path.is_absolute() {
			path
		} else {
			std::env::current_dir()?.join(path)
		};
		SdPath::Physical {
			device_slug: "local".into(),
			path,
		}
	};
	let receipt: JobReceipt = execute_action!(
		ctx,
		ThumbnailGenerateInput {
			scope,
			recursive: args.recursive,
			mode: match args.mode {
				Mode::Missing => ThumbnailGenerationMode::Missing,
				Mode::Stale => ThumbnailGenerationMode::Stale,
				Mode::Force => ThumbnailGenerationMode::Force,
			},
		}
	);
	crate::util::output::print_json(&receipt);
	Ok(())
}
