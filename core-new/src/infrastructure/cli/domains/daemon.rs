//! Daemon lifecycle management commands
//!
//! This module handles CLI commands that manage the daemon itself:
//! - Starting and stopping the daemon
//! - Checking daemon status
//! - Managing multiple daemon instances

use crate::infrastructure::cli::daemon::{Daemon, DaemonClient, DaemonCommand, DaemonConfig, DaemonResponse};
use crate::infrastructure::cli::output::{CliOutput, Message};
use crate::infrastructure::cli::output::messages::{LibraryInfo as OutputLibraryInfo};
use clap::Subcommand;
use comfy_table::Table;
use std::path::PathBuf;

#[derive(Subcommand, Clone, Debug)]
pub enum DaemonCommands {
    /// Start the Spacedrive daemon in the background
    Start {
        /// Run in foreground instead of daemonizing
        #[arg(short, long)]
        foreground: bool,
        /// Enable networking on startup
        #[arg(long)]
        enable_networking: bool,
    },

    /// Stop the Spacedrive daemon
    Stop,

    /// Check if the daemon is running and show status
    Status,

    /// Manage daemon instances
    #[command(subcommand)]
    Instance(InstanceCommands),
}

#[derive(Subcommand, Clone, Debug)]
pub enum InstanceCommands {
    /// List all daemon instances
    List,
    /// Stop a specific daemon instance
    Stop {
        /// Instance name to stop
        name: String,
    },
    /// Show currently targeted instance
    Current,
}

pub async fn handle_daemon_command(
    cmd: DaemonCommands,
    data_dir: PathBuf,
    instance_name: Option<String>,
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DaemonCommands::Start {
            foreground,
            enable_networking,
        } => {
            handle_start_daemon(data_dir, foreground, enable_networking, instance_name, output).await
        }
        DaemonCommands::Stop => {
            handle_stop_daemon(instance_name, output).await
        }
        DaemonCommands::Status => {
            handle_daemon_status(instance_name, output).await
        }
        DaemonCommands::Instance(instance_cmd) => {
            handle_instance_command(instance_cmd, output).await
        }
    }
}

async fn handle_start_daemon(
    data_dir: PathBuf,
    foreground: bool,
    enable_networking: bool,
    instance_name: Option<String>,
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    if Daemon::is_running_instance(instance_name.clone()) {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        output.warning(&format!(
            "Spacedrive daemon instance '{}' is already running",
            instance_display
        ))?;
        return Ok(());
    }

    output.print(Message::DaemonStarting {
        instance: instance_name.as_deref().unwrap_or("default").to_string(),
    })?;

    if foreground {
        // Run in foreground
        if enable_networking {
            // For networking enabled startup, we need a default password
            output.info("Starting daemon with networking enabled...")?;
            output.info("Using master key for secure device authentication.")?;

            match Daemon::new_with_networking_and_instance(
                data_dir.clone(),
                instance_name.clone(),
            )
            .await
            {
                Ok(daemon) => daemon.start().await?,
                Err(e) => {
                    output.error(Message::Error(format!("Failed to start daemon with networking: {}", e)))?;
                    output.info("Falling back to daemon without networking...")?;
                    let daemon =
                        Daemon::new_with_instance(data_dir, instance_name.clone()).await?;
                    daemon.start().await?;
                }
            }
        } else {
            let daemon = Daemon::new_with_instance(data_dir, instance_name.clone()).await?;
            daemon.start().await?;
        }
    } else {
        // Daemonize (simplified version - in production use proper daemonization)
        use std::process::Command;

        let exe = std::env::current_exe()?;
        let mut cmd = Command::new(exe);
        cmd.arg("daemon")
            .arg("start")
            .arg("--foreground")
            .arg("--data-dir")
            .arg(data_dir);

        if let Some(ref instance) = instance_name {
            cmd.arg("--instance").arg(instance);
        }

        if enable_networking {
            cmd.arg("--enable-networking");
        }

        // Detach from terminal
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            unsafe {
                cmd.pre_exec(|| {
                    // Create new session
                    libc::setsid();
                    Ok(())
                });
            }
        }

        cmd.spawn()?;

        // Wait a bit to see if it started
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        if Daemon::is_running_instance(instance_name.clone()) {
            let instance_display = instance_name.as_deref().unwrap_or("default");
            output.success(&format!(
                "Spacedrive daemon instance '{}' started successfully",
                instance_display
            ))?;
        } else {
            let instance_display = instance_name.as_deref().unwrap_or("default");
            output.error(Message::Error(format!(
                "Failed to start Spacedrive daemon instance '{}'",
                instance_display
            )))?;
        }
    }

    Ok(())
}

