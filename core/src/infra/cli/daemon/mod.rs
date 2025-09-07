//! Spacedrive daemon implementation
//!
//! The daemon runs in the background and handles all core operations.
//! The CLI communicates with it via Unix domain socket (on Unix) or named pipe (on Windows).

use crate::{infra::cli::state::CliState, Core};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, RwLock};
use tracing::{error, info, warn};

pub mod client;
pub mod config;
pub mod handlers;
pub mod services;
pub mod types;

pub use client::DaemonClient;
pub use config::DaemonConfig;
pub use types::*;

use services::{DaemonHelpers, StateService};

/// The daemon server
pub struct Daemon {
	core: Arc<Core>,
	config: DaemonConfig,
	start_time: std::time::Instant,
	shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
	cli_state: Arc<RwLock<CliState>>,
	data_dir: PathBuf,
	handler_registry: handlers::HandlerRegistry,
	state_service: Arc<StateService>,
}

impl Daemon {
	/// Create a new daemon instance
	pub async fn new(data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
		Self::new_with_instance(data_dir, None).await
	}

	/// Create a new daemon instance with optional instance name
	pub async fn new_with_instance(
		data_dir: PathBuf,
		instance_name: Option<String>,
	) -> Result<Self, Box<dyn std::error::Error>> {
		// Set up logging BEFORE initializing Core
		let config = DaemonConfig::new(instance_name.clone());
		if let Some(ref log_file) = config.log_file {
			DaemonHelpers::setup_file_logging(log_file)?;
		}

		let core = Arc::new(Core::new_with_config(data_dir.clone()).await?);

		// Load CLI state
		let cli_state = CliState::load(&data_dir).unwrap_or_default();
		let cli_state = Arc::new(RwLock::new(cli_state));

		// Create state service
		let state_service = Arc::new(StateService::new(cli_state.clone(), data_dir.clone()));

		// Auto-select first library if needed
		state_service.auto_select_library(&core).await?;

		// Create handler registry
		let shutdown_tx = Arc::new(tokio::sync::Mutex::new(None));
		let handler_registry = handlers::HandlerRegistry::new(
			std::time::Instant::now(),
			shutdown_tx.clone(),
			data_dir.clone(),
		);

		Ok(Self {
			core,
			config,
			start_time: std::time::Instant::now(),
			shutdown_tx,
			cli_state,
			data_dir,
			handler_registry,
			state_service,
		})
	}

	/// Create a new daemon instance with networking enabled
	pub async fn new_with_networking(
		data_dir: PathBuf,
	) -> Result<Self, Box<dyn std::error::Error>> {
		Self::new_with_networking_and_instance(data_dir, None).await
	}

	/// Create a new daemon instance with networking enabled and optional instance name
	pub async fn new_with_networking_and_instance(
		data_dir: PathBuf,
		instance_name: Option<String>,
	) -> Result<Self, Box<dyn std::error::Error>> {
		// Set up logging BEFORE initializing Core
		let config = DaemonConfig::new(instance_name.clone());
		if let Some(ref log_file) = config.log_file {
			DaemonHelpers::setup_file_logging(log_file)?;
		}

		let mut core = Core::new_with_config(data_dir.clone()).await?;

		// Initialize networking
		core.init_networking().await?;

		let core = Arc::new(core);

		// Load CLI state
		let cli_state = CliState::load(&data_dir).unwrap_or_default();
		let cli_state = Arc::new(RwLock::new(cli_state));

		// Create state service
		let state_service = Arc::new(StateService::new(cli_state.clone(), data_dir.clone()));

		// Auto-select first library if needed
		state_service.auto_select_library(&core).await?;

		// Create handler registry
		let shutdown_tx = Arc::new(tokio::sync::Mutex::new(None));
		let handler_registry = handlers::HandlerRegistry::new(
			std::time::Instant::now(),
			shutdown_tx.clone(),
			data_dir.clone(),
		);

		Ok(Self {
			core,
			config,
			start_time: std::time::Instant::now(),
			shutdown_tx,
			cli_state,
			data_dir,
			handler_registry,
			state_service,
		})
	}

