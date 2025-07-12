//! Daemon configuration

use std::path::PathBuf;

/// Daemon configuration
pub struct DaemonConfig {
	pub socket_path: PathBuf,
	pub pid_file: PathBuf,
	pub log_file: Option<PathBuf>,
	pub instance_name: Option<String>,
}

impl Default for DaemonConfig {
	fn default() -> Self {
		Self::new(None)
	}
}

impl DaemonConfig {
	/// Create a new daemon config with optional instance name
	pub fn new(instance_name: Option<String>) -> Self {
		let runtime_dir = dirs::runtime_dir()
			.or_else(|| dirs::cache_dir())
			.unwrap_or_else(|| PathBuf::from("/tmp"));

		let (socket_name, pid_name, log_name) = if let Some(ref name) = instance_name {
			(
				format!("spacedrive-{}.sock", name),
				format!("spacedrive-{}.pid", name),
				format!("spacedrive-{}.log", name),
			)
		} else {
			(
				"spacedrive.sock".to_string(),
				"spacedrive.pid".to_string(),
				"spacedrive.log".to_string(),
			)
		};

		Self {
			socket_path: runtime_dir.join(socket_name),
			pid_file: runtime_dir.join(pid_name),
			log_file: Some(runtime_dir.join(log_name)),
			instance_name,
		}
	}

	/// Get instance display name ("default" for None, or the actual name)
	pub fn instance_display_name(&self) -> &str {
		self.instance_name.as_deref().unwrap_or("default")
	}
}