async fn handle_stop_daemon(
    instance_name: Option<String>,
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    if !Daemon::is_running_instance(instance_name.clone()) {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        output.warning(&format!(
            "Spacedrive daemon instance '{}' is not running",
            instance_display
        ))?;
        return Ok(());
    }

    let instance_display = instance_name.as_deref().unwrap_or("default");
    output.print(Message::DaemonStopping {
        instance: instance_display.to_string(),
    })?;
    Daemon::stop_instance(instance_name.clone()).await?;

    // Wait a bit to ensure it's stopped
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    if !Daemon::is_running_instance(instance_name.clone()) {
        output.print(Message::DaemonStopped {
            instance: instance_display.to_string(),
        })?;
    } else {
        output.error(Message::Error(format!(
            "Failed to stop Spacedrive daemon instance '{}'",
            instance_display
        )))?;
    }

    Ok(())
}

async fn handle_daemon_status(
    instance_name: Option<String>,
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let instance_display = instance_name.as_deref().unwrap_or("default");

    if Daemon::is_running_instance(instance_name.clone()) {
        output.success(&format!(
            "Spacedrive daemon instance '{}' is running",
            instance_display
        ))?;

        // Try to get more info from daemon
        let client = DaemonClient::new_with_instance(instance_name);

        // Get status
        match client.send_command(DaemonCommand::GetStatus).await {
            Ok(DaemonResponse::Status(status)) => {
                output.section()
                    .title("Status")
                    .status("Version", &status.version)
                    .status("Uptime", &format!("{} seconds", status.uptime_secs))
                    .status("Active Jobs", &status.active_jobs.to_string())
                    .status("Total Locations", &status.total_locations.to_string())
                    .render()?;
            }
            Err(e) => {
                output.warning(&format!("Could not get status: {}", e))?;
            }
            _ => {}
        }

        // Get libraries
        match client
            .send_command(DaemonCommand::ListLibraries)
            .await
        {
            Ok(DaemonResponse::Libraries(libraries)) => {
                if !libraries.is_empty() {
                    output.section()
                        .empty_line()
                        .title("Libraries")
                        .render()?;
                    
                    for lib in &libraries {
                        output.section()
                            .text(&format!("   • {} ({})", lib.name, lib.id))
                            .render()?;
                    }
                }
            }
            Err(e) => {
                output.warning(&format!("Could not get libraries: {}", e))?;
            }
            _ => {}
        }

        // Get current library
        match client
            .send_command(DaemonCommand::GetCurrentLibrary)
            .await
        {
            Ok(DaemonResponse::CurrentLibrary(lib_opt)) => {
                if let Some(lib) = lib_opt {
                    output.section()
                        .empty_line()
                        .title("Current Library")
                        .item("Name", &lib.name)
                        .item("ID", &lib.id.to_string())
                        .item("Path", &lib.path.display().to_string())
                        .render()?;
                } else {
                    output.section()
                        .empty_line()
                        .title("Current Library")
                        .text("None selected")
                        .render()?;
                }
            }
            Err(e) => {
                output.warning(&format!("Could not get current library: {}", e))?;
            }
            _ => {}
        }
    } else {
        output.error(Message::DaemonNotRunning {
            instance: instance_display.to_string(),
        })?;
        
        let start_cmd = if instance_name.is_some() {
            format!("spacedrive --instance {} start", instance_display)
        } else {
            "spacedrive start".to_string()
        };
        
        output.section()
            .help()
                .item(&format!("Start it with: {}", start_cmd))
            .render()?;
    }

    Ok(())
}

async fn handle_instance_command(cmd: InstanceCommands, mut output: CliOutput) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InstanceCommands::List => match Daemon::list_instances() {
            Ok(instances) => {
                if instances.is_empty() {
                    output.info("No daemon instances found")?;
                } else {
                    let mut table = Table::new();
                    table.set_header(vec!["Instance", "Status", "Socket Path"]);

                    for instance in instances {
                        let status = if instance.is_running {
                            "Running"
                        } else {
                            "Stopped"
                        };

                        table.add_row(vec![
                            instance.display_name().to_string(),
                            status.to_string(),
                            instance.socket_path.display().to_string(),
                        ]);
                    }

                    output.section()
                        .table(table)
                        .render()?;
                }
            }
            Err(e) => {
                output.error(Message::Error(format!("Failed to list instances: {}", e)))?;
            }
        },

        InstanceCommands::Stop { name } => {
            let instance_name = if name == "default" {
                None
            } else {
                Some(name.clone())
            };
            match Daemon::stop_instance(instance_name).await {
                Ok(_) => {
                    output.success(&format!("Daemon instance '{}' stopped", name))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to stop instance '{}': {}", name, e)))?;
                }
            }
        }

        InstanceCommands::Current => {
            output.info("Current instance functionality not yet implemented")?;
            output.info("Use --instance <name> flag to target specific instances")?;
        }
    }

    Ok(())
}