//! CLI-specific networking command implementations
//!
//! This module contains all networking-related CLI command handlers,
//! separated from the core daemon functionality to maintain clean separation.

use std::path::PathBuf;
use uuid::Uuid;
use colored::*;
use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::networking::DeviceInfo;

/// Actions for pairing commands
#[derive(Debug, Clone)]
pub enum PairingAction {
    /// Generate a pairing code and wait for another device
    Generate { auto_accept: bool },
    /// Join another device using a pairing code
    Join { code: String },
    /// Show current pairing status
    Status,
    /// Cancel a pairing session
    Cancel { session_id: Uuid },
    /// List pending pairing requests
    List,
    /// Accept a pending pairing request
    Accept { request_id: Uuid },
    /// Reject a pending pairing request
    Reject { request_id: Uuid },
}

/// Handle networking-related CLI commands through the daemon
pub async fn handle_pairing_command(
    action: PairingAction,
    client: &DaemonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    // PairingAction is defined in this module

    match action {
        PairingAction::Generate { auto_accept } => {
            println!("🔑 Generating pairing code...");
            
            match client
                .send_command(DaemonCommand::StartPairingAsInitiator { auto_accept })
                .await?
            {
                DaemonResponse::PairingCodeGenerated { code, expires_in_seconds } => {
                    println!("\n🔗 Your Pairing Code");
                    println!("==================");
                    println!();
                    println!("Share this code with the other device:");
                    println!();
                    println!("    {}", code.bright_cyan().bold());
                    println!();
                    println!("⏰ This code expires in {} seconds", expires_in_seconds.to_string().yellow());
                    println!();
                    println!("💡 The other device should run:");
                    println!("   spacedrive network pair join \"{}\"", code.bright_blue());
                    
                    if auto_accept {
                        println!();
                        println!("🤖 Auto-accept enabled - pairing will complete automatically");
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to generate pairing code: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingAction::Join { code } => {
            println!("🔗 Joining pairing session...");
            println!("   Code: {}...", code.split_whitespace().take(3).collect::<Vec<_>>().join(" "));
            
            match client
                .send_command(DaemonCommand::StartPairingAsJoiner { code: code.clone() })
                .await?
            {
                DaemonResponse::Ok => {
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
                            println!("   Use 'spacedrive network pair status' to check final result");
                            break;
                        }
                        
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        attempts += 1;
                        
                        // Check pairing status with improved error handling
                        match client.send_command(DaemonCommand::GetPairingStatus).await {
                            Ok(DaemonResponse::PairingStatus { status, remote_device }) => {
                                match status.as_str() {
                                    "completed" => {
                                        println!("{} Pairing completed successfully!", "✅".green());
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
                                    s @ ("in_progress" | "waiting" | "connecting" | "authenticating") => {
                                        // Still in progress - show periodic updates
                                        if attempts % 5 == 0 {
                                            println!("   ⏳ Still pairing... ({}/{}) [{}]", attempts, max_attempts, s);
                                        }
                                    }
                                    s => {
                                        // Unknown status - log and continue
                                        if attempts % 10 == 0 {
                                            println!("   ℹ️  Status: {} ({}/{})", s, attempts, max_attempts);
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
                                    println!("   ⚠️  Unexpected response type from daemon ({}/{})", attempts, max_attempts);
                                }
                            }
                            Err(e) => {
                                // Network/communication error
                                if attempts % 15 == 0 {
                                    println!("   ⚠️  Connection issue: {} ({}/{})", e, attempts, max_attempts);
                                }
                                // Continue trying - daemon might be busy
                            }
                        }
                    }
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to join pairing session: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingAction::Status => {
            // Show current pairing status first
            println!("🔍 Checking pairing status...");
            println!();
            
            match client.send_command(DaemonCommand::GetPairingStatus).await {
                Ok(DaemonResponse::PairingStatus { status, remote_device }) => {
                    println!("📊 Current Pairing Status: {}", status.bright_blue());
                    if let Some(device) = remote_device {
                        println!("🔗 Connected Device: {} ({})", device.device_name, device.device_id.to_string()[..8].bright_cyan());
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
                        table.set_header(vec!["Request ID", "Device ID", "Device Name", "Received At"]);

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

        PairingAction::Cancel { session_id } => {
            match client
                .send_command(DaemonCommand::RejectPairing { request_id: session_id })
                .await?
            {
                DaemonResponse::Ok => {
                    println!("{} Pairing session {} cancelled", "✓".green(), session_id);
                }
                DaemonResponse::Error(err) => {
                    println!("{} Failed to cancel pairing session: {}", "✗".red(), err);
                }
                _ => {
                    println!("{} Unexpected response from daemon", "✗".red());
                }
            }
        }

        PairingAction::List => {
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
                        table.set_header(vec!["Request ID", "Device ID", "Device Name", "Received At"]);

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

        PairingAction::Accept { request_id } => {
            match client
                .send_command(DaemonCommand::AcceptPairing { request_id })
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

        PairingAction::Reject { request_id } => {
            match client
                .send_command(DaemonCommand::RejectPairing { request_id })
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
    let (transferred_size, transferred_unit) = format_bytes(bytes_transferred);
    let (total_size, total_unit) = format_bytes(total_bytes);
    
    format!("{:.1}% ({:.1} {} / {:.1} {})", 
        percentage, 
        transferred_size, transferred_unit,
        total_size, total_unit)
}

/// Helper function to format byte sizes
fn format_bytes(bytes: u64) -> (f64, &'static str) {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    (size, UNITS[unit_index])
}