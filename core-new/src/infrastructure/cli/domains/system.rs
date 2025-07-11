//! System monitoring commands
//!
//! This module handles CLI commands for system monitoring and information:
//! - System status checking
//! - Log viewing and following
//! - Real-time monitoring

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonConfig, DaemonResponse};
use clap::Subcommand;
use colored::Colorize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Subcommand, Clone, Debug)]
pub enum SystemCommands {
    /// Show system status
    Status,

    /// Monitor daemon logs in real-time
    Logs {
        /// Number of lines to show initially
        #[arg(short, long, default_value = "50")]
        lines: usize,
        /// Follow logs in real-time
        #[arg(short, long)]
        follow: bool,
    },

    /// Monitor all system activity in real-time
    Monitor,
    
    /// Launch Terminal User Interface for real-time monitoring
    Tui,
}

pub async fn handle_system_command(
    cmd: SystemCommands,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        SystemCommands::Status => {
            handle_status_command(instance_name).await
        }
        SystemCommands::Logs { lines, follow } => {
            handle_logs_command(lines, follow, instance_name).await
        }
        SystemCommands::Monitor => {
            handle_monitor_command(instance_name).await
        }
        SystemCommands::Tui => {
            handle_tui_command(instance_name).await
        }
    }
}

async fn handle_status_command(
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());
    
    match client.send_command(DaemonCommand::GetStatus).await {
        Ok(DaemonResponse::Status(status)) => {
            println!("📊 System Status");
            println!("  Version: {}", status.version.bright_cyan());
            println!("  Uptime: {} seconds", status.uptime_secs);
            
            if let Some(library_id) = status.current_library {
                println!("  Current Library: {}", library_id.to_string().bright_yellow());
            } else {
                println!("  Current Library: {}", "None".bright_red());
            }
            
            println!("  Active Jobs: {}", status.active_jobs);
            println!("  Total Locations: {}", status.total_locations);
            
            // Show basic system info
            println!("  OS: {}", std::env::consts::OS);
            println!("  Architecture: {}", std::env::consts::ARCH);
        }
        Ok(DaemonResponse::Error(e)) => {
            println!("❌ Failed to get system status: {}", e);
        }
        Err(e) => {
            println!("❌ Failed to communicate with daemon: {}", e);
        }
        _ => {
            println!("❌ Unexpected response from daemon");
        }
    }
    
    Ok(())
}

async fn handle_logs_command(
    lines: usize,
    follow: bool,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the daemon config to find the log file path
    let config = DaemonConfig::new(instance_name.clone());

    let log_file_path = config.log_file.ok_or("No log file configured for daemon")?;

    if !log_file_path.exists() {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        println!(
            "❌ Log file not found for daemon instance '{}'",
            instance_display
        );
        println!("   Expected at: {}", log_file_path.display());
        println!("   Make sure the daemon is running with logging enabled");
        return Ok(());
    }

    println!(
        "📋 {} - Press Ctrl+C to exit",
        format!(
            "Spacedrive Daemon Logs ({})",
            instance_name.as_deref().unwrap_or("default")
        )
        .bright_cyan()
    );
    println!(
        "   Log file: {}",
        log_file_path.display().to_string().bright_blue()
    );
    println!("═══════════════════════════════════════════");

    // Read initial lines
    let file = File::open(&log_file_path)?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

    // Show last N lines
    let start_index = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    for line in &all_lines[start_index..] {
        println!("{}", format_log_line(line));
    }

    if follow {
        // Follow mode - watch for new lines
        let mut file = File::open(&log_file_path)?;
        file.seek(SeekFrom::End(0))?;
        let mut reader = BufReader::new(file);

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // No new data, sleep and try again
                    sleep(Duration::from_millis(100)).await;
                }
                Ok(_) => {
                    // New line found
                    print!("{}", format_log_line(&line));
                }
                Err(e) => {
                    println!("❌ Error reading log file: {}", e);
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_monitor_command(
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Special case - monitor needs event streaming
    println!("📊 Job monitor not yet implemented for daemon mode");
    println!("   Use 'spacedrive job list' to see current jobs");
    Ok(())
}

async fn handle_tui_command(
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  Launching Terminal User Interface...");
    println!("❌ TUI command not yet implemented for daemon mode");
    println!("   This command will be available in a future update");
    println!("   The TUI will provide real-time monitoring of:");
    println!("   - Library overview and statistics");
    println!("   - Location management and indexing status");
    println!("   - Job progress and monitoring");
    println!("   - Event stream and system activity");
    println!("   - Interactive controls and navigation");
    Ok(())
}

fn format_log_line(line: &str) -> String {
    // Basic log formatting - colorize by log level
    if line.contains("ERROR") {
        line.red().to_string()
    } else if line.contains("WARN") {
        line.yellow().to_string()
    } else if line.contains("INFO") {
        line.normal().to_string()
    } else if line.contains("DEBUG") {
        line.bright_black().to_string()
    } else {
        line.to_string()
    }
}