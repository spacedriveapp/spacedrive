use super::state::CliState;
use crate::{
	infrastructure::{database::entities, jobs::types::JobStatus},
	library::Library,
	location::{create_location, LocationCreateArgs},
	infrastructure::actions::Action,
	operations::{
		indexing::{IndexMode, IndexScope},
	},
	shared::types::SdPath,
	Core,
};
use clap::{Subcommand, ValueEnum};
use colored::*;
use comfy_table::{presets::UTF8_FULL, Cell, Table};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

// We need to create a wrapper for clap ValueEnum since the original IndexMode doesn't have it
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexMode {
	/// Only metadata (fast)
	Shallow,
	/// Metadata + content hashing
	Content,
	/// Full analysis including media metadata
	Deep,
}

impl From<CliIndexMode> for IndexMode {
	fn from(mode: CliIndexMode) -> Self {
		match mode {
			CliIndexMode::Shallow => IndexMode::Shallow,
			CliIndexMode::Content => IndexMode::Content,
			CliIndexMode::Deep => IndexMode::Deep,
		}
	}
}

impl From<CliIndexMode> for crate::location::IndexMode {
	fn from(mode: CliIndexMode) -> Self {
		match mode {
			CliIndexMode::Shallow => crate::location::IndexMode::Shallow,
			CliIndexMode::Content => crate::location::IndexMode::Content,
			CliIndexMode::Deep => crate::location::IndexMode::Deep,
		}
	}
}

// We need to create a wrapper for clap ValueEnum since the original IndexScope doesn't have it
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexScope {
	/// Index only the current directory (single level)
	Current,
	/// Index recursively through all subdirectories
	Recursive,
}

impl From<CliIndexScope> for crate::operations::indexing::IndexScope {
	fn from(scope: CliIndexScope) -> Self {
		match scope {
			CliIndexScope::Current => crate::operations::indexing::IndexScope::Current,
			CliIndexScope::Recursive => crate::operations::indexing::IndexScope::Recursive,
		}
	}
}

#[derive(Subcommand, Clone)]
pub enum IndexCommands {
	/// Quick scan of a directory (metadata only, current scope)
	QuickScan {
		/// Path to scan
		path: PathBuf,
		/// Scope: current or recursive
		#[arg(short, long, value_enum, default_value = "current")]
		scope: CliIndexScope,
		/// Run ephemerally (no database writes)
		#[arg(short, long)]
		ephemeral: bool,
	},

	/// Browse external paths without adding to managed locations
	Browse {
		/// Path to browse
		path: PathBuf,
		/// Scope: current or recursive
		#[arg(short, long, value_enum, default_value = "current")]
		scope: CliIndexScope,
		/// Enable content analysis
		#[arg(short, long)]
		content: bool,
	},

	/// Traditional full location indexing
	Location {
		/// Location ID or path
		identifier: String,
		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,
		/// Scope: current or recursive
		#[arg(short, long, value_enum, default_value = "recursive")]
		scope: CliIndexScope,
	},
}

#[derive(Subcommand, Clone)]
pub enum LibraryCommands {
	/// Create a new library
	Create {
		/// Name of the library
		name: String,
		/// Path where to create the library
		#[arg(short, long)]
		path: Option<PathBuf>,
	},

	/// List all libraries
	List,

	/// Open and switch to a library
	Open {
		/// Path to the library
		path: PathBuf,
	},

	/// Switch to a library by name or ID
	Switch {
		/// Library name or UUID
		identifier: String,
	},

	/// Show current library info
	Current,

	/// Close the current library
	Close,
}

#[derive(Subcommand, Clone)]
pub enum LocationCommands {
	/// Add a new location to the current library
	Add {
		/// Path to add as a location
		path: PathBuf,
		/// Name for the location
		#[arg(short, long)]
		name: Option<String>,
		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,
	},

	/// List all locations in the current library
	List,

	/// Remove a location
	Remove {
		/// Location ID or path
		identifier: String,
	},

	/// Rescan a location
	Rescan {
		/// Location ID or path
		identifier: String,
		/// Force full rescan (ignore change detection)
		#[arg(short, long)]
		force: bool,
	},

	/// Show location details
	Info {
		/// Location ID or path
		identifier: String,
	},
}

#[derive(Subcommand, Clone)]
pub enum JobCommands {
	/// List all jobs
	List {
		/// Filter by status
		#[arg(short, long)]
		status: Option<String>,
		/// Show only recent jobs
		#[arg(short, long)]
		recent: bool,
	},

	/// Show job details
	Info {
		/// Job ID
		id: Uuid,
	},

