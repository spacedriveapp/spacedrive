//! Location management commands
//!
//! This module handles CLI commands for managing locations:
//! - Adding new locations to libraries
//! - Listing existing locations
//! - Removing locations
//! - Rescanning locations for changes

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use clap::{Subcommand, ValueEnum};
use colored::Colorize;
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
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

    match cmd {
        LocationCommands::Add { path, name, mode } => {
            println!(
                "📁 Adding location {}...",
                path.display().to_string().bright_blue()
            );

            match client
                .send_command(DaemonCommand::AddLocation {
                    path: path.clone(),
                    name,
                })
                .await
            {
                Ok(DaemonResponse::LocationAdded {
                    location_id,
                    job_id,
                }) => {
                    println!("✅ Location added successfully!");
                    println!("   Path: {}", path.display().to_string().bright_blue());
                    println!(
                        "   Location ID: {}",
                        location_id.to_string().bright_yellow()
                    );

                    if !job_id.is_empty() {
                        println!(
                            "   Job ID: {}",
                            job_id.chars().take(8).collect::<String>().bright_yellow()
                        );

                        // Automatically show brief progress info
                        println!("\n📊 Indexing started...");
                        println!(
                            "   To monitor detailed progress, run: {}",
                            "spacedrive job monitor".bright_cyan()
                        );

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
                                            println!(
                                                "   Status: {} ({}%)",
                                                job.status.bright_yellow(),
                                                (job.progress * 100.0) as u32
                                            );
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
                        println!("   Status: Location added but indexing failed to start");
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to add location: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LocationCommands::List => {
            match client
                .send_command(DaemonCommand::ListLocations)
                .await
            {
                Ok(DaemonResponse::Locations(locations)) => {
                    if locations.is_empty() {
                        println!(
                            "📭 No locations found. Add one with: spacedrive location add <path>"
                        );
                    } else {
                        let mut table = Table::new();
                        table.set_header(vec!["ID", "Name", "Path", "Status"]);

                        for loc in locations {
                            table.add_row(vec![
                                loc.id.to_string(),
                                loc.name,
                                loc.path.display().to_string(),
                                loc.status,
                            ]);
                        }

                        println!("{}", table);
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to list locations: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LocationCommands::Remove { identifier, yes } => {
            if !yes {
                use dialoguer::Confirm;
                let confirm = Confirm::new()
                    .with_prompt(format!("Are you sure you want to remove location '{}'?", identifier))
                    .default(false)
                    .interact()?;
                
                if !confirm {
                    println!("Operation cancelled");
                    return Ok(());
                }
            }
            println!("🗑️  Removing location {}...", identifier.bright_yellow());

            // Try to parse as UUID
            let id = match identifier.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    println!("❌ Invalid location ID: {}", identifier);
                    return Ok(());
                }
            };

            match client
                .send_command(DaemonCommand::RemoveLocation { id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Location removed successfully");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to remove location: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LocationCommands::Rescan { identifier, force } => {
            println!("🔄 Rescanning location {}...", identifier.bright_yellow());

            // Try to parse as UUID
            let id = match identifier.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    println!("❌ Invalid location ID: {}", identifier);
                    return Ok(());
                }
            };

            match client
                .send_command(DaemonCommand::RescanLocation { id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Rescan started successfully");
                    println!("   Use 'spacedrive job monitor' to track progress");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to rescan location: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LocationCommands::Info { identifier } => {
            println!("❌ Location info command not yet implemented");
        }
    }

    Ok(())
}