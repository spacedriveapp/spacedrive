//! Network management commands
//!
//! This module handles CLI commands for networking operations:
//! - Initializing and managing networking services
//! - Device discovery and management
//! - Pairing operations with other devices
//! - Spacedrop file sharing

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::utils::format_bytes_parts;
use crate::services::networking::DeviceInfo;
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
            handle_pairing_command(action, &client).await?;
        }
    }

    Ok(())
}

/// Handle pairing-related CLI commands through the daemon
async fn handle_pairing_command(
    action: PairingCommands,
    client: &DaemonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        PairingCommands::Generate => {
            println!("🔑 Generating pairing code...");

            match client
                .send_command(DaemonCommand::StartPairingAsInitiator)
                .await?
            {
                DaemonResponse::PairingCodeGenerated {
                    code,
                    expires_in_seconds,
                } => {
                    println!("\n🔗 Your Pairing Code");
                    println!("==================");
                    println!();
                    println!("Share this code with the other device:");
                    println!();
                    println!("    {}", code.bright_cyan().bold());
                    println!();
                    println!(
                        "⏰ This code expires in {} seconds",
                        expires_in_seconds.to_string().yellow()
                    );
                    println!();
                    println!("💡 The other device should run:");
                    println!("   spacedrive network pair join \"{}\"", code.bright_blue());
                    println!();
                    println!("✨ Pairing will auto-accept valid requests");
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to generate pairing code: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingCommands::Join { code } => {
            println!("🔗 Joining pairing session...");
            println!(
                "   Code: {}...",
                code.split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            match client
                .send_command(DaemonCommand::StartPairingAsJoiner { code: code.clone() })
                .await?
            {
                DaemonResponse::PairingInProgress => {
                    println!("{} Successfully joined pairing session!", "✓".green());
                    println!("   Pairing process started - this may take a few moments...");

                    // Monitor pairing status
                    println!();
                    println!("📊 Monitoring pairing progress...");

                    let mut attempts = 0;
                    let max_attempts = 30; // 30 seconds timeout

                    loop {
                        if attempts >= max_attempts {
                            println!("⏰ Pairing monitoring timed out");
                            println!(
                                "   Use 'spacedrive network pair status' to check final result"
                            );
                            break;
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        attempts += 1;

                        // Check pairing status with improved error handling
                        match client.send_command(DaemonCommand::GetPairingStatus).await {
                            Ok(DaemonResponse::PairingStatus {
                                status,
                                remote_device,
                            }) => {
                                match status.as_str() {
                                    "completed" => {
                                        println!(
                                            "{} Pairing completed successfully!",
                                            "✅".green()
                                        );
                                        if let Some(device) = remote_device {
                                            println!("   Paired with: {}", device.device_name);
                                        }
                                        break;
                                    }
                                    s if s.contains("failed") => {
                                        println!("{} Pairing failed: {}", "❌".red(), s);
                                        break;
                                    }
                                    "cancelled" | "canceled" => {
                                        println!("{} Pairing was cancelled", "⚠️".yellow());
                                        break;
                                    }
                                    s @ ("in_progress" | "waiting" | "connecting"
                                    | "authenticating") => {
                                        // Still in progress - show periodic updates
                                        if attempts % 5 == 0 {
                                            println!(
                                                "   ⏳ Still pairing... ({}/{}) [{}]",
                                                attempts, max_attempts, s
                                            );
                                        }
                                    }
                                    s => {
                                        // Unknown status - log and continue
                                        if attempts % 10 == 0 {
                                            println!(
                                                "   ℹ️  Status: {} ({}/{})",
                                                s, attempts, max_attempts
                                            );
                                        }
                                    }
                                }
                            }
                            Ok(DaemonResponse::Error(err)) => {
                                println!("{} Daemon error: {}", "❌".red(), err);
                                break;
                            }
                            Ok(_) => {
                                // Unexpected response type
                                if attempts % 20 == 0 {
                                    println!(
                                        "   ⚠️  Unexpected response type from daemon ({}/{})",
                                        attempts, max_attempts
                                    );
                                }
                            }
                            Err(e) => {
                                // Network/communication error
                                if attempts % 15 == 0 {
                                    println!(
                                        "   ⚠️  Connection issue: {} ({}/{})",
                                        e, attempts, max_attempts
                                    );
                                }
                                // Continue trying - daemon might be busy
                            }
                        }
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to join pairing session: {}", "✗".red(), err);
                }
                response => {
                    println!(
                        "{} Unexpected response from daemon: {:?}",
                        "✗".red(),
                        response
                    );
                }
            }
        }

        PairingCommands::Status => {
            // Show current pairing status first
            println!("🔍 Checking pairing status...");
            println!();

            match client.send_command(DaemonCommand::GetPairingStatus).await {
                Ok(DaemonResponse::PairingStatus {
                    status,
                    remote_device,
                }) => {
                    println!("📊 Current Pairing Status: {}", status.bright_blue());
                    if let Some(device) = remote_device {
                        println!(
                            "🔗 Connected Device: {} ({})",
                            device.device_name,
                            device.device_id.to_string()[..8].bright_cyan()
                        );
                    }
                    println!();
                }
                Ok(DaemonResponse::Error(err)) => {
                    println!("⚠️  Status check error: {}", err.yellow());
                    println!();
                }
                _ => {
                    println!("⚠️  Could not determine current status");
                    println!();
                }
            }

            // Show pending requests
            match client
                .send_command(DaemonCommand::ListPendingPairings)
                .await?
            {
                DaemonResponse::PendingPairings(requests) => {
                    if requests.is_empty() {
                        println!("📭 No pending pairing requests");
                        println!();
                        println!("💡 To start pairing:");
                        println!("   • Generate a code: spacedrive network pair generate");
                        println!("   • Join with a code: spacedrive network pair join \"<code>\"");
                    } else {
                        use comfy_table::{presets::UTF8_FULL, Table};
                        let mut table = Table::new();
                        table.load_preset(UTF8_FULL);
                        table.set_header(vec![
                            "Request ID",
                            "Device ID",
                            "Device Name",
                            "Received At",
                        ]);

                        for request in requests {
                            table.add_row(vec![
                                &request.request_id.to_string()[..8],
                                &request.device_id.to_string()[..8],
                                &request.device_name,
                                &request.received_at,
                            ]);
                        }

                        println!("📬 Pending Pairing Requests");
                        println!("{}", table);
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to get pairing status: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingCommands::ListPending => {
            // List pending pairing requests
            match client
                .send_command(DaemonCommand::ListPendingPairings)
                .await?
            {
                DaemonResponse::PendingPairings(requests) => {
                    if requests.is_empty() {
                        println!("📭 No pending pairing requests");
                    } else {
                        use comfy_table::{presets::UTF8_FULL, Table};
                        let mut table = Table::new();
                        table.load_preset(UTF8_FULL);
                        table.set_header(vec![
                            "Request ID",
                            "Device ID",
                            "Device Name",
                            "Received At",
                        ]);

                        for request in &requests {
                            table.add_row(vec![
                                &request.request_id.to_string()[..8],
                                &request.device_id.to_string()[..8],
                                &request.device_name,
                                &request.received_at,
                            ]);
                        }

                        println!("📬 Pending Pairing Requests");
                        println!("{}", table);

                        println!();
                        println!("💡 To accept a request:");
                        println!("   spacedrive network pair accept <request_id>");
                        println!("💡 To reject a request:");
                        println!("   spacedrive network pair reject <request_id>");
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to list pending pairings: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingCommands::Accept { request_id } => {
            let request_uuid = match request_id.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    println!("❌ Invalid request ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::AcceptPairing { request_id: request_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Pairing request {} accepted", "✓".green(), request_id);
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to accept pairing request: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingCommands::Reject { request_id } => {
            let request_uuid = match request_id.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    println!("❌ Invalid request ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::RejectPairing { request_id: request_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Pairing request {} rejected", "✓".green(), request_id);
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to reject pairing request: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }
    }

    Ok(())
}

/// Helper function to display device connection information
pub fn display_device_info(devices: &[DeviceInfo]) {
    if devices.is_empty() {
        println!("📭 No devices found");
        return;
    }

    use comfy_table::{presets::UTF8_FULL, Table};
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Device ID", "Name", "Fingerprint", "Last Seen"]);

    for device in devices {
        table.add_row(vec![
            &device.device_id.to_string()[..8],
            &device.device_name,
            &device.network_fingerprint.to_string()[..8],
            &device.last_seen.format("%Y-%m-%d %H:%M:%S").to_string(),
        ]);
    }

    println!("{}", table);
}

/// Helper function to format file transfer progress
pub fn format_transfer_progress(bytes_transferred: u64, total_bytes: u64) -> String {
    if total_bytes == 0 {
        return "Unknown".to_string();
    }

    let percentage = (bytes_transferred as f64 / total_bytes as f64) * 100.0;
    let (transferred_size, transferred_unit) = format_bytes_parts(bytes_transferred);
    let (total_size, total_unit) = format_bytes_parts(total_bytes);

    format!(
        "{:.1}% ({:.1} {} / {:.1} {})",
        percentage, transferred_size, transferred_unit, total_size, total_unit
    )
}