	/// Monitor jobs in real-time
	Monitor {
		/// Optional job ID to monitor a specific job
		#[arg(short, long)]
		job_id: Option<String>,
	},

	/// Pause a running job
	Pause {
		/// Job ID
		id: Uuid,
	},

	/// Resume a paused job
	Resume {
		/// Job ID
		id: Uuid,
	},

	/// Cancel a job
	Cancel {
		/// Job ID
		id: Uuid,
	},
}

#[derive(Subcommand, Clone)]
pub enum NetworkCommands {
	/// Initialize networking using master key
	Init,

	/// Start networking service
	Start,

	/// Stop networking service
	Stop,

	/// List connected devices
	Devices,

	/// Revoke trust from a device
	Revoke {
		/// Device ID to revoke
		device_id: Uuid,
	},

	/// Send a file via Spacedrop
	Spacedrop {
		/// Target device ID
		device_id: Uuid,
		/// File path to send
		file_path: PathBuf,
		/// Your name as sender
		#[arg(short, long, default_value = "Unknown")]
		sender: String,
		/// Optional message
		#[arg(short, long)]
		message: Option<String>,
	},

	/// Device pairing commands
	Pair {
		#[command(subcommand)]
		action: PairingCommands,
	},
}

#[derive(Subcommand, Clone)]
pub enum PairingCommands {
	/// Generate a pairing code and wait for another device to connect (initiator)
	Generate,

	/// Connect to another device using a pairing code (joiner)
	Join {
		/// The 12-word pairing code from the other device
		#[arg(short, long)]
		code: Option<String>,
	},

	/// Show current pairing status
	Status,

	/// List all pending pairing requests
	ListPending,

	/// Accept a pending pairing request
	Accept {
		/// Request ID to accept
		request_id: Uuid,
	},

	/// Reject a pending pairing request
	Reject {
		/// Request ID to reject
		request_id: Uuid,
	},
}

pub async fn handle_library_command(
	cmd: LibraryCommands,
	core: &Core,
	state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	match cmd {
		LibraryCommands::Create { name, path } => {
			println!("📚 Creating library '{}'...", name.bright_cyan());

			// Get the action manager from core context
			let action_manager = core
				.context
				.get_action_manager()
				.await
				.ok_or("Action manager not available")?;

			// Create the action
			let action = Action::LibraryCreate(
				crate::operations::libraries::create::action::LibraryCreateAction {
					name: name.clone(),
					path: path.clone(),
				}
			);

			// Dispatch the action
			match action_manager.dispatch(action).await {
				Ok(receipt) => {
					if let Some(payload) = receipt.result_payload {
						if let (Some(lib_id), Some(lib_path)) = (
							payload
								.get("library_id")
								.and_then(|v| v.as_str())
								.and_then(|s| uuid::Uuid::parse_str(s).ok()),
							payload.get("path").and_then(|v| v.as_str()),
						) {
							state.set_current_library(lib_id, std::path::PathBuf::from(lib_path));

							println!("✅ Library created successfully!");
							println!("   ID: {}", lib_id.to_string().bright_yellow());
							println!("   Path: {}", lib_path.bright_blue());
							println!("   Status: {}", "Active".bright_green());
						} else {
							println!("✅ Library created successfully!");
							println!(
								"   Action ID: {}",
								receipt.action_id.to_string().bright_yellow()
							);
						}
					} else {
						println!("✅ Library creation initiated!");
						println!(
							"   Action ID: {}",
							receipt.action_id.to_string().bright_yellow()
						);
					}
				}
				Err(e) => {
					println!("❌ Failed to create library: {}", e);
					return Err(Box::new(e));
				}
			}
		}

		LibraryCommands::List => {
			let libraries = core.libraries.list().await;

			if libraries.is_empty() {
				println!(
					"📭 No libraries found. Create one with: {}",
					"spacedrive library create <name>".bright_cyan()
				);
				return Ok(());
			}

			let mut table = Table::new();
			table
				.load_preset(UTF8_FULL)
				.set_header(vec!["Status", "Name", "ID", "Path"]);

			for library in libraries {
				let id = library.id();
				let name = library.name().await;
				let path = library.path();
				let is_current = state.current_library_id == Some(id);

				let status = if is_current {
					"●".bright_green().to_string()
				} else {
					"○".normal().to_string()
				};

				table.add_row(vec![
					Cell::new(status),
					Cell::new(name),
					Cell::new(id.to_string()),
					Cell::new(path.display()),
				]);
			}

			println!("{}", table);
		}

		LibraryCommands::Open { path } => {
			println!(
				"📂 Opening library at {}...",
				path.display().to_string().bright_blue()
			);

			let library = core.libraries.open_library(&path).await?;
			let lib_id = library.id();

			state.set_current_library(lib_id, path.clone());

			println!("✅ Library opened successfully!");
			println!("   Name: {}", library.name().await.bright_cyan());
			println!("   ID: {}", lib_id.to_string().bright_yellow());
		}

		LibraryCommands::Switch { identifier } => {
			let libraries = core.libraries.list().await;

			let mut found_library = None;
			for lib in libraries {
				let lib_name = lib.name().await;
				if lib.id().to_string().starts_with(&identifier) || lib_name == identifier {
					found_library = Some((lib, lib_name));
					break;
				}
			}

			match found_library {
				Some((lib, lib_name)) => {
					let lib_id = lib.id();
					let lib_path = lib.path().to_path_buf();
					state.set_current_library(lib_id, lib_path);

					println!("✅ Switched to library: {}", lib_name.bright_cyan());
				}
				None => {
					println!("❌ Library not found: {}", identifier.bright_red());
				}
			}
		}

		LibraryCommands::Current => {
			if let Some(lib_id) = &state.current_library_id {
				let libraries = core.libraries.list().await;
				if let Some(library) = libraries.into_iter().find(|lib| lib.id() == *lib_id) {
					println!("📚 Current library: {}", library.name().await.bright_cyan());
					println!("   ID: {}", lib_id.to_string().bright_yellow());
					println!(
						"   Path: {}",
						library.path().display().to_string().bright_blue()
					);
				} else {
					println!("⚠️  Current library no longer exists");
					state.current_library_id = None;
				}
			} else {
				println!(
					"📭 No library selected. Use: {}",
					"spacedrive library open <path>".bright_cyan()
				);
			}
		}

		LibraryCommands::Close => {
			if let Some(lib_id) = state.current_library_id {
				core.libraries.close_library(lib_id).await?;
				state.current_library_id = None;
				println!("✅ Library closed");
			} else {
				println!("📭 No library is currently open");
			}
		}
	}

	Ok(())
}

