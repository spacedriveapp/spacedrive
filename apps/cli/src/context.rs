use sd_core::client::CoreClient;
use std::path::PathBuf;
use uuid::Uuid;

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
	pub library_id: Option<Uuid>,
}

impl Context {
	pub fn new(
		core: CoreClient,
		format: OutputFormat,
		data_dir: PathBuf,
		socket_path: PathBuf,
	) -> Self {
		Self {
			core,
			format,
			data_dir,
			socket_path,
			library_id: None,
		}
	}

	pub fn with_library_id(mut self, library_id: Uuid) -> Self {
		self.library_id = Some(library_id);
		self
	}

	pub fn set_library_id(&mut self, library_id: Uuid) {
		self.library_id = Some(library_id);
	}
}
