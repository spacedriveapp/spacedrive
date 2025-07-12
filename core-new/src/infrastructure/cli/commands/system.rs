//! System monitoring commands
//!
//! This module handles CLI commands for system monitoring and information:
//! - System status checking
//! - Log viewing and following
//! - Real-time monitoring

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonConfig, DaemonResponse};
use crate::infrastructure::cli::output::{CliOutput, Message};
use clap::Subcommand;
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
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        SystemCommands::Status => {
            handle_status_command(instance_name, &mut output).await
        }
        SystemCommands::Logs { lines, follow } => {
            handle_logs_command(lines, follow, instance_name, &mut output).await
        }
        SystemCommands::Monitor => {
            handle_monitor_command(instance_name, &mut output).await
        }
        SystemCommands::Tui => {
            handle_tui_command(instance_name, &mut output).await
        }
    }
}

async fn handle_status_command(
    instance_name: Option<String>,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());
    
    match client.send_command(DaemonCommand::GetStatus).await {
        Ok(DaemonResponse::Status(status)) => {
            let section = output.section()
                .title("System Status")
                .item("Version", &status.version)
                .item("Uptime", &format!("{} seconds", status.uptime_secs));
            
            let section = if let Some(library_id) = status.current_library {
                section.item("Current Library", &library_id.to_string())
            } else {
                section.item("Current Library", "None")
            };
            
            section.item("Active Jobs", &status.active_jobs.to_string())
                .item("Total Locations", &status.total_locations.to_string())
                .empty_line()
                .item("OS", std::env::consts::OS)
                .item("Architecture", std::env::consts::ARCH)
                .render()?;
        }
        Ok(DaemonResponse::Error(e)) => {
            output.error(Message::Error(format!("Failed to get system status: {}", e)))?;
        }
        Err(e) => {
            output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
        }
        _ => {
            output.error(Message::Error("Unexpected response from daemon".to_string()))?;
        }
    }
    
    Ok(())
}

async fn handle_logs_command(
    lines: usize,
    follow: bool,
    instance_name: Option<String>,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the daemon config to find the log file path
    let config = DaemonConfig::new(instance_name.clone());

    let log_file_path = config.log_file.ok_or("No log file configured for daemon")?;

    if !log_file_path.exists() {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        output.error(Message::Error(format!(
            "Log file not found for daemon instance '{}'",
            instance_display
        )))?;
        output.section()
            .text(&format!("Expected at: {}", log_file_path.display()))
            .text("Make sure the daemon is running with logging enabled")
            .render()?;
        return Ok(());
    }

    output.print(Message::LogsShowing { path: log_file_path.clone() })?;
    output.section()
        .text(&format!(
            "Spacedrive Daemon Logs ({}) - Press Ctrl+C to exit",
            instance_name.as_deref().unwrap_or("default")
        ))
        .status("Log file", &log_file_path.display().to_string())
        .text("═══════════════════════════════════════════")
        .render()?;

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
        // For logs, we intentionally use println! to output directly to stdout
        // This preserves exact log formatting and bypasses the output system's formatting
        println!("{}", format_log_line(line, output));
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
                    // New line found - use print! for real-time output without newline
                    print!("{}", format_log_line(&line, output));
                }
                Err(e) => {
                    output.error(Message::Error(format!("Error reading log file: {}", e)))?;
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_monitor_command(
    instance_name: Option<String>,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    // Special case - monitor needs event streaming
    output.info("Job monitor not yet implemented for daemon mode")?;
    output.info("Use 'spacedrive job list' to see current jobs")?;
    Ok(())
}

async fn handle_tui_command(
    instance_name: Option<String>,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    output.info("Launching Terminal User Interface...")?;
    output.error(Message::Error("TUI command not yet implemented for daemon mode".to_string()))?;
    output.section()
        .text("This command will be available in a future update")
        .text("The TUI will provide real-time monitoring of:")
        .text("- Library overview and statistics")
        .text("- Location management and indexing status")
        .text("- Job progress and monitoring")
        .text("- Event stream and system activity")
        .text("- Interactive controls and navigation")
        .render()?;
    Ok(())
}

fn format_log_line(line: &str, output: &CliOutput) -> String {
    // Basic log formatting - colorize by log level if colors are enabled
    if !output.use_color() {
        return line.to_string();
    }
    
    use owo_colors::OwoColorize;
    if line.contains("ERROR") {
        line.red().to_string()
    } else if line.contains("WARN") {
        line.yellow().to_string()
    } else if line.contains("INFO") {
        line.to_string()
    } else if line.contains("DEBUG") {
        line.dimmed().to_string()
    } else {
        line.to_string()
    }
}