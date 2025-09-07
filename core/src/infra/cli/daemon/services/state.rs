//! State management service for the daemon

use crate::infra::cli::state::CliState;
use crate::library::Library;
use crate::Core;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Service for managing CLI state
pub struct StateService {
	cli_state: Arc<RwLock<CliState>>,
	data_dir: PathBuf,
}

impl StateService {
	/// Create a new state service
	pub fn new(cli_state: Arc<RwLock<CliState>>, data_dir: PathBuf) -> Self {
		Self {
			cli_state,
			data_dir,
		}
	}

	/// Get the current library from CLI state
	pub async fn get_current_library(&self, core: &Arc<Core>) -> Option<Arc<Library>> {
		let state = self.cli_state.read().await;
		if let Some(current_id) = state.current_library_id {
			core.libraries.get_library(current_id).await
		} else {
			// Fallback to first library if no current library is set
			let libraries = core.libraries.list().await;
			libraries.first().cloned()
		}
	}

	/// Switch to a different library
	pub async fn switch_library(
		&self,
		library_id: Uuid,
		library_path: PathBuf,
	) -> Result<(), Box<dyn std::error::Error>> {
		let mut state = self.cli_state.write().await;
		state.set_current_library(library_id, library_path);

		// Save state to disk
		state.save(&self.data_dir)?;
		Ok(())
	}

	/// Get the current library ID
	pub async fn get_current_library_id(&self) -> Option<Uuid> {
		let state = self.cli_state.read().await;
		state.current_library_id
	}

	/// Auto-select first library if none is set
	pub async fn auto_select_library(
		&self,
		core: &Arc<Core>,
	) -> Result<(), Box<dyn std::error::Error>> {
		let mut state = self.cli_state.write().await;

		if state.current_library_id.is_none() {
			let libraries = core.libraries.list().await;
			if let Some(first_lib) = libraries.first() {
				state.set_current_library(first_lib.id(), first_lib.path().to_path_buf());
				state.save(&self.data_dir)?;
			}
		}

		Ok(())
	}
}
