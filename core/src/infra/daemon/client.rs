use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};

pub struct DaemonClient {
	socket_path: PathBuf,
}

impl DaemonClient {
	pub fn new(socket_path: PathBuf) -> Self {
		Self { socket_path }
	}

	pub async fn send(
		&self,
		req: &DaemonRequest,
	) -> Result<DaemonResponse, Box<dyn std::error::Error + Send + Sync>> {
		let mut stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
			format!(
				"Failed to connect to daemon socket at {}: {}",
				self.socket_path.display(),
				e
			)
		})?;

		let payload =
			serde_json::to_vec(req).map_err(|e| format!("Failed to serialize request: {}", e))?;

		stream
			.write_all(&payload)
			.await
			.map_err(|e| format!("Failed to send request to daemon: {}", e))?;

		stream
			.shutdown()
			.await
			.map_err(|e| format!("Failed to shutdown write stream: {}", e))?;

		let mut buf = Vec::new();
		stream
			.read_to_end(&mut buf)
			.await
			.map_err(|e| format!("Failed to read response from daemon: {}", e))?;

		let response: DaemonResponse = serde_json::from_slice(&buf)
			.map_err(|e| format!("Failed to deserialize daemon response: {}", e))?;

		Ok(response)
	}
}
