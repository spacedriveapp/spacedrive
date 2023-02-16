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
	#[arg(help = "the git revision")]
	pub revision: String,
	#[arg(help = "the output path")]
	pub path: PathBuf,
}

#[derive(Args)]
pub struct BackendArgs {
	#[arg(help = "path to the cargo manifest")]
	pub manifest_path: PathBuf,
	#[arg(help = "the output path")]
	pub output_path: PathBuf,
}
