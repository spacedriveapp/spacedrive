//! Job management commands
//!
//! This module handles CLI commands for managing jobs:
//! - Listing jobs with optional filtering
//! - Getting detailed job information
//! - Monitoring job progress in real-time

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::monitoring;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use uuid::Uuid;

#[derive(Subcommand, Clone, Debug)]
pub enum JobCommands {
    /// List all jobs
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
        /// Show only recent jobs
        #[arg(short, long)]
        recent: bool,
    },

    /// Get detailed information about a job
    Info {
        /// Job ID (can be partial)
        id: String,
    },

    /// Monitor job progress in real-time
    Monitor {
        /// Specific job ID to monitor
        job_id: Option<String>,
        /// Exit when job completes
        #[arg(short, long)]
        exit_on_complete: bool,
    },

    /// Pause a job
    Pause {
        /// Job ID to pause
        id: String,
    },

    /// Resume a paused job
    Resume {
        /// Job ID to resume
        id: String,
    },

    /// Cancel a job
    Cancel {
        /// Job ID to cancel
        id: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Clear completed or failed jobs
    Clear {
        /// Only clear failed jobs
        #[arg(short, long)]
        failed: bool,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

pub async fn handle_job_command(
    cmd: JobCommands,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

    match cmd {
        JobCommands::List { status, recent: _ } => {
            let status_filter = status.map(|s| s.to_lowercase());

            match client
                .send_command(DaemonCommand::ListJobs {
                    status: status_filter,
                })
                .await
            {
                Ok(DaemonResponse::Jobs(jobs)) => {
                    if jobs.is_empty() {
                        println!("📭 No jobs found");
                    } else {
                        let mut table = Table::new();
                        table.set_header(vec!["ID", "Name", "Status", "Progress"]);

                        for job in jobs {
                            let progress_str = if job.status == "running" {
                                format!("{}%", (job.progress * 100.0) as u32)
                            } else {
                                "-".to_string()
                            };

                            let status_colored = match job.status.as_str() {
                                "running" => job.status.bright_yellow(),
                                "completed" => job.status.bright_green(),
                                "failed" => job.status.bright_red(),
                                _ => job.status.normal(),
                            };

                            table.add_row(vec![
                                job.id.to_string(),
                                job.name,
                                status_colored.to_string(),
                                progress_str,
                            ]);
                        }

                        println!("{}", table);
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to list jobs: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        JobCommands::Info { id } => {
            // Parse the job ID string to UUID
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid job ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::GetJobInfo { id: job_id })
                .await
            {
                Ok(DaemonResponse::JobInfo(Some(job))) => {
                    println!("📋 Job Information");
                    println!("   ID: {}", job.id.to_string().bright_yellow());
                    println!("   Name: {}", job.name.bright_cyan());
                    println!(
                        "   Status: {}",
                        match job.status.as_str() {
                            "running" => job.status.bright_yellow(),
                            "completed" => job.status.bright_green(),
                            "failed" => job.status.bright_red(),
                            _ => job.status.normal(),
                        }
                    );
                    println!("   Progress: {}%", (job.progress * 100.0) as u32);
                }
                Ok(DaemonResponse::JobInfo(None)) => {
                    println!("❌ Job not found");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Error: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        JobCommands::Monitor { job_id, exit_on_complete } => {
            monitoring::daemon_monitor::monitor_jobs(&mut client, job_id).await?;
        }

        JobCommands::Pause { id } => {
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid job ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::PauseJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Job paused successfully");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to pause job: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        JobCommands::Resume { id } => {
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid job ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::ResumeJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Job resumed successfully");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to resume job: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        JobCommands::Cancel { id, yes } => {
            if !yes {
                use dialoguer::Confirm;
                let confirm = Confirm::new()
                    .with_prompt(format!("Are you sure you want to cancel job '{}'?", id))
                    .default(false)
                    .interact()?;
                
                if !confirm {
                    println!("Operation cancelled");
                    return Ok(());
                }
            }
            
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    println!("❌ Invalid job ID format");
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::CancelJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Job cancelled successfully");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to cancel job: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        JobCommands::Clear { failed, yes } => {
            if !yes {
                use dialoguer::Confirm;
                let confirm = Confirm::new()
                    .with_prompt(if failed { 
                        "Are you sure you want to clear failed jobs?" 
                    } else { 
                        "Are you sure you want to clear all completed jobs?" 
                    })
                    .default(false)
                    .interact()?;
                
                if !confirm {
                    println!("Operation cancelled");
                    return Ok(());
                }
            }
            
            println!("❌ Clear command not yet implemented");
            println!("   This command will be available in a future update");
        }
    }

    Ok(())
}