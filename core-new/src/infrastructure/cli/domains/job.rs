//! Job management commands
//!
//! This module handles CLI commands for managing jobs:
//! - Listing jobs with optional filtering
//! - Getting detailed job information
//! - Monitoring job progress in real-time

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::monitoring;
use crate::infrastructure::cli::output::{CliOutput, Message};
use crate::infrastructure::cli::output::messages::{JobInfo as OutputJobInfo, JobStatus as OutputJobStatus};
use clap::Subcommand;
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
    mut output: CliOutput,
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
                        output.info("No jobs found")?;
                    } else {
                        let output_jobs: Vec<OutputJobInfo> = jobs.into_iter()
                            .map(|job| OutputJobInfo {
                                id: job.id,
                                name: job.name.clone(),
                                status: match job.status.as_str() {
                                    "queued" => OutputJobStatus::Queued,
                                    "running" => OutputJobStatus::Running,
                                    "completed" => OutputJobStatus::Completed,
                                    "failed" => OutputJobStatus::Failed,
                                    "paused" => OutputJobStatus::Paused,
                                    _ => OutputJobStatus::Queued,
                                },
                                progress: Some(job.progress),
                                started_at: 0, // TODO: Get actual timestamp from daemon
                                completed_at: None,
                            })
                            .collect();
                        
                        if matches!(output.format(), crate::infrastructure::cli::output::OutputFormat::Json) {
                            output.print(Message::JobList { jobs: output_jobs })?;
                        } else {
                            // For human output, use a table
                            let mut table = Table::new();
                            table.set_header(vec!["ID", "Name", "Status", "Progress"]);

                            for job in output_jobs {
                                let progress_str = match job.status {
                                    OutputJobStatus::Running => format!("{}%", (job.progress.unwrap_or(0.0) * 100.0) as u32),
                                    _ => "-".to_string(),
                                };

                                table.add_row(vec![
                                    job.id.to_string(),
                                    job.name,
                                    format!("{:?}", job.status),
                                    progress_str,
                                ]);
                            }

                            output.section()
                                .table(table)
                                .render()?;
                        }
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Failed to list jobs: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        JobCommands::Info { id } => {
            // Parse the job ID string to UUID
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    output.error(Message::Error("Invalid job ID format".to_string()))?;
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::GetJobInfo { id: job_id })
                .await
            {
                Ok(DaemonResponse::JobInfo(Some(job))) => {
                    output.section()
                        .title("Job Information")
                        .item("ID", &job.id.to_string())
                        .item("Name", &job.name)
                        .item("Status", &job.status)
                        .item("Progress", &format!("{}%", (job.progress * 100.0) as u32))
                        .render()?;
                }
                Ok(DaemonResponse::JobInfo(None)) => {
                    output.error(Message::Error("Job not found".to_string()))?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Error: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        JobCommands::Monitor { job_id, exit_on_complete } => {
            // Monitor command handles its own output
            monitoring::daemon_monitor::monitor_jobs(&mut client, job_id).await?;
        }

        JobCommands::Pause { id } => {
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    output.error(Message::Error("Invalid job ID format".to_string()))?;
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::PauseJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Job paused successfully")?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Failed to pause job: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }

        JobCommands::Resume { id } => {
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    output.error(Message::Error("Invalid job ID format".to_string()))?;
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::ResumeJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Job resumed successfully")?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Failed to resume job: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
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
                    output.info("Operation cancelled")?;
                    return Ok(());
                }
            }
            
            let job_id = match id.parse::<Uuid>() {
                Ok(uuid) => uuid,
                Err(_) => {
                    output.error(Message::Error("Invalid job ID format".to_string()))?;
                    return Ok(());
                }
            };
            
            match client
                .send_command(DaemonCommand::CancelJob { id: job_id })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Job cancelled successfully")?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Failed to cancel job: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
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
                    output.info("Operation cancelled")?;
                    return Ok(());
                }
            }
            
            output.error(Message::Error("Clear command not yet implemented".to_string()))?;
            output.info("This command will be available in a future update")?;
        }
    }

    Ok(())
}