//! Volume CLI commands

use crate::infra::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infra::cli::output::{CliOutput, Message};
use crate::volume::types::VolumeFingerprint;
use clap::Subcommand;
use comfy_table::Table;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, clap::ValueEnum, Serialize, Deserialize)]
pub enum VolumeTypeFilter {
	Primary,
	UserData,
	External,
	Secondary,
	System,
	Network,
	Unknown,
}

/// Volume management commands
#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum VolumeCommands {
	/// List all volumes
	List {
		/// Include system volumes (hidden by default)
		#[arg(long)]
		include_system: bool,

		/// Filter by volume type
		#[arg(long, value_enum)]
		type_filter: Option<VolumeTypeFilter>,

		/// Show volume type classifications
		#[arg(long)]
		show_types: bool,

		/// Include tracked volumes that are currently offline/disconnected
		#[arg(long)]
		show_offline: bool,
	},
	/// Show details for a specific volume
	Get {
		/// Volume fingerprint
		fingerprint: String,
	},
	/// Track a volume in a library
	Track {
		/// Volume fingerprint
		fingerprint: String,
		/// Optional name for the tracked volume
		#[arg(short, long)]
		name: Option<String>,
	},
	/// Untrack a volume from a library
	Untrack {
		/// Volume fingerprint
		fingerprint: String,
	},
	/// Run speed test on a volume
	SpeedTest {
		/// Volume fingerprint
		fingerprint: String,
	},
	/// Refresh volume list
	Refresh,
	/// Fix empty display names for tracked volumes
	FixNames,
}