	/// Start the daemon server
	pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
		// Logging is already set up in the constructor

		// Remove old socket if it exists
		if self.config.socket_path.exists() {
			std::fs::remove_file(&self.config.socket_path)?;
		}

		// Write PID file
		std::fs::write(&self.config.pid_file, std::process::id().to_string())?;

		// Create Unix socket
		let listener = UnixListener::bind(&self.config.socket_path)?;
		info!("Daemon listening on {:?}", self.config.socket_path);

		// Emit CoreStarted event to signal daemon is ready
		self.core
			.events
			.emit(crate::infra::event::Event::CoreStarted);

		// Set up shutdown channel
		let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
		*self.shutdown_tx.lock().await = Some(shutdown_tx);

		// Accept connections
		loop {
			tokio::select! {
				Ok((stream, _)) = listener.accept() => {
					let core = self.core.clone();
					let handler_registry = &self.handler_registry;
					let state_service = self.state_service.clone();

					// Handle client directly without spawning background task
					if let Err(e) = handle_client(stream, core, handler_registry, state_service).await {
						error!("Error handling client: {}", e);
					}
				}
				_ = &mut shutdown_rx => {
					info!("Daemon shutting down");
					break;
				}
			}
		}

		// Cleanup
		std::fs::remove_file(&self.config.socket_path).ok();
		std::fs::remove_file(&self.config.pid_file).ok();

