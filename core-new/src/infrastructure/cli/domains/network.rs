//! Network management commands
//!
//! This module handles CLI commands for networking operations:
//! - Initializing and managing networking services
//! - Device discovery and management
//! - Pairing operations with other devices
//! - Spacedrop file sharing

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::networking_commands;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Table};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Subcommand, Clone, Debug)]
pub enum NetworkCommands {
    /// Initialize networking using master key
    Init,

    /// Start networking services
    Start,

    /// Stop networking services  
    Stop,

    /// List discovered devices
    Devices,

    /// Pairing operations
    Pair {
        #[command(subcommand)]
        action: PairingCommands,
    },

    /// Revoke a paired device
    Revoke {
        /// Device ID to revoke
        device_id: String,
    },

    /// Spacedrop operations
    Spacedrop {
        /// Device ID to send to
        device_id: String,
        /// File path to send
        file_path: PathBuf,
        /// Sender name
        #[arg(short, long)]
        sender: Option<String>,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum PairingCommands {
    /// Generate a pairing code and wait for another device to connect (initiator)
    Generate,

    /// Join another device using their pairing code
    Join {
        /// The pairing code from the other device
        code: String,
    },

    /// Show pairing status
    Status,

    /// List pending pairing requests
    ListPending,

    /// Accept a pairing request
    Accept {
        /// Request ID
        request_id: String,
    },

    /// Reject a pairing request
    Reject {
        /// Request ID
        request_id: String,
    },
}

pub async fn handle_network_command(
    cmd: NetworkCommands,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

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
                    "spacedrive daemon start".bright_blue()
                );
                return Ok(());
            }
        }
    }

    match cmd {
        NetworkCommands::Init => {
            match client
                .send_command(DaemonCommand::InitNetworking)
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Networking initialized successfully", "✓".green());
                }
                DaemonResponse::Error(err) => {
                    println!("{} {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response", "✗".red());
                }
            }
        }

        NetworkCommands::Start => {
            match client
                .send_command(DaemonCommand::StartNetworking)
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Networking service started", "✓".green());
                }
                DaemonResponse::Error(err) => {
                    println!("{} {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response", "✗".red());
                }
            }
        }

        NetworkCommands::Stop => {
            match client
                .send_command(DaemonCommand::StopNetworking)
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Networking service stopped", "✓".green());
                }
                DaemonResponse::Error(err) => {
                    println!("{} {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response", "✗".red());
                }
            }
        }

        NetworkCommands::Devices => {
            match client
                .send_command(DaemonCommand::ListConnectedDevices)
                .await?
            {
                DaemonResponse::ConnectedDevices(devices) => {
                    if devices.is_empty() {
                        println!("No devices currently connected");
                    } else {
                        println!("Connected devices:");
                        let mut table = Table::new();
                        table.load_preset(UTF8_FULL);
                        table.set_header(vec!["Device ID", "Name", "Status", "Last Seen"]);

                        for device in devices {
                            table.add_row(vec![
                                &device.device_id.to_string()[..8],
                                &device.device_name,
                                &device.status,
                                &device.last_seen,
                            ]);
                        }

                        println!("{}", table);
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response", "✗".red());
                }
            }
        }

        NetworkCommands::Revoke { device_id } => {
            // Parse the device ID string to UUID
            let device_uuid = match device_id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid device ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::RevokeDevice { device_id: device_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Device {} revoked", "✓".green(), device_id);
                }
                DaemonResponse::Error(err) => {
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
            // Parse device_id to UUID
            let device_uuid = match device_id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid device ID format");
                    return Ok(());
                }
            };
            
            // Use sender name or default
            let sender_name = sender.unwrap_or_else(|| "Anonymous".to_string());
            
            match client
                .send_command(DaemonCommand::SendSpacedrop {
                    device_id: device_uuid,
                    file_path: file_path.to_string_lossy().to_string(),
                    sender_name,
                    message,
                })
                .await?
            {
                DaemonResponse::SpacedropStarted { transfer_id } => {
                    println!(
                        "{} Spacedrop started with transfer ID: {}",
                        "✓".green(),
                        transfer_id
                    );
                }
                DaemonResponse::Error(err) => {
                    println!("{} {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response", "✗".red());
                }
            }
        }

        NetworkCommands::Pair { action } => {
            // Convert to PairingAction and use the networking commands handler
            let pairing_action = match action {
                PairingCommands::Generate => {
                    networking_commands::PairingAction::Generate
                }
                PairingCommands::Join { code } => {
                    networking_commands::PairingAction::Join { code }
                }
                PairingCommands::Status => {
                    networking_commands::PairingAction::Status
                }
                PairingCommands::ListPending => {
                    networking_commands::PairingAction::List
                }
                PairingCommands::Accept { request_id } => {
                    // Parse request_id to UUID
                    let uuid = match request_id.parse::<Uuid>() {
                        Ok(id) => id,
                        Err(_) => {
                            println!("❌ Invalid request ID format");
                            return Ok(());
                        }
                    };
                    networking_commands::PairingAction::Accept {
                        request_id: uuid,
                    }
                }
                PairingCommands::Reject { request_id } => {
                    // Parse request_id to UUID
                    let uuid = match request_id.parse::<Uuid>() {
                        Ok(id) => id,
                        Err(_) => {
                            println!("❌ Invalid request ID format");
                            return Ok(());
                        }
                    };
                    networking_commands::PairingAction::Reject {
                        request_id: uuid,
                    }
                }
            };

            networking_commands::handle_pairing_command(
                pairing_action,
                &client,
            )
            .await?;
        }
    }

    Ok(())
}