pub async fn handle_volume_command(
	cmd: VolumeCommands,
	instance_name: Option<String>,
	mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut client = DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		VolumeCommands::List {
			include_system,
			type_filter,
			show_types,
			show_offline,
		} => {
			output.info("Fetching volumes...")?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::List {
					include_system,
					type_filter: type_filter.clone(),
					show_types,
					show_offline,
				}))
				.await
			{
				Ok(DaemonResponse::VolumeListWithTracking(volume_items)) => {
					if volume_items.is_empty() {
						output.info("No volumes found")?;
					} else {
						output.info(&format!("Found {} volume(s):", volume_items.len()))?;

						let mut table = Table::new();
						let mut headers = vec![
							"ID",
							"Name",
							"Mount Point",
							"File System",
							"Capacity",
							"Available",
							"Status",
							"Tracked",
						];

						if show_types {
							headers.insert(4, "Type");
						}

						table.set_header(headers);

						for volume_item in volume_items {
							let volume = &volume_item.volume;
							let is_tracked = volume_item.is_tracked;
							let tracked_name = volume_item.tracked_name.as_deref();
							let is_online = volume_item.is_online;

							// Apply filtering based on CLI flags
							let is_user_visible = volume.is_user_visible;
							let volume_type_str = format!("{:?}", volume.volume_type);

							// Skip system volumes unless --include-system is specified
							if !include_system && !is_user_visible {
								continue;
							}

							// Apply type filter if specified
							if let Some(ref filter) = type_filter {
								let filter_str = format!("{:?}", filter);
								if volume_type_str != filter_str {
									continue;
								}
							}

							let name = &volume.name;
							let mount_point = volume.mount_point.to_string_lossy();
							let file_system = format!("{:?}", volume.file_system);
							let total_capacity = volume.total_bytes_capacity;
							let available_space = volume.total_bytes_available;

							// Use is_online for tracked volumes, fall back to is_mounted for others
							let is_connected = if is_tracked {
								is_online
							} else {
								volume.is_mounted
							};

							// Get short ID from fingerprint
							let short_id = volume.fingerprint.short_id();

							// Format capacity in a human-readable way
							let capacity_str = if total_capacity > 0 {
								format_bytes(total_capacity)
							} else {
								"Unknown".to_string()
							};

							let available_str = if available_space > 0 {
								let formatted = format_bytes(available_space);
								if !is_connected {
									// For offline volumes, indicate the data is cached
									format!("{} (cached)", formatted)
								} else {
									formatted
								}
							} else {
								"Unknown".to_string()
							};

							let status = if is_connected { "Online" } else { "Offline" };

							let tracked_status = if is_tracked {
								if let Some(custom_name) = tracked_name {
									format!("Yes ({})", custom_name)
								} else {
									"Yes".to_string()
								}
							} else {
								"No".to_string()
							};

							let mut row = vec![
								short_id,
								name.to_string(),
								mount_point.to_string(),
								file_system,
							];

							if show_types {
								// Get display name for the volume type
								let type_display = match volume.volume_type {
									crate::volume::types::VolumeType::Primary => "[PRI]",
									crate::volume::types::VolumeType::UserData => "[USR]",
									crate::volume::types::VolumeType::External => "[EXT]",
									crate::volume::types::VolumeType::Secondary => "[SEC]",
									crate::volume::types::VolumeType::System => "[SYS]",
									crate::volume::types::VolumeType::Network => "[NET]",
									crate::volume::types::VolumeType::Unknown => "[UNK]",
								};
								row.push(type_display.to_string());
							}

							row.extend_from_slice(&[
								capacity_str,
								available_str,
								status.to_string(),
								tracked_status,
							]);

							table.add_row(row);
						}

						output.section().table(table).render()?;
					}
				}
				Ok(DaemonResponse::VolumeList(volumes)) => {
					// Fallback for when no current library is set
					if volumes.is_empty() {
						output.info("No volumes found")?;
					} else {
						output.info(&format!("Found {} volume(s):", volumes.len()))?;
						output.info("💡 Set a current library to see tracking status: spacedrive library switch <id>")?;

						let mut table = Table::new();
						table.set_header(vec![
							"Name",
							"Mount Point",
							"File System",
							"Capacity",
							"Available",
							"Status",
						]);

						for volume in volumes {
							let capacity_str = if volume.total_bytes_capacity > 0 {
								format_bytes(volume.total_bytes_capacity)
							} else {
								"Unknown".to_string()
							};

							let available_str = if volume.total_bytes_available > 0 {
								format_bytes(volume.total_bytes_available)
							} else {
								"Unknown".to_string()
							};

							let status = if volume.is_mounted {
								"Mounted"
							} else {
								"Unmounted"
							};

							table.add_row(vec![
								volume.name,
								volume.mount_point.display().to_string(),
								volume.file_system.to_string(),
								capacity_str,
								available_str,
								status.to_string(),
							]);
						}

						output.section().table(table).render()?;
					}
				}
				Ok(DaemonResponse::Error(msg)) => {
					output.error(Message::Error(msg))?;
				}
				Ok(_) => {
					output.error(Message::Error("Unexpected response".to_string()))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
			}
		}

		VolumeCommands::Get { fingerprint } => {
			output.info(&format!("Fetching volume {}...", fingerprint))?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::Get {
					fingerprint: fingerprint.clone(),
				}))
				.await
			{
				Ok(DaemonResponse::Volume(volume)) => {
					output.info(&format!("Volume: {}", volume.name))?;
					output.info(&format!("  Fingerprint: {}", volume.fingerprint.0))?;
					output.info(&format!("  Mount Point: {}", volume.mount_point.display()))?;
					output.info(&format!("  File System: {}", volume.file_system))?;
					output.info(&format!(
						"  Total Capacity: {} bytes",
						volume.total_bytes_capacity
					))?;
					output.info(&format!(
						"  Available Space: {} bytes",
						volume.total_bytes_available
					))?;
					output.info(&format!(
						"  Status: {}",
						if volume.is_mounted {
							"mounted"
						} else {
							"unmounted"
						}
					))?;
				}
				Ok(DaemonResponse::Error(msg)) => {
					output.error(Message::Error(msg))?;
				}
				Ok(_) => {
					output.error(Message::Error("Unexpected response".to_string()))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
			}
		}

		VolumeCommands::Track { fingerprint, name } => {
			let mut client = DaemonClient::new_with_instance(instance_name.clone());

			// Check if fingerprint looks like a short ID
			let resolved_fingerprint = if VolumeFingerprint::is_short_id(&fingerprint)
				|| VolumeFingerprint::is_medium_id(&fingerprint)
			{
				// TODO: Add short ID resolution via daemon
				output.info(&format!("🔍 Resolving short ID '{}'...", fingerprint))?;
				fingerprint.clone() // For now, pass through - daemon will handle resolution
			} else {
				fingerprint.clone()
			};

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::Track {
					fingerprint: resolved_fingerprint,
					name,
				}))
				.await
			{
				Ok(DaemonResponse::ActionOutput(action_output)) => {
					output.info(&format!("Successfully tracked volume {}", fingerprint))?;
					output.info(&format!("Action completed: {}", action_output))?;
				}
				Ok(DaemonResponse::Error(msg)) => {
					output.error(Message::Error(msg))?;
				}
				Ok(_) => {
					output.error(Message::Error("Unexpected response".to_string()))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
			}
		}

		VolumeCommands::Untrack { fingerprint } => {
			output.info(&format!("Untracking volume {}...", fingerprint))?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::Untrack {
					fingerprint: fingerprint.clone(),
				}))
				.await
			{
				Ok(DaemonResponse::ActionOutput(action_output)) => {
					output.info(&format!("Successfully untracked volume {}", fingerprint))?;
					output.info(&format!("Action completed: {}", action_output))?;
				}
				Ok(DaemonResponse::Error(msg)) => {
					output.error(Message::Error(msg))?;
				}
				Ok(_) => {
					output.error(Message::Error("Unexpected response".to_string()))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
			}
		}

		VolumeCommands::SpeedTest { fingerprint } => {
			output.info(&format!("Running speed test on volume {}...", fingerprint))?;
			output.info("This may take a moment...")?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::SpeedTest {
					fingerprint: fingerprint.clone(),
				}))
				.await
			{
				Ok(DaemonResponse::ActionOutput(action_output)) => {
					output.info(&format!("Speed test completed for volume {}", fingerprint))?;
					output.info(&format!("Action completed: {}", action_output))?;
				}
				Ok(DaemonResponse::Error(msg)) => {
					output.error(Message::Error(msg))?;
				}
				Ok(_) => {
					output.error(Message::Error("Unexpected response".to_string()))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
			}
		}

		VolumeCommands::Refresh => {
			let daemon_cmd = DaemonCommand::Volume(VolumeCommands::Refresh);
			let response = client.send_command(daemon_cmd).await?;

			match response {
				DaemonResponse::VolumeList(volumes) => {
					println!("ℹ️  Refreshed {} volume(s)", volumes.len());
				}
				DaemonResponse::Error(err) => {
					return Err(format!("Failed to refresh volumes: {}", err).into());
				}
				_ => {
					return Err("Unexpected response from daemon".into());
				}
			}
		}

		VolumeCommands::FixNames => {
			let daemon_cmd = DaemonCommand::Volume(VolumeCommands::FixNames);
			let response = client.send_command(daemon_cmd).await?;

			match response {
				DaemonResponse::Ok => {
					println!("✅ Fixed display names for tracked volumes");
				}
				DaemonResponse::Error(err) => {
					return Err(format!("Failed to fix display names: {}", err).into());
				}
				_ => {
					return Err("Unexpected response from daemon".into());
				}
			}
		}
	}

	Ok(())
}

/// Format bytes in a human-readable way
fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
	const THRESHOLD: u64 = 1024;

	if bytes == 0 {
		return "0 B".to_string();
	}

	let mut size = bytes as f64;
	let mut unit_index = 0;

	while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
		size /= THRESHOLD as f64;
		unit_index += 1;
	}

	if unit_index == 0 {
		format!("{} {}", bytes, UNITS[unit_index])
	} else {
		format!("{:.1} {}", size, UNITS[unit_index])
	}
}
