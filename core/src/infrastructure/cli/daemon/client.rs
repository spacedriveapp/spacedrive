//! Client for communicating with the daemon

use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::config::DaemonConfig;
use super::types::{DaemonCommand, DaemonResponse};

/// Client for communicating with the daemon
pub struct DaemonClient {
	socket_path: PathBuf,
	instance_name: Option<String>,
}

impl DaemonClient {
	pub fn new() -> Self {
		Self::new_with_instance(None)
	}

	pub fn new_with_instance(instance_name: Option<String>) -> Self {
		let config = DaemonConfig::new(instance_name.clone());
		Self {
			socket_path: config.socket_path,
			instance_name,
		}
	}

	/// Send a command to the daemon
	pub async fn send_command(
		&self,
		cmd: DaemonCommand,
	) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
		let mut stream = UnixStream::connect(&self.socket_path).await?;

		// Send command
		let json = serde_json::to_string(&cmd)?;
		stream.write_all(format!("{}\n", json).as_bytes()).await?;

		// Read response
		let mut reader = BufReader::new(stream);
		let mut line = String::new();
		reader.read_line(&mut line).await?;

		let response: DaemonResponse = serde_json::from_str(line.trim())?;
		Ok(response)
	}

	/// Check if daemon is running
	pub fn is_running(&self) -> bool {
		super::Daemon::is_running_instance(self.instance_name.clone())
	}
}