//! Volume CLI commands

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::output::{CliOutput, Message};
use clap::Subcommand;
use comfy_table::Table;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Volume management commands
#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum VolumeCommands {
	/// List all volumes
	List,
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
		VolumeCommands::List => {
			output.info("Fetching volumes...")?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::List))
				.await
			{
				Ok(DaemonResponse::VolumeListWithTracking(volume_infos)) => {
					if volume_infos.is_empty() {
						output.info("No volumes found")?;
					} else {
						output.info(&format!("Found {} volume(s):", volume_infos.len()))?;

						let mut table = Table::new();
						table.set_header(vec![
							"Name",
							"Mount Point",
							"File System",
							"Capacity",
							"Available",
							"Status",
							"Tracked",
						]);

						for volume_info in volume_infos {
							let volume = volume_info["volume"].as_object().unwrap();
							let is_tracked = volume_info["is_tracked"].as_bool().unwrap_or(false);
							let tracked_name = volume_info["tracked_name"].as_str();

							let name = volume["name"].as_str().unwrap_or("Unknown");
							let mount_point = volume["mount_point"].as_str().unwrap_or("Unknown");
							let file_system = volume["file_system"].as_str().unwrap_or("Unknown");
							let total_capacity =
								volume["total_bytes_capacity"].as_u64().unwrap_or(0);
							let available_space =
								volume["total_bytes_available"].as_u64().unwrap_or(0);
							let is_mounted = volume["is_mounted"].as_bool().unwrap_or(false);

							// Format capacity in a human-readable way
							let capacity_str = if total_capacity > 0 {
								format_bytes(total_capacity)
							} else {
								"Unknown".to_string()
							};

							let available_str = if available_space > 0 {
								format_bytes(available_space)
							} else {
								"Unknown".to_string()
							};

							let status = if is_mounted { "Mounted" } else { "Unmounted" };

							let tracked_status = if is_tracked {
								if let Some(custom_name) = tracked_name {
									format!("Yes ({})", custom_name)
								} else {
									"Yes".to_string()
								}
							} else {
								"No".to_string()
							};

							table.add_row(vec![
								name.to_string(),
								mount_point.to_string(),
								file_system.to_string(),
								capacity_str,
								available_str,
								status.to_string(),
								tracked_status,
							]);
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
			let name_display = name.as_deref().unwrap_or("(no custom name)");
			output.info(&format!(
				"Tracking volume {} as '{}'...",
				fingerprint, name_display
			))?;

			match client
				.send_command(DaemonCommand::Volume(VolumeCommands::Track {
					fingerprint: fingerprint.clone(),
					name: name.clone(),
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
