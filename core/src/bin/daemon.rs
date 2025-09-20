use clap::Parser;
use std::path::PathBuf;

/// Validate instance name to prevent path traversal attacks
fn validate_instance_name(instance: &str) -> Result<(), String> {
	if instance.is_empty() {
		return Err("Instance name cannot be empty".to_string());
	}
	if instance.len() > 64 {
		return Err("Instance name too long (max 64 characters)".to_string());
	}
	if !instance
		.chars()
		.all(|c| c.is_alphanumeric() || c == '-' || c == '_')
	{
		return Err("Instance name contains invalid characters. Only alphanumeric, dash, and underscore allowed".to_string());
	}
	Ok(())
}

#[derive(Parser, Debug)]
#[command(name = "spacedrive-daemon", about = "Spacedrive daemon")]
struct Args {
	/// Path to spacedrive data directory
	#[arg(long)]
	data_dir: Option<PathBuf>,

	/// Daemon instance name
	#[arg(long)]
	instance: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	// Resolve data directory
	let data_dir = args
		.data_dir
		.unwrap_or(sd_core::config::default_data_dir()?);

	// Calculate socket path based on instance
	let socket_path = if let Some(instance) = args.instance {
		// Validate instance name for security
		validate_instance_name(&instance).map_err(|e| format!("Invalid instance name: {}", e))?;
		data_dir
			.join("daemon")
			.join(format!("daemon-{}.sock", instance))
	} else {
		data_dir.join("daemon/daemon.sock")
	};

	sd_core::infra::daemon::bootstrap::start_default_server(
		socket_path,
		data_dir,
		true, // Always enable networking
	)
	.await
}
