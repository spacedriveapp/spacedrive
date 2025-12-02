use crate::config::CliConfig;
use crate::util::prelude::*;
use anyhow::Result;
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
	pub socket_addr: String,
	pub library_id: Option<Uuid>,
	pub cli_config: CliConfig,
}

impl Context {
	pub fn new(
		core: CoreClient,
		format: OutputFormat,
		data_dir: PathBuf,
		socket_addr: String,
	) -> Result<Self> {
		let cli_config = CliConfig::load(&data_dir)?;
		let library_id = cli_config.current_library_id;

		Ok(Self {
			core,
			format,
			data_dir,
			socket_addr,
			library_id,
			cli_config,
		})
	}

	pub fn with_library_id(mut self, library_id: Uuid) -> Self {
		self.library_id = Some(library_id);
		self
	}

	pub fn set_library_id(&mut self, library_id: Uuid) -> Result<()> {
		self.library_id = Some(library_id);
		self.cli_config
			.set_current_library(library_id, &self.data_dir)
	}

	/// Get the current library ID
	pub fn get_current_library_id(&self) -> Option<Uuid> {
		self.library_id
	}

	/// Set the current library by ID
	pub fn set_current_library(&mut self, library_id: Uuid) -> Result<()> {
		self.library_id = Some(library_id);
		self.cli_config
			.set_current_library(library_id, &self.data_dir)
	}

	/// Clear the current library
	pub fn clear_current_library(&mut self) -> Result<()> {
		self.library_id = None;
		self.cli_config.clear_current_library(&self.data_dir)
	}

	/// Switch to a library by ID
	pub fn switch_to_library(&mut self, library_id: Uuid) -> Result<()> {
		self.library_id = Some(library_id);
		self.cli_config
			.set_current_library(library_id, &self.data_dir)
	}

	/// Switch to a library by name
	pub async fn switch_to_library_named(&mut self, name: &str) -> Result<()> {
		let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
			self,
			sd_core::ops::libraries::list::query::ListLibrariesInput {
				include_stats: false
			}
		);

		if let Some(lib) = libs.iter().find(|lib| lib.name == name) {
			self.library_id = Some(lib.id);
			self.cli_config
				.set_current_library(lib.id, &self.data_dir)?;
			Ok(())
		} else {
			anyhow::bail!("Library '{}' not found", name)
		}
	}

	/// Get current library info
	pub async fn get_current_library_info(
		&self,
	) -> Result<Option<sd_core::ops::libraries::info::output::LibraryInfoOutput>> {
		if let Some(_library_id) = self.library_id {
			let input = sd_core::ops::libraries::info::query::LibraryInfoQueryInput {};
			let info: sd_core::ops::libraries::info::output::LibraryInfoOutput =
				execute_query!(self, input);
			Ok(Some(info))
		} else {
			Ok(None)
		}
	}

	/// Require a current library (error if none set)
	pub fn require_current_library(&self) -> Result<Uuid> {
		self.library_id.ok_or_else(|| {
			anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library.")
		})
	}

	/// Get current library ID or throw error
	pub fn get_current_library_id_or_throw(&self) -> Result<Uuid> {
		self.require_current_library()
	}

	/// Create a new library and optionally switch to it
	pub async fn create_and_switch_to_library(
		&mut self,
		name: String,
		path: PathBuf,
		set_as_current: bool,
	) -> Result<Uuid> {
		let input = sd_core::ops::libraries::create::input::LibraryCreateInput {
			name: name.clone(),
			path: Some(path),
		};
		let output: sd_core::ops::libraries::create::output::LibraryCreateOutput =
			execute_action!(self, input);

		if set_as_current {
			self.library_id = Some(output.library_id);
		}

		Ok(output.library_id)
	}

	/// Get current library jobs with optional status filter
	pub async fn get_current_library_jobs(
		&self,
		status: Option<sd_core::infra::job::types::JobStatus>,
	) -> Result<Vec<sd_core::ops::jobs::list::output::JobListItem>> {
		let library_id = self.require_current_library()?;
		let input = sd_core::ops::jobs::list::query::JobListInput { status };
		let output: sd_core::ops::jobs::list::output::JobListOutput = execute_query!(self, input);
		Ok(output.jobs)
	}

	/// Get current library status
	pub async fn get_current_library_status(
		&self,
	) -> Result<Option<sd_core::ops::libraries::info::output::LibraryInfoOutput>> {
		self.get_current_library_info().await
	}

	/// Check if there's an active library
	pub fn has_active_library(&self) -> bool {
		self.library_id.is_some()
	}

	/// List all available libraries
	pub async fn list_libraries(
		&self,
	) -> Result<Vec<sd_core::ops::libraries::list::output::LibraryInfo>> {
		let input = sd_core::ops::libraries::list::query::ListLibrariesInput {
			include_stats: false,
		};
		let output: Vec<sd_core::ops::libraries::list::output::LibraryInfo> =
			execute_core_query!(self, input);
		Ok(output)
	}

	/// Validate and fix the current library selection
	/// If the stored library ID is not found, select the first available library
	pub async fn validate_and_fix_library(&mut self) -> Result<()> {
		if let Some(stored_library_id) = self.library_id {
			let libraries = self.list_libraries().await?;

			if libraries.is_empty() {
				self.library_id = None;
				self.cli_config.clear_current_library(&self.data_dir)?;
				return Ok(());
			}

			let library_exists = libraries.iter().any(|lib| lib.id == stored_library_id);

			if !library_exists {
				if let Some(first_lib) = libraries.first() {
					self.library_id = Some(first_lib.id);
					self.cli_config
						.set_current_library(first_lib.id, &self.data_dir)?;
				}
			}
		} else {
			let libraries = self.list_libraries().await?;
			if let Some(first_lib) = libraries.first() {
				self.library_id = Some(first_lib.id);
				self.cli_config
					.set_current_library(first_lib.id, &self.data_dir)?;
			}
		}

		Ok(())
	}
}
