//! Spacedrive daemon implementation
//!
//! The daemon runs in the background and handles all core operations.
//! The CLI communicates with it via Unix domain socket (on Unix) or named pipe (on Windows).

use crate::{infrastructure::database::entities, Core};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tracing::{error, info, warn};
use uuid::Uuid;

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
				format!("spacedrive-{}.log", name)
			)
		} else {
			(
				"spacedrive.sock".to_string(),
				"spacedrive.pid".to_string(),
				"spacedrive.log".to_string()
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

/// Commands that can be sent to the daemon
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonCommand {
	// Core management
	Ping,
	Shutdown,
	GetStatus,

	// Library commands
	CreateLibrary { name: String, path: Option<PathBuf> },
	ListLibraries,
	SwitchLibrary { id: Uuid },
	GetCurrentLibrary,

	// Location commands
	AddLocation { path: PathBuf, name: Option<String> },
	ListLocations,
	RescanLocation { id: Uuid },
	RemoveLocation { id: Uuid },

	// Job commands
	ListJobs { status: Option<String> },
	GetJobInfo { id: Uuid },
	PauseJob { id: Uuid },
	ResumeJob { id: Uuid },
	CancelJob { id: Uuid },

	// Subscribe to events
	SubscribeEvents,

	// Networking commands  
	InitNetworking,
	StartNetworking,
	StopNetworking,
	ListConnectedDevices,
	RevokeDevice { device_id: Uuid },
	SendSpacedrop { 
		device_id: Uuid, 
		file_path: String, 
		sender_name: String, 
		message: Option<String> 
	},

	// Pairing commands
	StartPairingAsInitiator,
	StartPairingAsJoiner { code: String },
	GetPairingStatus,
	ListPendingPairings,
	AcceptPairing { request_id: Uuid },
	RejectPairing { request_id: Uuid },
}

/// Responses from the daemon
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
	Ok,
	Error(String),
	Pong,
	Status(DaemonStatus),
	LibraryCreated {
		id: Uuid,
		name: String,
		path: PathBuf,
	},
	Libraries(Vec<LibraryInfo>),
	CurrentLibrary(Option<LibraryInfo>),
	LocationAdded {
		location_id: Uuid,
		job_id: String,
	},
	Locations(Vec<LocationInfo>),
	Jobs(Vec<JobInfo>),
	JobInfo(Option<JobInfo>),
	Event(String), // Serialized event
	
	// Networking responses
	ConnectedDevices(Vec<ConnectedDeviceInfo>),
	SpacedropStarted { transfer_id: Uuid },

	// Pairing responses
	PairingCodeGenerated { code: String, expires_in_seconds: u32 },
	PairingInProgress,
	PairingStatus { status: String, remote_device: Option<ConnectedDeviceInfo> },
	PendingPairings(Vec<PairingRequestInfo>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonStatus {
	pub version: String,
	pub uptime_secs: u64,
	pub current_library: Option<Uuid>,
	pub active_jobs: usize,
	pub total_locations: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryInfo {
	pub id: Uuid,
	pub name: String,
	pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationInfo {
	pub id: Uuid,
	pub name: String,
	pub path: PathBuf,
	pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobInfo {
	pub id: Uuid,
	pub name: String,
	pub status: String,
	pub progress: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectedDeviceInfo {
	pub device_id: Uuid,
	pub device_name: String,
	pub device_type: String,
	pub os_version: String,
	pub app_version: String,
	pub peer_id: String,
	pub status: String,
	pub connection_active: bool,
	pub last_seen: String,
	pub connected_at: Option<String>,
	pub bytes_sent: u64,
	pub bytes_received: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingRequestInfo {
	pub request_id: Uuid,
	pub device_id: Uuid,
	pub device_name: String,
	pub received_at: String,
}

/// The daemon server
pub struct Daemon {
	core: Arc<Core>,
	config: DaemonConfig,
	start_time: std::time::Instant,
	shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
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
		let core = Arc::new(Core::new_with_config(data_dir).await?);

		// Ensure device is registered for all libraries
		let libraries = core.libraries.list().await;
		for library in libraries {
			// Register device if not already registered
			let db = library.db();
			let device = core.device.to_device()?;

			// Check if device exists
			use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

			let existing = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.eq(device.id))
				.one(db.conn())
				.await?;

			if existing.is_none() {
				// Register the device
				use sea_orm::ActiveValue::Set;
				let device_model = entities::device::ActiveModel {
					id: sea_orm::ActiveValue::NotSet,
					uuid: Set(device.id),
					name: Set(device.name),
					os: Set(device.os.to_string()),
					os_version: Set(None),
					hardware_model: Set(device.hardware_model),
					network_addresses: Set(serde_json::json!(device.network_addresses)),
					is_online: Set(true),
					last_seen_at: Set(device.last_seen_at),
					capabilities: Set(serde_json::json!({
						"indexing": true,
						"p2p": true,
						"volume_detection": true
					})),
					sync_leadership: Set(serde_json::json!(device.sync_leadership)),
					created_at: Set(device.created_at),
					updated_at: Set(device.updated_at),
				};

				use sea_orm::ActiveModelTrait;
				device_model.insert(db.conn()).await?;
				info!(
					"Registered device {} in library {}",
					device.id,
					library.id()
				);
			}
		}

		Ok(Self {
			core,
			config: DaemonConfig::new(instance_name.clone()),
			start_time: std::time::Instant::now(),
			shutdown_tx: Arc::new(tokio::sync::Mutex::new(None)),
		})
	}

	/// Create a new daemon instance with networking enabled
	pub async fn new_with_networking(
		data_dir: PathBuf
	) -> Result<Self, Box<dyn std::error::Error>> {
		Self::new_with_networking_and_instance(data_dir, None).await
	}

	/// Create a new daemon instance with networking enabled and optional instance name
	pub async fn new_with_networking_and_instance(
		data_dir: PathBuf,
		instance_name: Option<String>,
	) -> Result<Self, Box<dyn std::error::Error>> {
		let mut core = Core::new_with_config(data_dir).await?;
		
		// Initialize networking
		core.init_networking().await?;
		core.start_networking().await?;
		
		let core = Arc::new(core);

		// Ensure device is registered for all libraries
		let libraries = core.libraries.list().await;
		for library in libraries {
			// Register device if not already registered
			let db = library.db();
			let device = core.device.to_device()?;

			// Check if device exists
			use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

			let existing = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.eq(device.id))
				.one(db.conn())
				.await?;

			if existing.is_none() {
				// Register the device
				use sea_orm::ActiveValue::Set;
				let device_model = entities::device::ActiveModel {
					id: sea_orm::ActiveValue::NotSet,
					uuid: Set(device.id),
					name: Set(device.name),
					os: Set(device.os.to_string()),
					os_version: Set(None),
					hardware_model: Set(device.hardware_model),
					network_addresses: Set(serde_json::json!(device.network_addresses)),
					is_online: Set(true),
					last_seen_at: Set(device.last_seen_at),
					capabilities: Set(serde_json::json!({
						"indexing": true,
						"p2p": true,
						"volume_detection": true
					})),
					sync_leadership: Set(serde_json::json!(device.sync_leadership)),
					created_at: Set(device.created_at),
					updated_at: Set(device.updated_at),
				};

				use sea_orm::ActiveModelTrait;
				device_model.insert(db.conn()).await?;
				info!(
					"Registered device {} in library {}",
					device.id,
					library.id()
				);
			}
		}

		Ok(Self {
			core,
			config: DaemonConfig::new(instance_name.clone()),
			start_time: std::time::Instant::now(),
			shutdown_tx: Arc::new(tokio::sync::Mutex::new(None)),
		})
	}

	/// Start the daemon server
	pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
		// Remove old socket if it exists
		if self.config.socket_path.exists() {
			std::fs::remove_file(&self.config.socket_path)?;
		}

		// Write PID file
		std::fs::write(&self.config.pid_file, std::process::id().to_string())?;

		// Create Unix socket
		let listener = UnixListener::bind(&self.config.socket_path)?;
		info!("Daemon listening on {:?}", self.config.socket_path);

		// Set up shutdown channel
		let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
		*self.shutdown_tx.lock().await = Some(shutdown_tx);

		// Accept connections
		loop {
			tokio::select! {
				Ok((stream, _)) = listener.accept() => {
					let core = self.core.clone();
					let start_time = self.start_time;
					let shutdown_tx = self.shutdown_tx.clone();

					// Handle client directly without spawning background task
					if let Err(e) = handle_client(stream, core, start_time, shutdown_tx).await {
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
	pub async fn stop_instance(instance_name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
		let config = DaemonConfig::new(instance_name.clone());

		// First check if daemon is actually running
		if !Self::is_running_instance(instance_name) {
			return Err(format!("Daemon instance '{}' is not running", config.instance_display_name()).into());
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
						Some(file_str.strip_prefix("spacedrive-")
								   .and_then(|s| s.strip_suffix(".sock"))
								   .unwrap_or("unknown")
								   .to_string())
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

/// Daemon instance information
#[derive(Debug)]
pub struct DaemonInstance {
	pub name: Option<String>,  // None for default instance
	pub socket_path: PathBuf,
	pub is_running: bool,
}

impl DaemonInstance {
	/// Get instance display name (\"default\" for None, or the actual name)
	pub fn display_name(&self) -> &str {
		self.name.as_deref().unwrap_or("default")
	}
}

/// Handle a client connection
async fn handle_client(
	stream: UnixStream,
	core: Arc<Core>,
	start_time: std::time::Instant,
	shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
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

		match serde_json::from_str::<DaemonCommand>(trimmed) {
			Ok(cmd) => {
				let is_shutdown = matches!(cmd, DaemonCommand::Shutdown);
				let response = handle_command(cmd, &core, start_time).await;
				let json = serde_json::to_string(&response)?;
				writer.write_all(format!("{}\n", json).as_bytes()).await?;

				if is_shutdown {
					// Trigger daemon shutdown
					let mut shutdown_guard = shutdown_tx.lock().await;
					if let Some(tx) = shutdown_guard.take() {
						let _ = tx.send(());
					}
					break;
				}
			}
			Err(e) => {
				let response = DaemonResponse::Error(format!("Invalid command: {}", e));
				let json = serde_json::to_string(&response)?;
				writer.write_all(format!("{}\n", json).as_bytes()).await?;
			}
		}

		line.clear();
	}

	Ok(())
}

/// Handle a daemon command
async fn handle_command(
	cmd: DaemonCommand,
	core: &Arc<Core>,
	start_time: std::time::Instant,
) -> DaemonResponse {
	info!("Handling daemon command: {:?}", cmd);
	match cmd {
		DaemonCommand::Ping => DaemonResponse::Pong,

		DaemonCommand::Shutdown => {
			// Shutdown will be handled by the daemon after sending response
			DaemonResponse::Ok
		}

		DaemonCommand::GetStatus => {
			let libraries = core.libraries.list().await;
			let current_library = libraries.first().map(|l| l.id());

			DaemonResponse::Status(DaemonStatus {
				version: env!("CARGO_PKG_VERSION").to_string(),
				uptime_secs: start_time.elapsed().as_secs(),
				current_library,
				active_jobs: 0,     // TODO: Get from job manager
				total_locations: 0, // TODO: Get from location manager
			})
		}

		DaemonCommand::ListLibraries => {
			let libraries = core.libraries.list().await;
			let infos: Vec<LibraryInfo> =
				futures::future::join_all(libraries.into_iter().map(|lib| async move {
					LibraryInfo {
						id: lib.id(),
						name: lib.name().await,
						path: lib.path().to_path_buf(),
					}
				}))
				.await;

			DaemonResponse::Libraries(infos)
		}

		DaemonCommand::CreateLibrary { name, path } => {
			match core.libraries.create_library(&name, path).await {
				Ok(library) => {
					// Register device in the new library
					let db = library.db();
					let device = match core.device.to_device() {
						Ok(d) => d,
						Err(e) => return DaemonResponse::Error(e.to_string()),
					};

					// Register the device
					use sea_orm::ActiveValue::Set;
					let device_model = entities::device::ActiveModel {
						id: sea_orm::ActiveValue::NotSet,
						uuid: Set(device.id),
						name: Set(device.name.clone()),
						os: Set(device.os.to_string()),
						os_version: Set(None),
						hardware_model: Set(device.hardware_model),
						network_addresses: Set(serde_json::json!(device.network_addresses)),
						is_online: Set(true),
						last_seen_at: Set(device.last_seen_at),
						capabilities: Set(serde_json::json!({
							"indexing": true,
							"p2p": true,
							"volume_detection": true
						})),
						sync_leadership: Set(serde_json::json!(device.sync_leadership)),
						created_at: Set(device.created_at),
						updated_at: Set(device.updated_at),
					};

					use sea_orm::ActiveModelTrait;
					match device_model.insert(db.conn()).await {
						Ok(_) => {
							info!(
								"Registered device {} in new library {}",
								device.id,
								library.id()
							);
							DaemonResponse::LibraryCreated {
								id: library.id(),
								name: library.name().await,
								path: library.path().to_path_buf(),
							}
						}
						Err(e) => {
							DaemonResponse::Error(format!("Failed to register device: {}", e))
						}
					}
				}
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::GetCurrentLibrary => {
			// For now, return the first library as current
			// TODO: Implement proper current library tracking
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				DaemonResponse::CurrentLibrary(Some(LibraryInfo {
					id: library.id(),
					name: library.name().await,
					path: library.path().to_path_buf(),
				}))
			} else {
				DaemonResponse::CurrentLibrary(None)
			}
		}

		DaemonCommand::AddLocation { path, name } => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				// Get current device from database
				let db = library.db();
				let device = match core.device.to_device() {
					Ok(d) => d,
					Err(e) => return DaemonResponse::Error(e.to_string()),
				};

				// Find device in database
				use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
				let device_record = match entities::device::Entity::find()
					.filter(entities::device::Column::Uuid.eq(device.id))
					.one(db.conn())
					.await
				{
					Ok(Some(d)) => d,
					Ok(None) => {
						return DaemonResponse::Error(
							"Device not registered in database".to_string(),
						)
					}
					Err(e) => return DaemonResponse::Error(format!("Database error: {}", e)),
				};

				// Create location manager
				let location_manager =
					crate::location::LocationManager::new((*core.events).clone());

				// Add location
				match location_manager
					.add_location(
						library.clone(),
						path.clone(),
						name,
						device_record.id,
						crate::location::IndexMode::Content,
					)
					.await
				{
					Ok((location_id, job_id)) => DaemonResponse::LocationAdded {
						location_id,
						job_id,
					},
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::ListLocations => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				let location_manager =
					crate::location::LocationManager::new((*core.events).clone());

				match location_manager.list_locations(library.as_ref()).await {
					Ok(locations) => {
						let infos: Vec<LocationInfo> = locations
							.into_iter()
							.map(|loc| {
								LocationInfo {
									id: loc.id,
									name: loc.name,
									path: loc.path,
									status: "active".to_string(), // TODO: Get actual status
								}
							})
							.collect();

						DaemonResponse::Locations(infos)
					}
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::RemoveLocation { id } => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				let location_manager =
					crate::location::LocationManager::new((*core.events).clone());

				match location_manager.remove_location(library.as_ref(), id).await {
					Ok(_) => DaemonResponse::Ok,
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::RescanLocation { id } => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				let location_manager =
					crate::location::LocationManager::new((*core.events).clone());

				match location_manager
					.rescan_location(library.clone(), id, false)
					.await
				{
					Ok(_) => DaemonResponse::Ok,
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::ListJobs { status } => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				let job_manager = library.jobs();

				// For running jobs, get from memory for live monitoring
				if let Some(ref status_str) = status {
					if status_str == "running" {
						let running_jobs = job_manager.list_running_jobs().await;
						let infos: Vec<JobInfo> = running_jobs
							.into_iter()
							.map(|j| JobInfo {
								id: j.id,
								name: j.name,
								status: j.status.to_string(),
								progress: j.progress,
							})
							.collect();

						return DaemonResponse::Jobs(infos);
					}
				}

				// For other statuses, query the database
				let status_filter = status.and_then(|s| match s.as_str() {
					"queued" => Some(crate::infrastructure::jobs::types::JobStatus::Queued),
					"running" => Some(crate::infrastructure::jobs::types::JobStatus::Running),
					"completed" => Some(crate::infrastructure::jobs::types::JobStatus::Completed),
					"failed" => Some(crate::infrastructure::jobs::types::JobStatus::Failed),
					"paused" => Some(crate::infrastructure::jobs::types::JobStatus::Paused),
					"cancelled" => Some(crate::infrastructure::jobs::types::JobStatus::Cancelled),
					_ => None,
				});

				match job_manager.list_jobs(status_filter).await {
					Ok(jobs) => {
						let infos: Vec<JobInfo> = jobs
							.into_iter()
							.map(|j| JobInfo {
								id: j.id,
								name: j.name,
								status: j.status.to_string(),
								progress: j.progress,
							})
							.collect();

						DaemonResponse::Jobs(infos)
					}
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::GetJobInfo { id } => {
			// Get current library
			let libraries = core.libraries.list().await;
			if let Some(library) = libraries.first() {
				let job_manager = library.jobs();

				match job_manager.get_job_info(id).await {
					Ok(job) => DaemonResponse::JobInfo(job.map(|j| JobInfo {
						id: j.id,
						name: j.name,
						status: j.status.to_string(),
						progress: j.progress,
					})),
					Err(e) => DaemonResponse::Error(e.to_string()),
				}
			} else {
				DaemonResponse::Error("No library selected".to_string())
			}
		}

		DaemonCommand::SwitchLibrary { id } => {
			// For now, we don't actually switch - just verify the library exists
			let libraries = core.libraries.list().await;
			if libraries.iter().any(|lib| lib.id() == id) {
				DaemonResponse::Ok
			} else {
				DaemonResponse::Error("Library not found".to_string())
			}
		}

		DaemonCommand::PauseJob { id } => {
			// TODO: Implement job pause when job manager supports it
			DaemonResponse::Error("Job pause not yet implemented".to_string())
		}

		DaemonCommand::ResumeJob { id } => {
			// TODO: Implement job resume when job manager supports it
			DaemonResponse::Error("Job resume not yet implemented".to_string())
		}

		DaemonCommand::CancelJob { id } => {
			// TODO: Implement job cancel when job manager supports it
			DaemonResponse::Error("Job cancel not yet implemented".to_string())
		}

		DaemonCommand::SubscribeEvents => {
			// TODO: Implement event subscription
			DaemonResponse::Error("Event subscription not yet implemented".to_string())
		}

		// Networking commands
		DaemonCommand::InitNetworking => {
			// Check if networking is already initialized
			if core.networking().is_some() {
				DaemonResponse::Ok // Networking is already available
			} else {
				// Networking not available - daemon needs to be restarted with networking
				DaemonResponse::Error(
					"Networking not available. Restart daemon with: spacedrive start --enable-networking".to_string()
				)
			}
		}

		DaemonCommand::StartNetworking => {
			match core.start_networking().await {
				Ok(_) => DaemonResponse::Ok,
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::StopNetworking => {
			// TODO: Implement networking stop when available
			DaemonResponse::Error("Stop networking not yet implemented".to_string())
		}

		DaemonCommand::ListConnectedDevices => {
			match core.get_connected_devices_info().await {
				Ok(devices) => {
					let connected_devices: Vec<ConnectedDeviceInfo> = devices
						.into_iter()
						.map(|device| {
							// Get connection status from networking service
							let (peer_id, connection_active, connected_at, bytes_sent, bytes_received) = 
								if let Some(networking) = &core.networking {
									// Try to get connection details - this is a simplified version
									// In a real implementation, we'd access the connection registry
									("unknown".to_string(), true, Some("now".to_string()), 0, 0)
								} else {
									("unavailable".to_string(), false, None, 0, 0)
								};

							ConnectedDeviceInfo {
								device_id: device.device_id,
								device_name: device.device_name,
								device_type: format!("{:?}", device.device_type),
								os_version: device.os_version,
								app_version: device.app_version,
								peer_id,
								status: "connected".to_string(),
								connection_active,
								last_seen: device.last_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
								connected_at,
								bytes_sent,
								bytes_received,
							}
						})
						.collect();

					DaemonResponse::ConnectedDevices(connected_devices)
				}
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::RevokeDevice { device_id } => {
			match core.revoke_device(device_id).await {
				Ok(_) => DaemonResponse::Ok,
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::SendSpacedrop { 
			device_id, 
			file_path, 
			sender_name, 
			message 
		} => {
			match core.send_spacedrop(device_id, &file_path, sender_name, message).await {
				Ok(transfer_id) => DaemonResponse::SpacedropStarted { transfer_id },
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		// Pairing commands
		DaemonCommand::StartPairingAsInitiator => {
			match core.start_pairing_as_initiator().await {
				Ok((code, expires_in_seconds)) => DaemonResponse::PairingCodeGenerated { 
					code, 
					expires_in_seconds 
				},
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::StartPairingAsJoiner { code } => {
			match core.start_pairing_as_joiner(&code).await {
				Ok(_) => DaemonResponse::PairingInProgress,
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::GetPairingStatus => {
			match core.get_pairing_status().await {
				Ok(sessions) => {
					// Convert sessions to status format for compatibility
					if let Some(session) = sessions.first() {
						let status = match &session.state {
							crate::networking::PairingState::Idle => "idle",
							crate::networking::PairingState::GeneratingCode => "generating_code",
							crate::networking::PairingState::Broadcasting => "broadcasting",
							crate::networking::PairingState::Scanning => "scanning",
							crate::networking::PairingState::WaitingForConnection => "waiting_for_connection",
							crate::networking::PairingState::Connecting => "connecting",
							crate::networking::PairingState::Authenticating => "authenticating",
							crate::networking::PairingState::ExchangingKeys => "exchanging_keys",
							crate::networking::PairingState::AwaitingConfirmation => "awaiting_confirmation",
							crate::networking::PairingState::EstablishingSession => "establishing_session",
							crate::networking::PairingState::ChallengeReceived { .. } => "authenticating",
							crate::networking::PairingState::ResponseSent => "authenticating",
							crate::networking::PairingState::Completed => "completed",
							crate::networking::PairingState::Failed { .. } => "failed",
							crate::networking::PairingState::ResponsePending { .. } => "responding",
						}.to_string();
						
						DaemonResponse::PairingStatus { 
							status, 
							remote_device: None // No device info available yet in new system
						}
					} else {
						DaemonResponse::PairingStatus { 
							status: "no_active_pairing".to_string(), 
							remote_device: None 
						}
					}
				}
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::ListPendingPairings => {
			match core.list_pending_pairings().await {
				Ok(requests) => {
					let pairing_requests = requests.into_iter().map(|req| PairingRequestInfo {
						request_id: req.request_id,
						device_id: req.device_id,
						device_name: req.device_name,
						received_at: req.received_at.to_string(),
					}).collect();
					DaemonResponse::PendingPairings(pairing_requests)
				}
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::AcceptPairing { request_id } => {
			match core.accept_pairing_request(request_id).await {
				Ok(_) => DaemonResponse::Ok,
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}

		DaemonCommand::RejectPairing { request_id } => {
			match core.reject_pairing_request(request_id).await {
				Ok(_) => DaemonResponse::Ok,
				Err(e) => DaemonResponse::Error(e.to_string()),
			}
		}
	}
}

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
		Daemon::is_running_instance(self.instance_name.clone())
	}
}
