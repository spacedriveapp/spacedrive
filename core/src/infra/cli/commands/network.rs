//! Network management commands
//!
//! This module handles CLI commands for networking operations:
//! - Initializing and managing networking services
//! - Device discovery and management
//! - Pairing operations with other devices
//! - Spacedrop file sharing

use crate::infra::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infra::cli::output::{CliOutput, Message};
use crate::infra::cli::output::messages::{DeviceInfo as OutputDeviceInfo, DeviceStatus, PairingRequest};
use crate::infra::cli::utils::format_bytes_parts;
use crate::service::networking::DeviceInfo;
use clap::Subcommand;
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
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

    // Check if daemon is running for most commands
    match &cmd {
        NetworkCommands::Init { .. } => {
            // Init doesn't require daemon to be running
        }
        _ => {
            if !client.is_running() {
                output.error(Message::DaemonNotRunning {
                    instance: instance_name.as_deref().unwrap_or("default").to_string(),
                })?;
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
                    output.print(Message::NetworkingInitialized)?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
                }
            }
        }

        NetworkCommands::Start => {
            match client
                .send_command(DaemonCommand::StartNetworking)
                .await?
            {
                DaemonResponse::Ok => {
                    output.print(Message::NetworkingStarted)?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
                }
            }
        }

        NetworkCommands::Stop => {
            match client
                .send_command(DaemonCommand::StopNetworking)
                .await?
            {
                DaemonResponse::Ok => {
                    output.print(Message::NetworkingStopped)?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
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
                        output.info("No devices currently connected")?;
                    } else {
                        let output_devices: Vec<OutputDeviceInfo> = devices.into_iter()
                            .map(|device| OutputDeviceInfo {
                                id: device.device_id.to_string(),
                                name: device.device_name.clone(),
                                status: match device.status.as_str() {
                                    "online" => DeviceStatus::Online,
                                    "offline" => DeviceStatus::Offline,
                                    "paired" => DeviceStatus::Paired,
                                    "discovered" => DeviceStatus::Discovered,
                                    _ => DeviceStatus::Offline,
                                },
                                peer_id: None, // TODO: Get from daemon if available
                            })
                            .collect();

                        if matches!(output.format(), crate::infra::cli::output::OutputFormat::Json) {
                            output.print(Message::DevicesList { devices: output_devices })?;
                        } else {
                            // For human output, use a table
                            let mut table = Table::new();
                            table.load_preset(UTF8_FULL);
                            table.set_header(vec!["Device ID", "Name", "Status"]);

                            for device in output_devices {
                                table.add_row(vec![
                                    &device.id[..8.min(device.id.len())],
                                    &device.name,
                                    &format!("{:?}", device.status),
                                ]);
                            }

                            output.section()
                                .title("Connected Devices")
                                .table(table)
                                .render()?;
                        }
                    }
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
                }
            }
        }

        NetworkCommands::Revoke { device_id } => {
            // Parse the device ID string to UUID
            let device_uuid = match device_id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    output.error(Message::Error("Invalid device ID format".to_string()))?;
                    return Ok(());
                }
            };

            match client
                .send_command(DaemonCommand::RevokeDevice { device_id: device_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    output.success(&format!("Device {} revoked", device_id))?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
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
                    output.error(Message::Error("Invalid device ID format".to_string()))?;
                    return Ok(());
                }
            };

            // Use sender name or default
            let sender_name = sender.unwrap_or_else(|| "Anonymous".to_string());

            match client
                .send_command(DaemonCommand::SendSpacedrop {
                    device_id: device_uuid,
                    file_path: file_path.to_string_lossy().to_string(),
                    sender_name: sender_name.clone(),
                    message,
                })
                .await?
            {
                DaemonResponse::SpacedropStarted { transfer_id } => {
                    output.print(Message::SpacedropSent {
                        file_name: file_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        device_name: device_id.clone(),
                    })?;
                    output.info(&format!("Transfer ID: {}", transfer_id))?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(err))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response".to_string()))?;
                }
            }
        }

        NetworkCommands::Pair { action } => {
            handle_pairing_command(action, &client, &mut output).await?;
        }
    }

    Ok(())
}

