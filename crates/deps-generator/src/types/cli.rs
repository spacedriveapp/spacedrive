use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Arguments {
	#[command(subcommand)]
	pub action: Action,
}

#[derive(Subcommand)]
pub enum Action {
	Frontend(FrontendArgs),
	Backend(BackendArgs),
}

#[derive(Args)]
pub struct FrontendArgs {
	// could source this from `$GITHUB_SHA` if not set
	#[arg(help = "the git revision")]
	pub revision: String,
	#[arg(help = "the output path")]
	pub path: PathBuf,
}

#[derive(Args)]
pub struct BackendArgs {
	// could use `Cargo.toml` as the default from current dir (if not set)
	#[arg(help = "path to the cargo manifest")]
	pub manifest_path: PathBuf,
	#[arg(help = "the output path")]
	pub output_path: PathBuf,
}
