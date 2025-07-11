//! Daemon lifecycle management commands
//!
//! This module handles CLI commands that manage the daemon itself:
//! - Starting and stopping the daemon
//! - Checking daemon status
//! - Managing multiple daemon instances

use crate::infrastructure::cli::daemon::{Daemon, DaemonClient, DaemonCommand, DaemonConfig, DaemonResponse};
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;
use comfy_table::Table;

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
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DaemonCommands::Start {
            foreground,
            enable_networking,
        } => {
            handle_start_daemon(data_dir, foreground, enable_networking, instance_name).await
        }
        DaemonCommands::Stop => {
            handle_stop_daemon(instance_name).await
        }
        DaemonCommands::Status => {
            handle_daemon_status(instance_name).await
        }
        DaemonCommands::Instance(instance_cmd) => {
            handle_instance_command(instance_cmd).await
        }
    }
}

async fn handle_start_daemon(
    data_dir: PathBuf,
    foreground: bool,
    enable_networking: bool,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if Daemon::is_running_instance(instance_name.clone()) {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        println!(
            "⚠️  Spacedrive daemon instance '{}' is already running",
            instance_display
        );
        return Ok(());
    }

    println!("🚀 Starting Spacedrive daemon...");

    if foreground {
        // Run in foreground
        if enable_networking {
            // For networking enabled startup, we need a default password
            println!("🔐 Starting daemon with networking enabled...");
            println!("   Using master key for secure device authentication.");

            match Daemon::new_with_networking_and_instance(
                data_dir.clone(),
                instance_name.clone(),
            )
            .await
            {
                Ok(daemon) => daemon.start().await?,
                Err(e) => {
                    println!("❌ Failed to start daemon with networking: {}", e);
                    println!("   Falling back to daemon without networking...");
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
            println!(
                "✅ Spacedrive daemon instance '{}' started successfully",
                instance_display
            );
        } else {
            let instance_display = instance_name.as_deref().unwrap_or("default");
            println!(
                "❌ Failed to start Spacedrive daemon instance '{}'",
                instance_display
            );
        }
    }

    Ok(())
}

async fn handle_stop_daemon(
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !Daemon::is_running_instance(instance_name.clone()) {
        let instance_display = instance_name.as_deref().unwrap_or("default");
        println!(
            "⚠️  Spacedrive daemon instance '{}' is not running",
            instance_display
        );
        return Ok(());
    }

    let instance_display = instance_name.as_deref().unwrap_or("default");
    println!(
        "🛑 Stopping Spacedrive daemon instance '{}'...",
        instance_display
    );
    Daemon::stop_instance(instance_name.clone()).await?;

    // Wait a bit to ensure it's stopped
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    if !Daemon::is_running_instance(instance_name.clone()) {
        println!(
            "✅ Spacedrive daemon instance '{}' stopped",
            instance_display
        );
    } else {
        println!(
            "❌ Failed to stop Spacedrive daemon instance '{}'",
            instance_display
        );
    }

    Ok(())
}

async fn handle_daemon_status(
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let instance_display = instance_name.as_deref().unwrap_or("default");

    if Daemon::is_running_instance(instance_name.clone()) {
        println!(
            "✅ Spacedrive daemon instance '{}' is running",
            instance_display
        );

        // Try to get more info from daemon
        let client = DaemonClient::new_with_instance(instance_name);

        // Get status
        match client.send_command(DaemonCommand::GetStatus).await {
            Ok(DaemonResponse::Status(status)) => {
                println!("\n📊 Status:");
                println!("   Version: {}", status.version.bright_blue());
                println!(
                    "   Uptime: {} seconds",
                    status.uptime_secs.to_string().bright_yellow()
                );
                println!(
                    "   Active Jobs: {}",
                    status.active_jobs.to_string().bright_green()
                );
                println!("   Total Locations: {}", status.total_locations);
            }
            Err(e) => {
                println!("   ⚠️  Could not get status: {}", e);
            }
            _ => {}
        }

        // Get libraries
        match client
            .send_command(DaemonCommand::ListLibraries)
            .await
        {
            Ok(DaemonResponse::Libraries(libraries)) => {
                println!("\n📚 Libraries:");
                if libraries.is_empty() {
                    println!("   No libraries found");
                } else {
                    for lib in &libraries {
                        println!(
                            "   • {} ({})",
                            lib.name.bright_cyan(),
                            lib.id.to_string().bright_yellow()
                        );
                    }
                }
            }
            Err(e) => {
                println!("   ⚠️  Could not get libraries: {}", e);
            }
            _ => {}
        }

        // Get current library
        match client
            .send_command(DaemonCommand::GetCurrentLibrary)
            .await
        {
            Ok(DaemonResponse::CurrentLibrary(Some(lib))) => {
                println!("\n🔍 Current Library:");
                println!(
                    "   {} ({})",
                    lib.name.bright_cyan().bold(),
                    lib.id.to_string().bright_yellow()
                );
                println!("   Path: {}", lib.path.display().to_string().bright_blue());
            }
            Ok(DaemonResponse::CurrentLibrary(None)) => {
                println!("\n🔍 Current Library: None selected");
            }
            Err(e) => {
                println!("   ⚠️  Could not get current library: {}", e);
            }
            _ => {}
        }
    } else {
        println!(
            "❌ Spacedrive daemon instance '{}' is not running",
            instance_display
        );
        if instance_name.is_some() {
            println!(
                "   Start it with: spacedrive --instance {} daemon start",
                instance_display
            );
        } else {
            println!("   Start it with: spacedrive daemon start");
        }
    }

    Ok(())
}

async fn handle_instance_command(cmd: InstanceCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InstanceCommands::List => match Daemon::list_instances() {
            Ok(instances) => {
                if instances.is_empty() {
                    println!("📭 No daemon instances found");
                } else {
                    let mut table = Table::new();
                    table.set_header(vec!["Instance", "Status", "Socket Path"]);

                    for instance in instances {
                        let status = if instance.is_running {
                            "Running".green()
                        } else {
                            "Stopped".red()
                        };

                        table.add_row(vec![
                            instance.display_name().to_string(),
                            status.to_string(),
                            instance.socket_path.display().to_string(),
                        ]);
                    }

                    println!("{}", table);
                }
            }
            Err(e) => {
                println!("❌ Failed to list instances: {}", e);
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
                    println!("✅ Daemon instance '{}' stopped", name);
                }
                Err(e) => {
                    println!("❌ Failed to stop instance '{}': {}", name, e);
                }
            }
        }

        InstanceCommands::Current => {
            // This would show the current instance based on CLI args or context
            println!("Current instance functionality not yet implemented");
            println!("Use --instance <name> flag to target specific instances");
        }
    }

    Ok(())
}