/// Handle pairing-related CLI commands through the daemon
async fn handle_pairing_command(
    action: PairingCommands,
    client: &DaemonClient,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        PairingCommands::Generate => {
            output.info("Generating pairing code...")?;

            match client
                .send_command(DaemonCommand::StartPairingAsInitiator)
                .await?
            {
                DaemonResponse::PairingCodeGenerated {
                    code,
                    expires_in_seconds,
                } => {
                    output.print(Message::PairingCodeGenerated { code: code.clone() })?;

                    output.section()
                        .empty_line()
                        .text(&format!("This code expires in {} seconds", expires_in_seconds))
                        .empty_line()
                        .help()
                            .item(&format!("The other device should run: spacedrive network pair join \"{}\"", code))
                            .item("Pairing will auto-accept valid requests")
                        .render()?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to generate pairing code: {}", err)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        PairingCommands::Join { code } => {
            output.info("Joining pairing session...")?;
            let code_preview = code.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            output.info(&format!("Code: {}...", code_preview))?;

            match client
                .send_command(DaemonCommand::StartPairingAsJoiner { code: code.clone() })
                .await?
            {
                DaemonResponse::PairingInProgress => {
                    output.print(Message::PairingInProgress {
                        device_name: "remote device".to_string()
                    })?;
                    output.info("Pairing process started - this may take a few moments...")?;

                    // Monitor pairing status
                    output.info("")?;
                    output.info("Monitoring pairing progress...")?;

                    let mut attempts = 0;
                    let max_attempts = 30;

                    loop {
                        if attempts >= max_attempts {
                            output.warning("Pairing monitoring timed out")?;
                            output.info("Use 'spacedrive network pair status' to check final result")?;
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
                                        if let Some(device) = remote_device {
                                            output.print(Message::PairingSuccess {
                                                device_name: device.device_name,
                                                device_id: device.device_id.to_string(),
                                            })?;
                                        } else {
                                            output.success("Pairing completed successfully!")?;
                                        }
                                        break;
                                    }
                                    s if s.contains("failed") => {
                                        output.print(Message::PairingFailed { reason: s.to_string() })?;
                                        break;
                                    }
                                    "cancelled" | "canceled" => {
                                        output.warning("Pairing was cancelled")?;
                                        break;
                                    }
                                    s @ ("in_progress" | "waiting" | "connecting"
                                    | "authenticating" | "exchanging_keys" | "establishing_session") => {
                                        // Still in progress - show periodic updates
                                        if attempts % 5 == 0 {
                                            output.info(&format!(
                                                "Still pairing... ({}/{}) [{}]",
                                                attempts, max_attempts, s
                                            ))?;
                                        }
                                    }
                                    s => {
                                        // Unknown status - log and continue
                                        if attempts % 10 == 0 {
                                            output.info(&format!(
                                                "Status: {} ({}/{})",
                                                s, attempts, max_attempts
                                            ))?;
                                        }
                                    }
                                }
                            }
                            Ok(DaemonResponse::Error(err)) => {
                                output.error(Message::Error(format!("Daemon error: {}", err)))?;
                                break;
                            }
                            Ok(_) => {
                                // Unexpected response type
                                if attempts % 20 == 0 {
                                    output.warning(&format!(
                                        "Unexpected response type from daemon ({}/{})",
                                        attempts, max_attempts
                                    ))?;
                                }
                            }
                            Err(e) => {
                                // Network/communication error
                                if attempts % 15 == 0 {
                                    output.warning(&format!(
                                        "Connection issue: {} ({}/{})",
                                        e, attempts, max_attempts
                                    ))?;
                                }
                                // Continue trying - daemon might be busy
                            }
                        }
                    }
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to join pairing session: {}", err)))?;
                }
                response => {
                    output.error(Message::Error(format!(
                        "Unexpected response from daemon: {:?}",
                        response
                    )))?;
                }
            }
        }

        PairingCommands::Status => {
            // Show current pairing status first
            output.info("Checking pairing status...")?;
            output.info("")?;

            match client.send_command(DaemonCommand::GetPairingStatus).await {
                Ok(DaemonResponse::PairingStatus {
                    status,
                    remote_device,
                }) => {
                    let section = output.section()
                        .title("Current Pairing Status")
                        .item("Status", &status);

                    let section = if let Some(device) = remote_device {
                        section.item("Connected Device", &format!("{} ({})",
                            device.device_name,
                            &device.device_id.to_string()[..8]
                        ))
                    } else {
                        section
                    };
                    section.render()?;
                }
                Ok(DaemonResponse::Error(err)) => {
                    output.warning(&format!("Status check error: {}", err))?;
                }
                _ => {
                    output.warning("Could not determine current status")?;
                }
            }

            // Show pending requests
            match client
                .send_command(DaemonCommand::ListPendingPairings)
                .await?
            {
                DaemonResponse::PendingPairings(requests) => {
                    if requests.is_empty() {
                        output.info("No pending pairing requests")?;
                        output.section()
                            .title("To start pairing:")
                            .help()
                                .item("Generate a code: spacedrive network pair generate")
                                .item("Join with a code: spacedrive network pair join \"<code>\"")
                            .render()?;
                    } else {
                        let pending_requests: Vec<PairingRequest> = requests.into_iter()
                            .map(|req| PairingRequest {
                                id: req.request_id.to_string(),
                                device_name: req.device_name,
                                timestamp: 0, // TODO: Parse from received_at string
                            })
                            .collect();

                        output.print(Message::PairingStatus {
                            status: "active".to_string(),
                            pending_requests,
                        })?;
                    }
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to get pairing status: {}", err)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
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
                        output.info("No pending pairing requests")?;
                    } else {
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

                        output.section()
                            .title("Pending Pairing Requests")
                            .table(table)
                            .empty_line()
                            .help()
                                .item("To accept a request: spacedrive network pair accept <request_id>")
                                .item("To reject a request: spacedrive network pair reject <request_id>")
                            .render()?;
                    }
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to list pending pairings: {}", err)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        PairingCommands::Accept { request_id } => {
            let request_uuid = match request_id.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    output.error(Message::Error("Invalid request ID format".to_string()))?;
                    return Ok(());
                }
            };

            match client
                .send_command(DaemonCommand::AcceptPairing { request_id: request_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    output.success(&format!("Pairing request {} accepted", request_id))?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to accept pairing request: {}", err)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        PairingCommands::Reject { request_id } => {
            let request_uuid = match request_id.parse::<Uuid>() {
                Ok(id) => id,
                Err(_) => {
                    output.error(Message::Error("Invalid request ID format".to_string()))?;
                    return Ok(());
                }
            };

            match client
                .send_command(DaemonCommand::RejectPairing { request_id: request_uuid })
                .await?
            {
                DaemonResponse::Ok => {
                    output.success(&format!("Pairing request {} rejected", request_id))?;
                }
                DaemonResponse::Error(err) => {
                    output.error(Message::Error(format!("Failed to reject pairing request: {}", err)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
    }

    Ok(())
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