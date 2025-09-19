use sd_core::client::CoreClient;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
	Human,
	Json,
}

#[derive(Clone)]
pub struct Context {
	pub core: CoreClient,
	pub format: OutputFormat,
	pub data_dir: PathBuf,
	pub socket_path: PathBuf,
}

impl Context {
	pub fn new(core: CoreClient, format: OutputFormat, data_dir: PathBuf, socket_path: PathBuf) -> Self {
		Self { core, format, data_dir, socket_path }
	}
}
