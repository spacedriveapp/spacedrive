// use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Resolve default data dir and socket path
	let data_dir = sd_core::config::default_data_dir()?;
	let socket_path = data_dir.join("daemon/daemon.sock");

	sd_core::infra::daemon::bootstrap::start_default_server(socket_path, data_dir).await
}
