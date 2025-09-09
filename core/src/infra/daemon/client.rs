use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};

pub struct DaemonClient {
	socket_path: PathBuf,
}

impl DaemonClient {
	pub fn new(socket_path: PathBuf) -> Self { Self { socket_path } }

	pub async fn send(&self, req: &DaemonRequest) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
		let mut stream = UnixStream::connect(&self.socket_path).await?;
		let payload = serde_json::to_vec(req)?;
		stream.write_all(&payload).await?;
		stream.shutdown().await?;
		let mut buf = Vec::new();
		stream.read_to_end(&mut buf).await?;
		Ok(serde_json::from_slice(&buf)?)
	}
}


