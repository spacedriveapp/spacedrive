use clap::Parser;
use std::path::PathBuf;
use tokio::signal;

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

	// Resolve base data directory
	let base_data_dir = args
		.data_dir
		.unwrap_or(sd_core::config::default_data_dir()?);

	// Calculate instance-specific data directory and socket path
	let (data_dir, socket_path) = if let Some(instance) = args.instance {
		// Validate instance name for security
		validate_instance_name(&instance).map_err(|e| format!("Invalid instance name: {}", e))?;

		// Each instance gets its own data directory
		let instance_data_dir = base_data_dir.join("instances").join(&instance);
		let socket_path = base_data_dir
			.join("daemon")
			.join(format!("daemon-{}.sock", instance));

		(instance_data_dir, socket_path)
	} else {
		// Default instance uses the base data directory
		let socket_path = base_data_dir.join("daemon/daemon.sock");
		(base_data_dir.clone(), socket_path)
	};

	// Set up signal handling for graceful shutdown
	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install signal handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	// Run the daemon server with signal handling
	tokio::select! {
		result = sd_core::infra::daemon::bootstrap::start_default_server(
			socket_path,
			data_dir,
			true, // Always enable networking
		) => {
			result
		}
		() = ctrl_c => {
			println!("Received Ctrl+C, shutting down gracefully...");
			Ok(())
		}
		() = terminate => {
			println!("Received SIGTERM, shutting down gracefully...");
			Ok(())
		}
	}
}