pub async fn handle_location_command(
	cmd: LocationCommands,
	core: &Core,
	state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	// Ensure we have a current library
	let library = get_current_library(core, state).await?;

	match cmd {
		LocationCommands::Add { path, name, mode } => {
			println!(
				"📍 Adding location: {}...",
				path.display().to_string().bright_blue()
			);

			// Get the action manager from core context
			let action_manager = core
				.context
				.get_action_manager()
				.await
				.ok_or("Action manager not available")?;

			// Convert CliIndexMode to IndexMode
			let action_mode = match mode {
				CliIndexMode::Shallow => IndexMode::Shallow,
				CliIndexMode::Content => IndexMode::Content,
				CliIndexMode::Deep => IndexMode::Deep,
			};

			// Create the action
			let action = Action::LocationAdd {
				library_id: library.id(),
				action: crate::operations::locations::add::action::LocationAddAction {
					path: path.clone(),
					name: name.clone(),
					mode: action_mode,
				}
			};

			// Dispatch the action
			match action_manager.dispatch(action).await {
				Ok(receipt) => {
					if let Some(payload) = receipt.result_payload {
						if let Some(location_id) =
							payload.get("location_id").and_then(|v| v.as_str())
						{
							println!("✅ Location added successfully!");
							println!("   ID: {}", location_id.bright_yellow());
							println!(
								"   Name: {}",
								name.unwrap_or_else(|| path
									.file_name()
									.unwrap()
									.to_string_lossy()
									.to_string())
									.bright_cyan()
							);
							println!("   Path: {}", path.display().to_string().bright_blue());

							if receipt.job_handle.is_some() {
								println!(
									"   Status: {} (job dispatched)",
									"Indexing".bright_yellow()
								);
								println!(
									"\n💡 Tip: Monitor indexing progress with: {}",
									"spacedrive job monitor".bright_cyan()
								);
							} else {
								println!("   Status: {}", "Ready".bright_green());
							}
						} else {
							println!("✅ Location addition initiated!");
							println!(
								"   Action ID: {}",
								receipt.action_id.to_string().bright_yellow()
							);
						}
					} else if receipt.job_handle.is_some() {
						println!("✅ Location addition job started!");
						println!(
							"   Action ID: {}",
							receipt.action_id.to_string().bright_yellow()
						);
						println!(
							"\n💡 Tip: Monitor progress with: {}",
							"spacedrive job monitor".bright_cyan()
						);
					}
				}
				Err(e) => {
					println!("❌ Failed to add location: {}", e);
					return Err(Box::new(e));
				}
			}
		}

		LocationCommands::List => {
			let db = library.db();
			let locations = entities::location::Entity::find().all(db.conn()).await?;

			if locations.is_empty() {
				println!(
					"📭 No locations found. Add one with: {}",
					"spacedrive location add <path>".bright_cyan()
				);
				return Ok(());
			}

			let mut table = Table::new();
			table.load_preset(UTF8_FULL).set_header(vec![
				"ID", "Name", "Path", "Mode", "Status", "Files", "Size",
			]);

			for location in locations {
				let status_color = match location.scan_state.as_str() {
					"pending" => "Pending".bright_yellow(),
					"scanning" => "Scanning".bright_blue(),
					"complete" => "Complete".bright_green(),
					"error" => "Error".bright_red(),
					"paused" => "Paused".bright_magenta(),
					_ => "Unknown".normal(),
				};

				let size_str = format_bytes(location.total_byte_size as u64);

				table.add_row(vec![
					Cell::new(location.id),
					Cell::new(location.name.unwrap_or_default()),
					Cell::new(location.path),
					Cell::new(location.index_mode),
					Cell::new(status_color),
					Cell::new(location.total_file_count),
					Cell::new(size_str),
				]);
			}

			println!("{}", table);
		}

		LocationCommands::Remove { identifier } => {
			println!("🗑️  Removing location {}...", identifier.bright_red());
			// TODO: Implement location removal
			println!("⚠️  Location removal not yet implemented");
		}

		LocationCommands::Rescan { identifier, force } => {
			println!("🔄 Rescanning location {}...", identifier.bright_blue());
			if force {
				println!(
					"   Mode: {} (ignoring change detection)",
					"Full scan".bright_yellow()
				);
			}
			// TODO: Implement rescan
			println!("⚠️  Location rescan not yet implemented");
		}

		LocationCommands::Info { identifier } => {
			let db = library.db();

			// Try to find by ID first, then by path
			let location = if let Ok(id) = identifier.parse::<i32>() {
				entities::location::Entity::find_by_id(id)
					.one(db.conn())
					.await?
			} else {
				entities::location::Entity::find()
					.filter(entities::location::Column::Path.contains(&identifier))
					.one(db.conn())
					.await?
			};

			match location {
				Some(loc) => {
					println!("📍 Location Details");
					println!("   ID: {}", loc.id.to_string().bright_yellow());
					println!("   Name: {}", loc.name.unwrap_or_default().bright_cyan());
					println!("   Path: {}", loc.path.bright_blue());
					println!("   Mode: {}", loc.index_mode.bright_magenta());
					println!(
						"   Status: {}",
						match loc.scan_state.as_str() {
							"complete" => loc.scan_state.bright_green(),
							"scanning" => loc.scan_state.bright_blue(),
							"error" => loc.scan_state.bright_red(),
							_ => loc.scan_state.normal(),
						}
					);
					println!(
						"   Files: {}",
						loc.total_file_count.to_string().bright_white()
					);
					println!(
						"   Size: {}",
						format_bytes(loc.total_byte_size as u64).bright_white()
					);

					if let Some(last_scan) = loc.last_scan_at {
						println!("   Last scan: {}", last_scan.to_string().bright_white());
					}

					if let Some(error) = loc.error_message {
						println!("   Error: {}", error.bright_red());
					}
				}
				None => {
					println!("❌ Location not found: {}", identifier.bright_red());
				}
			}
		}
	}

	Ok(())
}

