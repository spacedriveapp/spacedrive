//! Daemon-based monitoring implementation

use crate::infrastructure::cli::{
    daemon::{self, DaemonClient},
    utils::{format_bytes, progress_styles},
};
use colored::*;
use indicatif::{MultiProgress, ProgressBar};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Monitor jobs through the daemon
pub async fn monitor_jobs(
    client: &mut DaemonClient,
    job_id: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "📡 {} - Press Ctrl+C to exit",
        "Spacedrive Job Monitor".bright_cyan()
    );
    println!("═══════════════════════════════════════════");
    println!();

    // Create progress bars for active jobs
    let multi_progress = MultiProgress::new();
    let mut job_bars: HashMap<String, ProgressBar> = HashMap::new();

    let style = progress_styles::basic_style();

    // If monitoring specific job
    if let Some(ref specific_job_id) = job_id {
        println!(
            "📊 Monitoring job {}...\n",
            specific_job_id
                .chars()
                .take(8)
                .collect::<String>()
                .bright_yellow()
        );
    } else {
        println!("⏳ Monitoring all jobs...\n");
    }

    // Poll for job updates
    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                // Get job list
                match client.send_command(daemon::DaemonCommand::ListJobs { status: Some("running".to_string()) }).await {
                    Ok(daemon::DaemonResponse::Jobs(jobs)) => {
                        // Track which jobs are still active
                        let mut active_job_ids = std::collections::HashSet::new();
                        
                        for job in &jobs {
                            // Filter by specific job if requested
                            if let Some(ref specific_id) = job_id {
                                if !job.id.to_string().starts_with(specific_id) {
                                    continue;
                                }
                            }

                            active_job_ids.insert(job.id.to_string());

                            // Create or update progress bar
                            let job_key = job.id.to_string();
                            let pb = job_bars.entry(job_key.clone()).or_insert_with(|| {
                                let bar = multi_progress.add(ProgressBar::new(100));
                                bar.set_style(style.clone());
                                bar.set_prefix(format!("[{}]", job.name.bright_cyan()));
                                bar
                            });

                            // Update progress
                            pb.set_position((job.progress * 100.0) as u64);
                            pb.set_message(format!(
                                "{} • {}",
                                format!("{:.1}%", job.progress * 100.0).bright_green(),
                                job.status.to_string().dimmed()
                            ));

                            // If job is no longer running, finish the bar
                            if job.status.to_string() == "Completed" {
                                pb.finish_with_message(format!("✅ {}", "Completed".bright_green()));
                                job_bars.remove(&job_key);
                            } else if job.status.to_string() == "Failed" {
                                pb.abandon_with_message(format!("❌ {}", "Failed".bright_red()));
                                job_bars.remove(&job_key);
                            }
                        }

                        // Clean up progress bars for jobs that are no longer active
                        let keys_to_remove: Vec<String> = job_bars.keys()
                            .filter(|k| !active_job_ids.contains(*k))
                            .cloned()
                            .collect();
                        
                        for key in keys_to_remove {
                            if let Some(pb) = job_bars.remove(&key) {
                                pb.finish_and_clear();
                            }
                        }

                        // If we have specific job and no jobs found, exit
                        if job_id.is_some() && jobs.is_empty() {
                            println!("Job not found or not running");
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Error getting job list: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            _ = tokio::signal::ctrl_c() => {
                println!("\n\n👋 Exiting monitor...");
                break;
            }
        }
    }

    Ok(())
}

/// Monitor a specific job by ID through daemon
pub async fn monitor_job_by_id(
    client: &mut DaemonClient,
    job_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    monitor_jobs(client, Some(job_id.to_string())).await
}