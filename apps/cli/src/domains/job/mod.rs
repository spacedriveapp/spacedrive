mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;
use crate::ui::{create_simple_progress};

use crate::context::Context;
use sd_core::ops::{
    jobs::{
        info::output::JobInfoOutput,
        list::output::JobListOutput,
        control::{
            pause::{JobPauseInput, JobPauseOutput},
            resume::{JobResumeInput, JobResumeOutput},
            cancel::{JobCancelInput, JobCancelOutput},
        },
    },
    libraries::list::query::ListLibrariesQuery,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum JobCmd {
    /// List jobs
    List(JobListArgs),
    /// Job info
    Info(JobInfoArgs),
    /// Monitor jobs with real-time progress
    Monitor(JobMonitorArgs),
    /// Pause a job
    Pause(JobControlArgs),
    /// Resume a job
    Resume(JobControlArgs),
    /// Cancel a job
    Cancel(JobControlArgs),
}

pub async fn run(ctx: &Context, cmd: JobCmd) -> Result<()> {
    match cmd {
        JobCmd::List(args) => {
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
            if libs.is_empty() {
                println!("No libraries found");
                return Ok(());
            }

            for lib in libs {
                let out: JobListOutput = execute_query!(ctx, args.to_query(lib.id));
                print_output!(ctx, &out, |o: &JobListOutput| {
                    for j in &o.jobs {
                        println!(
                            "- {} {} {} {:?}",
                            j.id,
                            j.name,
                            (j.progress * 100.0) as u32,
                            j.status
                        );
                    }
                });
            }
        }
        JobCmd::Info(args) => {
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
            let _lib = libs.get(0).ok_or_else(|| anyhow::anyhow!("No libraries found"))?;

            let out: Option<JobInfoOutput> = execute_query!(ctx, args.to_query());
            print_output!(ctx, &out, |o: &Option<JobInfoOutput>| {
                match o {
                    Some(j) => println!(
                        "{} {} {}% {:?}",
                        j.id,
                        j.name,
                        (j.progress * 100.0) as u32,
                        j.status
                    ),
                    None => println!("Job not found"),
                }
            });
        }
        JobCmd::Monitor(args) => {
            run_job_monitor(ctx, args).await?;
        }
        JobCmd::Pause(args) => {
            let input = JobPauseInput::new(args.job_id);
            let out: JobPauseOutput = execute_action!(ctx, input);
            print_output!(ctx, &out, |o: &JobPauseOutput| {
                if o.success {
                    println!("Job {} paused successfully", o.job_id);
                } else {
                    println!("❌ Failed to pause job {}", o.job_id);
                }
            });
        }
        JobCmd::Resume(args) => {
            let input = JobResumeInput::new(args.job_id);
            let out: JobResumeOutput = execute_action!(ctx, input);
            print_output!(ctx, &out, |o: &JobResumeOutput| {
                if o.success {
                    println!("Job {} resumed successfully", o.job_id);
                } else {
                    println!("❌ Failed to resume job {}", o.job_id);
                }
            });
        }
        JobCmd::Cancel(args) => {
            let input = JobCancelInput::new(args.job_id);
            let out: JobCancelOutput = execute_action!(ctx, input);
            print_output!(ctx, &out, |o: &JobCancelOutput| {
                if o.success {
                    println!("Job {} cancelled successfully", o.job_id);
                } else {
                    println!("❌ Failed to cancel job {}", o.job_id);
                }
            });
        }
    }
    Ok(())
}

/// Run the job monitor with either TUI or simple progress bars
async fn run_job_monitor(ctx: &Context, args: JobMonitorArgs) -> Result<()> {
    use std::time::Duration;
    use tokio::time::sleep;

    if args.simple {
        // Simple progress bar mode
        run_simple_job_monitor(ctx, args).await
    } else {
        // Full TUI mode
        run_tui_job_monitor(ctx, args).await
    }
}

/// Run simple progress bar monitoring with real-time events
async fn run_simple_job_monitor(ctx: &Context, args: JobMonitorArgs) -> Result<()> {
    use sd_core::infra::daemon::types::EventFilter;
    use sd_core::infra::event::Event;
    use std::collections::HashMap;

    println!("📡 Monitoring jobs (real-time mode) - Press Ctrl+C to exit");
    println!("═══════════════════════════════════════════════════════");

    // Subscribe to job events
    let event_types = vec![
        "JobProgress".to_string(),
        "JobStarted".to_string(),
        "JobCompleted".to_string(),
        "JobFailed".to_string(),
        "JobCancelled".to_string(),
        "JobPaused".to_string(),
        "JobResumed".to_string(),
    ];

    let filter = args.job_id.map(|job_id| EventFilter {
        library_id: None,
        job_id: Some(job_id.to_string()),
        device_id: None,
    });

    // Try to subscribe to events, fall back to polling if not supported
    match ctx.core.subscribe_events(event_types, filter).await {
        Ok(mut event_stream) => {
            println!("Connected to real-time event stream");

            let mut progress_bars = HashMap::new();

            // Preload currently running jobs so we see in-progress jobs that started earlier
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> =
                execute_query!(ctx, ListLibrariesQuery::basic());

            for lib in libs {
                let query = JobListArgs { status: args.status.clone() }.to_query(lib.id);
                let job_list: sd_core::ops::jobs::list::output::JobListOutput = execute_query!(ctx, query);

                for job in job_list.jobs {
                    // If monitoring a specific job, skip others
                    if let Some(target_id) = args.job_id {
                        if job.id != target_id { continue; }
                    }

                    // Only show non-terminal jobs
                    if !job.status.is_terminal() && !progress_bars.contains_key(&job.id.to_string()) {
                        let pb = crate::ui::create_simple_progress(&job.name, 100);
                        pb.set_message(format!("{} [{}] - Resuming...", job.name, &job.id.to_string()[..8]));
                        pb.set_position((job.progress * 100.0) as u64);
                        progress_bars.insert(job.id.to_string(), pb);
                    }
                }
            }

            // Listen for events
            while let Some(event) = event_stream.recv().await {
                match event {
                    Event::JobStarted { job_id, job_type } => {
                        println!("🚀 Job started: {} [{}]", job_type, &job_id[..8]);
                        let pb = crate::ui::create_simple_progress(&job_type, 100);
                        pb.set_message(format!("{} [{}] - Starting...", job_type, &job_id[..8]));
                        progress_bars.insert(job_id, pb);
                    }

                    Event::JobProgress { job_id, job_type, progress, message, .. } => {
                        // If we connected mid-job, create a bar on first progress event
                        let pb = progress_bars.entry(job_id.clone()).or_insert_with(|| {
                            let pb = crate::ui::create_simple_progress(&job_type, 100);
                            pb.set_message(format!("{} [{}] - Starting...", job_type, &job_id[..8]));
                            pb
                        });

                        let msg = message.unwrap_or_else(|| "Processing...".to_string());
                        pb.set_message(format!("{} [{}] - {}", job_type, &job_id[..8], msg));
                        pb.set_position((progress * 100.0) as u64);
                    }

                    Event::JobCompleted { job_id, job_type, .. } => {
                        if let Some(pb) = progress_bars.get(&job_id) {
                            pb.finish_with_message(format!("{} [{}] - Completed", job_type, &job_id[..8]));
                            progress_bars.remove(&job_id);
                        }
                        println!("Job completed: {} [{}]", job_type, &job_id[..8]);
                    }

                    Event::JobFailed { job_id, job_type, error } => {
                        if let Some(pb) = progress_bars.get(&job_id) {
                            pb.finish_with_message(format!("❌ {} [{}] - Failed", job_type, &job_id[..8]));
                            progress_bars.remove(&job_id);
                        }
                        println!("❌ Job failed: {} [{}] - {}", job_type, &job_id[..8], error);
                    }

                    Event::JobCancelled { job_id, job_type } => {
                        if let Some(pb) = progress_bars.get(&job_id) {
                            pb.finish_with_message(format!("🚫 {} [{}] - Cancelled", job_type, &job_id[..8]));
                            progress_bars.remove(&job_id);
                        }
                        println!("🚫 Job cancelled: {} [{}]", job_type, &job_id[..8]);
                    }

                    Event::JobPaused { job_id } => {
                        if let Some(pb) = progress_bars.get(&job_id) {
                            pb.set_message(format!("⏸️ Job paused [{}]", &job_id[..8]));
                        }
                        println!("⏸️ Job paused: [{}]", &job_id[..8]);
                    }

                    Event::JobResumed { job_id } => {
                        if let Some(pb) = progress_bars.get(&job_id) {
                            pb.set_message(format!("▶️ Job resumed [{}]", &job_id[..8]));
                        }
                        println!("▶️ Job resumed: [{}]", &job_id[..8]);
                    }

                    _ => {} // Ignore other events
                }
            }
        }

        Err(_) => {
            // Fall back to polling mode
            println!("⚠️ Real-time events not available, using polling mode");
            run_polling_job_monitor(ctx, args).await?;
        }
    }

    Ok(())
}

/// Fallback polling-based job monitoring
async fn run_polling_job_monitor(ctx: &Context, args: JobMonitorArgs) -> Result<()> {
    use crate::ui::{create_simple_progress};
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::time::sleep;

    let mut progress_bars = HashMap::new();
    let refresh_duration = Duration::from_secs(args.refresh);

    loop {
        // Get current jobs
        let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> =
            execute_query!(ctx, ListLibrariesQuery::basic());

        if libs.is_empty() {
            println!("No libraries found");
            break;
        }

        for lib in libs {
            let query = JobListArgs {
                status: args.status.clone()
            }.to_query(lib.id);

            let job_list: JobListOutput = execute_query!(ctx, query);

            for job in job_list.jobs {
                if let Some(target_id) = args.job_id {
                    if job.id != target_id {
                        continue;
                    }
                }

                // Update or create progress bar
                if !progress_bars.contains_key(&job.id) {
                    let pb = create_simple_progress(&job.name, 100);
                    progress_bars.insert(job.id, pb);
                }

                if let Some(pb) = progress_bars.get(&job.id) {
                    pb.set_position((job.progress * 100.0) as u64);

                    if job.status.is_terminal() {
                        pb.finish_with_message(format!(
                            "{} {} - {}",
                            crate::ui::job_status_icon(job.status),
                            job.name,
                            job.status
                        ));
                        progress_bars.remove(&job.id);
                    }
                }
            }
        }

        sleep(refresh_duration).await;
    }

    Ok(())
}

/// Run TUI job monitor
async fn run_tui_job_monitor(_ctx: &Context, _args: JobMonitorArgs) -> Result<()> {
    println!("📡 TUI Job Monitor");
    println!("TUI implementation is being refined. Use --simple for now:");
    println!("  sd job monitor --simple");
    Ok(())
}
