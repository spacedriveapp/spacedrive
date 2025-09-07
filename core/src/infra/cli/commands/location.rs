//! Location management commands
//!
//! This module handles CLI commands for managing locations:
//! - Adding new locations to libraries
//! - Listing existing locations
//! - Removing locations
//! - Rescanning locations for changes

use crate::infra::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infra::cli::output::messages::LocationInfo as OutputLocationInfo;
use crate::infra::cli::output::{CliOutput, Message};
use clap::{Subcommand, ValueEnum};
use comfy_table::Table;
use std::path::PathBuf;
use uuid::Uuid;

// Re-export from the commands module for consistency
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexMode {
	/// Only metadata (fast)
	Shallow,
	/// Metadata + content hashing
	Content,
	/// Full analysis including media metadata
	Deep,
}

#[derive(Subcommand, Clone, Debug)]
pub enum LocationCommands {
	/// Add a new location to the current library
	Add {
		/// Path to add as a location
		path: PathBuf,
		/// Custom name for the location
		#[arg(short, long)]
		name: Option<String>,
		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,
	},

	/// List all locations in the current library
	List,

	/// Get information about a specific location
	Info {
		/// Location ID or path
		identifier: String,
	},

	/// Remove a location from the library
	Remove {
		/// Location ID or path
		identifier: String,
		/// Skip confirmation prompt
		#[arg(short, long)]
		yes: bool,
	},

	/// Rescan a location for changes
	Rescan {
		/// Location ID or path
		identifier: String,
		/// Force full rescan
		#[arg(short, long)]
		force: bool,
	},
}

pub async fn handle_location_command(
	cmd: LocationCommands,
	instance_name: Option<String>,
	mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut client = DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		LocationCommands::Add { path, name, mode } => {
			output.info(&format!("Adding location {}...", path.display()))?;

			let response = client
				.send_command(DaemonCommand::AddLocation {
					path: path.clone(),
					name,
				})
				.await;

			match response {
				Ok(DaemonResponse::LocationAdded {
					location_id,
					job_id,
				}) => {
					output.print(Message::LocationAdded {
						path: path.clone(),
						id: location_id,
					})?;

					if !job_id.is_empty() {
						output
							.section()
							.item("Job ID", &job_id.chars().take(8).collect::<String>())
							.empty_line()
							.text("Indexing started...")
							.help()
							.item("To monitor detailed progress, run: spacedrive job monitor")
							.end()
							.render()?;

						// Show basic progress by checking job status periodically
						let mut last_status = String::new();
						for _ in 0..10 {
							// Check for 10 seconds
							tokio::time::sleep(std::time::Duration::from_secs(1)).await;

							if let Ok(uuid) = job_id.parse::<uuid::Uuid>() {
								match client
									.send_command(DaemonCommand::GetJobInfo { id: uuid })
									.await
								{
									Ok(DaemonResponse::JobInfo(Some(job))) => {
										if job.status != last_status {
											output.print(Message::JobProgress {
												id: uuid,
												name: "Indexing".to_string(),
												progress: job.progress,
												message: Some(job.status.clone()),
											})?;
											last_status = job.status.clone();
										}

										if job.status == "completed" || job.status == "failed" {
											break;
										}
									}
									_ => {}
								}
							}
						}
					} else {
						output.warning("Location added but indexing failed to start")?;
					}
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to add location: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				Ok(resp) => {
					eprintln!("DEBUG: Got unexpected response: {:?}", resp);
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LocationCommands::List => {
			match client.send_command(DaemonCommand::ListLocations).await {
				Ok(DaemonResponse::Locations(locations)) => {
					if locations.is_empty() {
						output.info(
							"No locations found. Add one with: spacedrive location add <path>",
						)?;
					} else {
						let output_locations: Vec<OutputLocationInfo> = locations
							.into_iter()
							.map(|loc| OutputLocationInfo {
								id: loc.id,
								path: loc.path,
								indexed_files: 0, // TODO: Get actual count from daemon
							})
							.collect();

						if matches!(
							output.format(),
							crate::infra::cli::output::OutputFormat::Json
						) {
							output.print(Message::LocationList {
								locations: output_locations,
							})?;
						} else {
							// For human output, use a table
							let mut table = Table::new();
							table.set_header(vec!["ID", "Path", "Files"]);

							for loc in output_locations {
								table.add_row(vec![
									loc.id.to_string(),
									loc.path.display().to_string(),
									loc.indexed_files.to_string(),
								]);
							}

							output.section().table(table).render()?;
						}
					}
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to list locations: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LocationCommands::Remove { identifier, yes } => {
			if !yes {
				use dialoguer::Confirm;
				let confirm = Confirm::new()
					.with_prompt(format!(
						"Are you sure you want to remove location '{}'?",
						identifier
					))
					.default(false)
					.interact()?;

				if !confirm {
					output.info("Operation cancelled")?;
					return Ok(());
				}
			}
			output.info(&format!("Removing location {}...", identifier))?;

			// Try to parse as UUID
			let id = match identifier.parse::<Uuid>() {
				Ok(id) => id,
				Err(_) => {
					output.error(Message::Error(format!(
						"Invalid location ID: {}",
						identifier
					)))?;
					return Ok(());
				}
			};

			match client
				.send_command(DaemonCommand::RemoveLocation { id })
				.await
			{
				Ok(DaemonResponse::Ok) => {
					output.success("Location removed successfully")?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to remove location: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LocationCommands::Rescan { identifier, force } => {
			output.info(&format!("Rescanning location {}...", identifier))?;

			// Try to parse as UUID
			let id = match identifier.parse::<Uuid>() {
				Ok(id) => id,
				Err(_) => {
					output.error(Message::Error(format!(
						"Invalid location ID: {}",
						identifier
					)))?;
					return Ok(());
				}
			};

			match client
				.send_command(DaemonCommand::RescanLocation { id })
				.await
			{
				Ok(DaemonResponse::Ok) => {
					output.success("Rescan started successfully")?;
					output.info("Use 'spacedrive job monitor' to track progress")?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to rescan location: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LocationCommands::Info { identifier } => {
			output.error(Message::Error(
				"Location info command not yet implemented".to_string(),
			))?;
		}
	}

	Ok(())
}
