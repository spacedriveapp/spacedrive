//! Color definitions and utilities for consistent CLI styling

use crossterm::style::{Color, Stylize};
use sd_core::infra::job::types::JobStatus;

/// Color scheme for the CLI
pub struct Colors;

impl Colors {
	pub const SUCCESS: Color = Color::Green;
	pub const ERROR: Color = Color::Red;
	pub const WARNING: Color = Color::Yellow;
	pub const INFO: Color = Color::Blue;
	pub const MUTED: Color = Color::DarkGrey;
	pub const ACCENT: Color = Color::Cyan;
	pub const PROGRESS_COMPLETE: Color = Color::Green;
	pub const PROGRESS_ACTIVE: Color = Color::Blue;
	pub const PROGRESS_BACKGROUND: Color = Color::DarkGrey;
}

/// Get color for job status
pub fn job_status_color(status: JobStatus) -> Color {
	match status {
		JobStatus::Queued => Colors::MUTED,
		JobStatus::Running => Colors::WARNING,
		JobStatus::Paused => Colors::INFO,
		JobStatus::Completed => Colors::SUCCESS,
		JobStatus::Failed => Colors::ERROR,
		JobStatus::Cancelled => Colors::MUTED,
	}
}

/// Get status icon for job
pub fn job_status_icon(status: JobStatus) -> &'static str {
	match status {
		JobStatus::Queued => "⏳",
		JobStatus::Running => "⚡",
		JobStatus::Paused => "⏸️",
		JobStatus::Completed => "✅",
		JobStatus::Failed => "❌",
		JobStatus::Cancelled => "🚫",
	}
}

/// Format job status with color and icon
pub fn format_job_status(status: JobStatus) -> String {
	format!(
		"{} {}",
		job_status_icon(status),
		status.to_string().with(job_status_color(status))
	)
}

/// Spinner characters for animated progress
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Get spinner character for frame
pub fn spinner_char(frame: usize) -> char {
	SPINNER_CHARS[frame % SPINNER_CHARS.len()]
}