		Ok(())
	}

	/// Check if daemon is running
	pub fn is_running() -> bool {
		Self::is_running_instance(None)
	}

	/// Check if daemon instance is running
	pub fn is_running_instance(instance_name: Option<String>) -> bool {
		let config = DaemonConfig::new(instance_name);

		if let Ok(pid_str) = std::fs::read_to_string(&config.pid_file) {
			if let Ok(pid) = pid_str.trim().parse::<u32>() {
				// Check if process is running (Unix only)
				#[cfg(unix)]
				{
					use std::process::Command;
					let output = Command::new("kill")
						.args(&["-0", &pid.to_string()])
						.output();

					if let Ok(output) = output {
						return output.status.success();
					}
				}
			}
		}

		false
	}

	/// Stop a running daemon
	pub async fn stop() -> Result<(), Box<dyn std::error::Error>> {
		Self::stop_instance(None).await
	}

	/// Stop a running daemon instance
	pub async fn stop_instance(
		instance_name: Option<String>,
	) -> Result<(), Box<dyn std::error::Error>> {
		let config = DaemonConfig::new(instance_name.clone());

		// First check if daemon is actually running
		if !Self::is_running_instance(instance_name) {
			return Err(format!(
				"Daemon instance '{}' is not running",
				config.instance_display_name()
			)
			.into());
		}

		// Try to connect and send shutdown command
		match UnixStream::connect(&config.socket_path).await {
			Ok(mut stream) => {
				let cmd = DaemonCommand::Shutdown;
				let json = serde_json::to_string(&cmd)?;
				stream.write_all(format!("{}\n", json).as_bytes()).await?;
				stream.flush().await?;

				// Wait a bit for graceful shutdown
				tokio::time::sleep(std::time::Duration::from_millis(500)).await;
			}
			Err(_) => {
				// If we can't connect to socket, try to kill the process
				if let Ok(pid_str) = std::fs::read_to_string(&config.pid_file) {
					if let Ok(pid) = pid_str.trim().parse::<u32>() {
						#[cfg(unix)]
						{
							use std::process::Command;
							Command::new("kill")
								.args(&["-TERM", &pid.to_string()])
								.output()?;
						}
					}
				}
			}
		}

		// Clean up files
		std::fs::remove_file(&config.socket_path).ok();
		std::fs::remove_file(&config.pid_file).ok();

		Ok(())
	}

	/// Wait for daemon to be ready by attempting to connect
	pub async fn wait_for_ready(
		instance_name: Option<String>,
		timeout_secs: u64,
	) -> Result<bool, Box<dyn std::error::Error>> {
		let config = DaemonConfig::new(instance_name);
		let start = std::time::Instant::now();
		let timeout = std::time::Duration::from_secs(timeout_secs);

		while start.elapsed() < timeout {
			// Try to connect to the socket
			if let Ok(mut stream) = UnixStream::connect(&config.socket_path).await {
				// Try to send a simple ping command to verify it's responsive
				let cmd = DaemonCommand::Ping;
				let json = serde_json::to_string(&cmd)?;

				if stream
					.write_all(format!("{}\n", json).as_bytes())
					.await
					.is_ok()
				{
					if stream.flush().await.is_ok() {
						// Try to read the response
						let mut reader = BufReader::new(stream);
						let mut line = String::new();
						if reader.read_line(&mut line).await.is_ok() {
							if let Ok(response) = serde_json::from_str::<DaemonResponse>(&line) {
								if matches!(response, DaemonResponse::Pong) {
									return Ok(true);
								}
							}
						}
					}
				}
			}

			// Wait a bit before retrying
			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		}

		Ok(false)
	}

	/// List all daemon instances
	pub fn list_instances() -> Result<Vec<DaemonInstance>, Box<dyn std::error::Error>> {
		let runtime_dir = dirs::runtime_dir()
			.or_else(|| dirs::cache_dir())
			.unwrap_or_else(|| PathBuf::from("/tmp"));

		let mut instances = Vec::new();

		// Find all spacedrive-*.sock files
		if let Ok(entries) = std::fs::read_dir(&runtime_dir) {
			for entry in entries.flatten() {
				let file_name = entry.file_name();
				let file_str = file_name.to_string_lossy();

				if file_str.starts_with("spacedrive") && file_str.ends_with(".sock") {
					let instance_name = if file_str == "spacedrive.sock" {
						None // Default instance
					} else {
						// Extract instance name from spacedrive-{name}.sock
						Some(
							file_str
								.strip_prefix("spacedrive-")
								.and_then(|s| s.strip_suffix(".sock"))
								.unwrap_or("unknown")
								.to_string(),
						)
					};

					let is_running = Self::is_running_instance(instance_name.clone());
					instances.push(DaemonInstance {
						name: instance_name,
						socket_path: entry.path(),
						is_running,
					});
				}
			}
		}

		// Sort by name for consistent output
		instances.sort_by(|a, b| {
			match (&a.name, &b.name) {
				(None, None) => std::cmp::Ordering::Equal,
				(None, Some(_)) => std::cmp::Ordering::Less, // Default first
				(Some(_), None) => std::cmp::Ordering::Greater,
				(Some(a), Some(b)) => a.cmp(b),
			}
		});

		Ok(instances)
	}
}

/// Handle a client connection
async fn handle_client(
	stream: UnixStream,
	core: Arc<Core>,
	handler_registry: &handlers::HandlerRegistry,
	state_service: Arc<StateService>,
) -> Result<(), Box<dyn std::error::Error>> {
	let (reader, mut writer) = stream.into_split();
	let mut reader = BufReader::new(reader);
	let mut line = String::new();

	while reader.read_line(&mut line).await? > 0 {
		let trimmed = line.trim();
		if trimmed.is_empty() {
			line.clear();
			continue;
		}

		let response = match serde_json::from_str::<DaemonCommand>(trimmed) {
			Ok(cmd) => {
				// info!("Handling daemon command: {:?}", cmd);
				handler_registry.handle(cmd, &core, &state_service).await
			}
			Err(e) => DaemonResponse::Error(format!("Invalid command: {}", e)),
		};

		let json = serde_json::to_string(&response)?;
		writer.write_all(format!("{}\n", json).as_bytes()).await?;
		writer.flush().await?;

		line.clear();
	}

	Ok(())
}
