use std::path::PathBuf;

use anyhow::Result;
use spacedrive_core::infra::daemon::client::DaemonClient;
use spacedrive_core::infra::daemon::types::{DaemonRequest, DaemonResponse};

#[tokio::main]
async fn main() -> Result<()> {
	let data_dir = spacedrive_core::config::default_data_dir()?;
	let socket = data_dir.join("daemon/daemon.sock");

	let client = DaemonClient::new(socket);

	// Ping test
	match client.send(&DaemonRequest::Ping).await? {
		DaemonResponse::Pong => println!("Daemon reachable"),
		other => println!("Unexpected response: {:?}", other),
	}

	Ok(())
}


