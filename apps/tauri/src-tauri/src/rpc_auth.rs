//! # Daemon RPC secret
//!
//! The daemon serves a connection only after its first line presents the
//! secret it wrote to `rpc.secret` in its data directory at start (threat
//! model F1). This crate does not link `sd-core`, so it speaks that first line
//! here; `sd_core::infra::daemon::auth` is the definition.

use std::path::Path;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Connect to the daemon at `socket_addr` owning `data_dir` and present its
/// secret. The request goes on the returned stream next.
///
/// The file is read on every connection because the daemon issues a new
/// secret each time it starts.
pub async fn connect(socket_addr: &str, data_dir: &Path) -> std::io::Result<TcpStream> {
	let mut stream = TcpStream::connect(socket_addr).await?;
	let secret = tokio::fs::read_to_string(data_dir.join("rpc.secret")).await?;
	let line = serde_json::json!({ "auth": secret.trim() }).to_string() + "\n";
	stream.write_all(line.as_bytes()).await?;
	Ok(stream)
}