pub async fn handle_job_command(
	cmd: JobCommands,
	core: &Core,
	state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	let library = get_current_library(core, state).await?;

	match cmd {
		JobCommands::List { status, recent } => {
			let status_filter = status.and_then(|s| match s.to_lowercase().as_str() {
				"running" => Some(JobStatus::Running),
				"completed" => Some(JobStatus::Completed),
				"failed" => Some(JobStatus::Failed),
				"paused" => Some(JobStatus::Paused),
				"cancelled" => Some(JobStatus::Cancelled),
				_ => None,
			});

			let jobs = library.jobs().list_jobs(status_filter).await?;

			if jobs.is_empty() {
				println!("📭 No jobs found");
				return Ok(());
			}

			let mut table = Table::new();
			table.load_preset(UTF8_FULL).set_header(vec![
				"ID", "Type", "Status", "Progress", "Started", "Duration",
			]);

			let display_jobs = if recent {
				jobs.into_iter().take(10).collect()
			} else {
				jobs
			};

			for job in display_jobs {
				let status_color = match job.status {
					JobStatus::Running => "Running".bright_blue(),
					JobStatus::Completed => "Completed".bright_green(),
					JobStatus::Failed => "Failed".bright_red(),
					JobStatus::Paused => "Paused".bright_yellow(),
					JobStatus::Cancelled => "Cancelled".bright_magenta(),
					_ => "Unknown".normal(),
				};

				let progress = format!("{:.1}%", job.progress * 100.0);
				let duration = if let Some(completed) = job.completed_at {
					let duration = completed - job.started_at;
					format!("{:.1}s", duration.num_seconds())
				} else {
					let duration = chrono::Utc::now() - job.started_at;
					format!("{:.1}s", duration.num_seconds())
				};

				table.add_row(vec![
					Cell::new(job.id.to_string().chars().take(8).collect::<String>()),
					Cell::new(job.name),
					Cell::new(status_color),
					Cell::new(progress),
					Cell::new(job.started_at.format("%H:%M:%S")),
					Cell::new(duration),
				]);
			}

			println!("{}", table);

			if recent {
				println!(
					"\n💡 Showing recent jobs. Use without {} to see all",
					"--recent".bright_cyan()
				);
			}
		}

		JobCommands::Info { id } => {
			if let Some(job) = library.jobs().get_job_info(id).await? {
				println!("💼 Job Details");
				println!("   ID: {}", job.id.to_string().bright_yellow());
				println!("   Type: {}", job.name.bright_cyan());
				println!(
					"   Status: {}",
					match job.status {
						JobStatus::Running => "Running".bright_blue(),
						JobStatus::Completed => "Completed".bright_green(),
						JobStatus::Failed => "Failed".bright_red(),
						JobStatus::Paused => "Paused".bright_yellow(),
						JobStatus::Cancelled => "Cancelled".bright_magenta(),
						_ => "Unknown".normal(),
					}
				);
				println!("   Progress: {:.1}%", job.progress * 100.0);
				println!("   Started: {}", job.started_at.format("%Y-%m-%d %H:%M:%S"));

				if let Some(completed) = job.completed_at {
					println!("   Completed: {}", completed.format("%Y-%m-%d %H:%M:%S"));
					let duration = completed - job.started_at;
					println!("   Duration: {:.1}s", duration.num_seconds());
				}

				if let Some(error) = job.error_message {
					println!("   Error: {}", error.bright_red());
				}
			} else {
				println!("❌ Job not found: {}", id.to_string().bright_red());
			}
		}

		JobCommands::Monitor { job_id } => {
			if let Some(id) = job_id {
				// Monitor specific job
				if let Ok(_uuid) = id.parse::<Uuid>() {
					println!(
						"📊 Monitoring job {}...",
						id.chars().take(8).collect::<String>().bright_yellow()
					);
					// TODO: Implement single job monitoring
					println!("⚠️  Single job monitoring not yet implemented. Showing all jobs:");
				}
			}
			super::monitor::run_monitor(core).await?;
		}

		JobCommands::Pause { id } => {
			println!("⏸️  Pausing job {}...", id.to_string().bright_yellow());
			// TODO: Implement job pause
			println!("⚠️  Job pause not yet implemented");
		}

		JobCommands::Resume { id } => {
			println!("▶️  Resuming job {}...", id.to_string().bright_blue());
			// TODO: Implement job resume
			println!("⚠️  Job resume not yet implemented");
		}

		JobCommands::Cancel { id } => {
			println!("❌ Cancelling job {}...", id.to_string().bright_red());
			// TODO: Implement job cancel
			println!("⚠️  Job cancel not yet implemented");
		}
	}

	Ok(())
}

