//! Reusable progress bar primitives and utilities

use crossterm::style::{Color, Stylize};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Duration;
use uuid::Uuid;

use super::colors::{Colors, job_status_color, job_status_icon};
use sd_core::infra::job::types::JobStatus;

/// Configuration for progress bars
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    pub width: u16,
    pub show_percentage: bool,
    pub show_eta: bool,
    pub show_speed: bool,
    pub template: Option<String>,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            width: 40,
            show_percentage: true,
            show_eta: true,
            show_speed: false,
            template: None,
        }
    }
}

/// A reusable progress bar for jobs
pub struct JobProgressBar {
    pub id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub progress: f32,
    pub bar: ProgressBar,
    pub spinner_frame: usize,
}

impl JobProgressBar {
    /// Create a new job progress bar
    pub fn new(id: Uuid, name: String, status: JobStatus, progress: f32) -> Self {
        let bar = ProgressBar::new(100);
        let mut instance = Self {
            id,
            name,
            status,
            progress,
            bar,
            spinner_frame: 0,
        };
        instance.update_style();
        instance.update_progress();
        instance
    }

    /// Update the progress bar style based on job status
    pub fn update_style(&mut self) {
        let style = match self.status {
            JobStatus::Running => {
                ProgressStyle::with_template(
                    "{spinner:.yellow} {msg} [{bar:40.blue/grey}] {percent}% | {pos}/{len}"
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ ")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            }
            JobStatus::Completed => {
                ProgressStyle::with_template(
                    "{msg} [{bar:40.green/grey}] {percent}%"
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ ")
            }
            JobStatus::Failed => {
                ProgressStyle::with_template(
                    "❌ {msg} [{bar:40.red/grey}] {percent}%"
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ ")
            }
            JobStatus::Cancelled => {
                ProgressStyle::with_template(
                    "🚫 {msg} [{bar:40.grey/grey}] {percent}%"
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ ")
            }
            JobStatus::Paused => {
                ProgressStyle::with_template(
                    "⏸️ {msg} [{bar:40.cyan/grey}] {percent}% | Paused"
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ ")
            }
            JobStatus::Queued => {
                ProgressStyle::with_template(
                    "⏳ {msg} [{bar:40.grey/grey}] Queued"
                )
                .unwrap()
                .progress_chars("░░░░░░░░░░")
            }
        };

        self.bar.set_style(style);
        self.bar.set_message(format!("{} [{}]", self.name, self.id.to_string()[..8].to_string()));
    }

    /// Update progress value
    pub fn update_progress(&mut self) {
        let position = (self.progress * 100.0) as u64;
        self.bar.set_position(position);
    }

    /// Update job status and refresh style
    pub fn update_status(&mut self, status: JobStatus) {
        if self.status != status {
            self.status = status;
            self.update_style();
        }
    }

    /// Update progress value
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
        self.update_progress();
    }

    /// Tick the spinner for running jobs
    pub fn tick(&mut self) {
        if self.status == JobStatus::Running {
            self.spinner_frame = (self.spinner_frame + 1) % 10;
            self.bar.tick();
        }
    }

    /// Finish the progress bar
    pub fn finish(&mut self) {
        match self.status {
            JobStatus::Completed => {
                self.bar.finish_with_message(format!(
                    "{} {} [{}] Complete",
                    job_status_icon(self.status),
                    self.name,
                    self.id.to_string()[..8].to_string()
                ));
            }
            JobStatus::Failed => {
                self.bar.finish_with_message(format!(
                    "{} {} [{}] Failed",
                    job_status_icon(self.status),
                    self.name,
                    self.id.to_string()[..8].to_string()
                ));
            }
            JobStatus::Cancelled => {
                self.bar.finish_with_message(format!(
                    "{} {} [{}] Cancelled",
                    job_status_icon(self.status),
                    self.name,
                    self.id.to_string()[..8].to_string()
                ));
            }
            _ => {
                self.bar.finish_and_clear();
            }
        }
    }
}

/// Manager for multiple progress bars
pub struct JobProgressManager {
    pub multi: MultiProgress,
    pub bars: std::collections::HashMap<Uuid, JobProgressBar>,
    pub config: ProgressConfig,
}

impl JobProgressManager {
    /// Create a new progress manager
    pub fn new(config: ProgressConfig) -> Self {
        Self {
            multi: MultiProgress::new(),
            bars: std::collections::HashMap::new(),
            config,
        }
    }

    /// Add a new job progress bar
    pub fn add_job(&mut self, id: Uuid, name: String, status: JobStatus, progress: f32) {
        let mut job_bar = JobProgressBar::new(id, name, status, progress);
        let bar = self.multi.add(job_bar.bar.clone());
        job_bar.bar = bar;
        self.bars.insert(id, job_bar);
    }

    /// Update a job's progress
    pub fn update_job(&mut self, id: Uuid, status: JobStatus, progress: f32) {
        if let Some(job_bar) = self.bars.get_mut(&id) {
            job_bar.update_status(status);
            job_bar.set_progress(progress);
        }
    }

    /// Remove a completed job
    pub fn remove_job(&mut self, id: Uuid) {
        if let Some(mut job_bar) = self.bars.remove(&id) {
            job_bar.finish();
        }
    }

    /// Tick all running jobs
    pub fn tick_all(&mut self) {
        for job_bar in self.bars.values_mut() {
            job_bar.tick();
        }
    }

    /// Get count of jobs by status
    pub fn count_by_status(&self, status: JobStatus) -> usize {
        self.bars.values().filter(|bar| bar.status == status).count()
    }

    /// Clear all completed jobs
    pub fn clear_completed(&mut self) {
        let completed_ids: Vec<Uuid> = self.bars
            .iter()
            .filter(|(_, bar)| bar.status.is_terminal())
            .map(|(id, _)| *id)
            .collect();

        for id in completed_ids {
            self.remove_job(id);
        }
    }
}

/// Simple progress bar for single operations
pub fn create_simple_progress(message: &str, total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {eta}"
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏ ")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Create a spinner for indeterminate progress
pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}
