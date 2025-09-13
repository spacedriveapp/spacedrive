use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::Core;
use crate::infra::daemon::state::SessionStateService;

/// Validate instance name to prevent path traversal attacks
pub fn validate_instance_name(instance: &str) -> Result<(), String> {
	if instance.is_empty() {
		return Err("Instance name cannot be empty".to_string());
	}
	if instance.len() > 64 {
		return Err("Instance name too long (max 64 characters)".to_string());
	}
	if !instance.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
		return Err("Instance name contains invalid characters. Only alphanumeric, dash, and underscore allowed".to_string());
	}
	Ok(())
}

/// Manages lifecycle of Core instances (by name)
pub struct CoreInstanceManager {
	instances: Arc<RwLock<HashMap<String, Arc<Core>>>>,
	default_data_dir: PathBuf,
	enable_networking: bool,
	session_state: Arc<SessionStateService>,
}

impl CoreInstanceManager {
	pub fn new(default_data_dir: PathBuf, enable_networking: bool, session_state: Arc<SessionStateService>) -> Self {
		Self {
			instances: Arc::new(RwLock::new(HashMap::new())),
			default_data_dir,
			enable_networking,
			session_state,
		}
	}

	/// Get or start the default instance
	pub async fn get_default(&self) -> Result<Arc<Core>, String> {
		self.get_or_start("default".to_string(), None).await
	}

	/// Get or start a named instance, optionally with a specific data_dir
	pub async fn get_or_start(
		&self,
		name: String,
		data_dir: Option<PathBuf>,
	) -> Result<Arc<Core>, String> {
		// Validate instance name for security
		validate_instance_name(&name)?;

		// Use entry API to avoid race conditions
		use std::collections::hash_map::Entry;

		let mut instances = self.instances.write().await;
		let entry = instances.entry(name.clone());

		match entry {
			Entry::Occupied(existing) => {
				// Instance already exists, return it
				Ok(existing.get().clone())
			}
			Entry::Vacant(vacant) => {
				// Instance doesn't exist, create it
				let data_dir = data_dir.unwrap_or_else(|| self.default_data_dir.clone());
				let core = Arc::new(
					Core::new_with_config(data_dir, self.session_state.clone())
						.await
						.map_err(|e| format!("Failed to create core: {}", e))?
				);

				let core_with_networking = if self.enable_networking {
					Core::init_networking_shared(core.clone(), self.session_state.clone())
						.await
						.map_err(|e| format!("Failed to initialize networking: {}", e))?
				} else {
					core
				};

				// Insert and return the new instance
				vacant.insert(core_with_networking.clone());
				Ok(core_with_networking)
			}
		}
	}

	/// Shutdown a named instance
	pub async fn shutdown(&self, name: &str) -> Result<(), String> {
		// Validate instance name for security
		validate_instance_name(name)?;

		if let Some(core) = self.instances.write().await.remove(name) {
			core.shutdown().await.map_err(|e| format!("Shutdown failed: {}", e))?;
		}
		Ok(())
	}
}