pub async fn handle_network_command(
	cmd: NetworkCommands,
	_core: &Core,
	_state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand};

	let client = DaemonClient::new();

	// Check if daemon is running for most commands
	match &cmd {
		NetworkCommands::Init { .. } => {
			// Init doesn't require daemon to be running
		}
		_ => {
			if !client.is_running() {
				println!(
					"{} Daemon is not running. Start it with: {}",
					"✗".red(),
					"spacedrive start".bright_blue()
				);
				return Ok(());
			}
		}
	}

	match cmd {
		NetworkCommands::Init => match client.send_command(DaemonCommand::InitNetworking).await? {
			crate::infrastructure::cli::daemon::DaemonResponse::Ok => {
				println!("{} Networking initialized successfully", "✓".green());
			}
			crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
				println!("{} {}", "✗".red(), err);
			}
			_ => {
				println!("{} Unexpected response", "✗".red());
			}
		},

		NetworkCommands::Start => {
			match client.send_command(DaemonCommand::StartNetworking).await? {
				crate::infrastructure::cli::daemon::DaemonResponse::Ok => {
					println!("{} Networking service started", "✓".green());
				}
				crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		NetworkCommands::Stop => match client.send_command(DaemonCommand::StopNetworking).await? {
			crate::infrastructure::cli::daemon::DaemonResponse::Ok => {
				println!("{} Networking service stopped", "✓".green());
			}
			crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
				println!("{} {}", "✗".red(), err);
			}
			_ => {
				println!("{} Unexpected response", "✗".red());
			}
		},

		NetworkCommands::Devices => {
			match client
				.send_command(DaemonCommand::ListConnectedDevices)
				.await?
			{
				crate::infrastructure::cli::daemon::DaemonResponse::ConnectedDevices(devices) => {
					if devices.is_empty() {
						println!("📭 No devices currently connected");
						println!();
						println!("💡 To connect devices:");
						println!("   • Generate pairing code: spacedrive network pair generate");
						println!("   • Join with code: spacedrive network pair join \"<code>\"");
					} else {
						println!("🌐 Connected Devices ({}):", devices.len());
						println!();

						let mut table = Table::new();
						table.load_preset(UTF8_FULL);
						table.set_header(vec![
							"Device ID",
							"Name",
							"Type",
							"OS",
							"App Version",
							"Peer ID",
							"Status",
							"Active",
							"Last Seen",
						]);

						for device in &devices {
							let status_color = if device.connection_active {
								"🟢 Connected".green()
							} else {
								"🔴 Disconnected".red()
							};

							let active_indicator = if device.connection_active {
								"✓".green()
							} else {
								"✗".red()
							};

							table.add_row(vec![
								Cell::new(&format!("{}...", &device.device_id.to_string()[..8])),
								Cell::new(&device.device_name),
								Cell::new(&device.device_type),
								Cell::new(&device.os_version),
								Cell::new(&device.app_version),
								Cell::new(&format!(
									"{}...",
									&device.peer_id[..std::cmp::min(8, device.peer_id.len())]
								)),
								Cell::new(&status_color.to_string()),
								Cell::new(&active_indicator.to_string()),
								Cell::new(&device.last_seen),
							]);
						}

						println!("{}", table);

						// Show summary stats if we have active connections
						let active_connections =
							devices.iter().filter(|d| d.connection_active).count();
						if active_connections > 0 {
							println!();
							println!("📊 Connection Summary:");
							println!(
								"   • Active connections: {}/{}",
								active_connections,
								devices.len()
							);

							let total_sent: u64 = devices.iter().map(|d| d.bytes_sent).sum();
							let total_received: u64 =
								devices.iter().map(|d| d.bytes_received).sum();

							if total_sent > 0 || total_received > 0 {
								println!(
									"   • Data transferred: {} sent, {} received",
									format_bytes(total_sent),
									format_bytes(total_received)
								);
							}
						}
					}
				}
				crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
					println!("{} Failed to list devices: {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response from daemon", "✗".red());
				}
			}
		}

		NetworkCommands::Revoke { device_id } => {
			match client
				.send_command(DaemonCommand::RevokeDevice { device_id })
				.await?
			{
				crate::infrastructure::cli::daemon::DaemonResponse::Ok => {
					println!("{} Device {} revoked", "✓".green(), device_id);
				}
				crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		NetworkCommands::Spacedrop {
			device_id,
			file_path,
			sender,
			message,
		} => {
			match client
				.send_command(DaemonCommand::SendSpacedrop {
					device_id,
					file_path: file_path.to_string_lossy().to_string(),
					sender_name: sender,
					message,
				})
				.await?
			{
				crate::infrastructure::cli::daemon::DaemonResponse::SpacedropStarted {
					transfer_id,
				} => {
					println!(
						"{} Spacedrop started with transfer ID: {}",
						"✓".green(),
						transfer_id
					);
				}
				crate::infrastructure::cli::daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		NetworkCommands::Pair { action } => {
			// Convert from legacy PairingCommands to new PairingAction
			let action = match action {
				PairingCommands::Generate => {
					crate::infrastructure::cli::networking_commands::PairingAction::Generate
				}
				PairingCommands::Join { code } => {
					let code = match code {
						Some(c) => c,
						None => {
							use dialoguer::Input;
							Input::new()
								.with_prompt("Enter the 12-word pairing code")
								.interact_text()?
						}
					};
					crate::infrastructure::cli::networking_commands::PairingAction::Join { code }
				}
				PairingCommands::Status => {
					crate::infrastructure::cli::networking_commands::PairingAction::Status
				}
				PairingCommands::ListPending => {
					crate::infrastructure::cli::networking_commands::PairingAction::List
				}
				PairingCommands::Accept { request_id } => {
					crate::infrastructure::cli::networking_commands::PairingAction::Accept {
						request_id,
					}
				}
				PairingCommands::Reject { request_id } => {
					crate::infrastructure::cli::networking_commands::PairingAction::Reject {
						request_id,
					}
				}
			};
			crate::infrastructure::cli::networking_commands::handle_pairing_command(
				action, &client,
			)
			.await?;
		}
	}

	Ok(())
}

pub async fn handle_legacy_scan_command(
	path: PathBuf,
	mode: CliIndexMode,
	watch: bool,
	core: &Core,
	state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	let library = get_current_library(core, state).await?;

	println!("🔍 Starting indexing job...");
	println!("   Path: {}", path.display().to_string().bright_blue());
	println!("   Mode: {}", format!("{:?}", mode).bright_magenta());

	// Get device from database
	let db = library.db();
	let device = core.device.to_device()?;

	let device_record = entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
		.ok_or("Device not registered in database")?;

	// Create location and start indexing
	let location_args = LocationCreateArgs {
		path: path.clone(),
		name: Some(path.file_name().unwrap().to_string_lossy().to_string()),
		index_mode: mode.into(),
	};

	let location_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	println!("✅ Indexing job started!");
	println!(
		"   Location ID: {}",
		location_id.to_string().bright_yellow()
	);

	if watch {
		println!("\n📡 Monitoring job progress...\n");
		super::monitor::run_monitor(core).await?;
	} else {
		println!(
			"\n💡 Monitor progress with: {}",
			"spacedrive job monitor".bright_cyan()
		);
	}

	Ok(())
}

pub async fn handle_status_command(
	core: &Core,
	state: &CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	println!("🚀 {} Status", "Spacedrive CLI".bright_cyan());
	println!("═══════════════════════════════════════");

	// Device info
	let device = core.device.to_device()?;
	println!("\n📱 Device");
	println!("   ID: {}", device.id.to_string().bright_yellow());
	println!("   Name: {}", device.name.bright_white());
	println!(
		"   OS: {} {}",
		device.os,
		device.hardware_model.as_deref().unwrap_or("")
	);

	// Current library
	println!("\n📚 Library");
	if let Some(lib_id) = &state.current_library_id {
		let libraries = core.libraries.list().await;
		if let Some(library) = libraries.into_iter().find(|lib| lib.id() == *lib_id) {
			println!("   Current: {}", library.name().await.bright_cyan());
			println!("   ID: {}", lib_id.to_string().bright_yellow());
			println!(
				"   Path: {}",
				library.path().display().to_string().bright_blue()
			);

			// Get stats
			let db = library.db();
			let entry_count = entities::entry::Entity::find()
				.count(db.conn())
				.await
				.unwrap_or(0);
			let location_count = entities::location::Entity::find()
				.count(db.conn())
				.await
				.unwrap_or(0);

			println!("   Entries: {}", entry_count.to_string().bright_white());
			println!(
				"   Locations: {}",
				location_count.to_string().bright_white()
			);
		} else {
			println!("   ⚠️  Current library no longer exists");
		}
	} else {
		println!("   📭 No library selected");
	}

	// System info
	println!("\n🖥️  System");
	println!("   Event subscribers: {}", core.events.subscriber_count());
	println!("   Libraries loaded: {}", core.libraries.list().await.len());

	Ok(())
}

pub async fn handle_index_command(
	cmd: IndexCommands,
	core: &Core,
	state: &CliState,
) -> Result<(), Box<dyn std::error::Error>> {
	use crate::{
		operations::indexing::{
			IndexMode as JobIndexMode, IndexPersistence, IndexScope as JobIndexScope, IndexerJob,
			IndexerJobConfig,
		},
		shared::types::SdPath,
	};

	match cmd {
		IndexCommands::QuickScan {
			path,
			scope,
			ephemeral,
		} => {
			if !path.exists() {
				return Err(format!("Path does not exist: {}", path.display()).into());
			}

			println!(
				"🔍 {} scan of {}",
				if ephemeral {
					"Ephemeral quick"
				} else {
					"Quick"
				},
				path.display().to_string().bright_cyan()
			);
			println!("   Scope: {}", format!("{:?}", scope).bright_yellow());

			// Create SdPath - for demo we'll use a nil device UUID
			let device = core.device.to_device()?;
			let sd_path = SdPath::new(device.id, path);

			let job = if ephemeral {
				IndexerJob::ephemeral_browse(sd_path, scope.into())
			} else {
				// Need a library for persistent jobs
				let library = get_current_library(core, state).await?;
				// For quick scan, we'll create a UI navigation job
				IndexerJob::ui_navigation(library.id(), sd_path)
			};

			// Dispatch the job
			let library = if ephemeral {
				// Use a temporary library for ephemeral jobs - in practice this should be handled differently
				get_current_library(core, state).await?
			} else {
				get_current_library(core, state).await?
			};

			let handle = library.jobs().dispatch(job).await?;

			println!("✅ Quick scan job started!");
			println!("   Job ID: {}", handle.id().to_string().bright_yellow());

			if ephemeral {
				println!("   Mode: Ephemeral (no database writes)");
			}

			println!(
				"\n💡 Monitor progress with: {}",
				"spacedrive job monitor".bright_cyan()
			);
		}

		IndexCommands::Browse {
			path,
			scope,
			content,
		} => {
			if !path.exists() {
				return Err(format!("Path does not exist: {}", path.display()).into());
			}

			println!(
				"🌐 Browsing {} (scope: {:?})",
				path.display().to_string().bright_cyan(),
				scope
			);

			let device = core.device.to_device()?;
			let sd_path = SdPath::new(device.id, path);

			// Create ephemeral job with appropriate mode
			let mut config = IndexerJobConfig::ephemeral_browse(sd_path, scope.into());
			if content {
				config.mode = JobIndexMode::Content;
				println!("   Content analysis: {}", "Enabled".bright_green());
			}

			let job = IndexerJob::new(config);

			// For browsing, we still need a library context but results won't be persisted
			let library = get_current_library(core, state).await?;
			let handle = library.jobs().dispatch(job).await?;

			println!("✅ Browse job started!");
			println!("   Job ID: {}", handle.id().to_string().bright_yellow());
			println!("   Mode: Ephemeral browsing");

			println!(
				"\n💡 Monitor progress with: {}",
				"spacedrive job monitor".bright_cyan()
			);
		}

		IndexCommands::Location {
			identifier,
			mode,
			scope,
		} => {
			let library = get_current_library(core, state).await?;

			// Find location by ID or path
			let locations = entities::location::Entity::find()
				.all(library.db().conn())
				.await?;

			let location = locations
				.into_iter()
				.find(|loc| {
					// Try to match by UUID first
					if let Ok(uuid) = identifier.parse::<Uuid>() {
						loc.uuid == uuid
					} else {
						// Match by path
						loc.path == identifier
					}
				})
				.ok_or_else(|| format!("Location not found: {}", identifier))?;

			println!(
				"📂 Indexing location: {}",
				location.name.as_deref().unwrap_or("Unnamed").bright_cyan()
			);
			println!("   Path: {}", location.path.bright_blue());
			println!("   Mode: {:?}", mode);
			println!("   Scope: {:?}", scope);

			let device = core.device.to_device()?;
			let sd_path = SdPath::new(device.id, PathBuf::from(&location.path));

			// Create appropriate job configuration
			let mut config = IndexerJobConfig::new(location.uuid, sd_path, mode.into());
			config.scope = scope.into();

			let job = IndexerJob::new(config);
			let handle = library.jobs().dispatch(job).await?;

			println!("✅ Location indexing job started!");
			println!("   Job ID: {}", handle.id().to_string().bright_yellow());
			println!("   Location: {}", location.uuid.to_string().bright_yellow());

			println!(
				"\n💡 Monitor progress with: {}",
				"spacedrive job monitor".bright_cyan()
			);
		}
	}

	Ok(())
}

async fn get_current_library(
	core: &Core,
	state: &CliState,
) -> Result<Arc<Library>, Box<dyn std::error::Error>> {
	if let Some(lib_id) = &state.current_library_id {
		let libraries = core.libraries.list().await;
		libraries
			.into_iter()
			.find(|lib| lib.id() == *lib_id)
			.ok_or_else(|| "Current library not found. Please select a library.".into())
	} else {
		Err("No library selected. Use 'spacedrive library open <path>' or 'spacedrive library create <name>'.".into())
	}
}

fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_index = 0;

	while size >= 1024.0 && unit_index < UNITS.len() - 1 {
		size /= 1024.0;
		unit_index += 1;
	}

	format!("{:.2} {}", size, UNITS[unit_index])
}
