//! Logs domain for viewing and following daemon logs

mod args;

pub use args::*;

use crate::context::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Run logs command
pub async fn run(ctx: &Context, cmd: LogsCmd) -> Result<()> {
	match cmd {
		LogsCmd::Show(args) => run_logs_show(ctx, args).await,
		LogsCmd::Follow(args) => run_logs_follow(ctx, args).await,
	}
}

/// Show recent logs
async fn run_logs_show(ctx: &Context, args: LogsShowArgs) -> Result<()> {
	use std::fs::File;
	use std::io::{BufRead, BufReader};

	println!("Recent logs (last {} lines)", args.lines);

	// Get the daemon log file path
	let log_file_path = get_daemon_log_path(ctx).await?;

	if !log_file_path.exists() {
		println!("No log file found at: {}", log_file_path.display());
		println!("Start the daemon to begin logging");
		return Ok(());
	}

	// Read the last N lines from the log file
	let lines = read_last_lines(&log_file_path, args.lines)?;

	for line in lines {
		if let Some(formatted) = format_log_line(&line, &args) {
			println!("{}", formatted);
		}
	}

	Ok(())
}

/// Follow logs in real-time
async fn run_logs_follow(ctx: &Context, args: LogsFollowArgs) -> Result<()> {
	use sd_core::infra::daemon::types::EventFilter;
	use sd_core::infra::event::Event;

	println!("Following logs in real-time - Press Ctrl+C to exit");
	if let Some(ref level) = args.level {
		println!("Filtering by level: {}", level);
	}
	if let Some(ref component) = args.component {
		println!("Filtering by component: {}", component);
	}
	if let Some(ref job_id) = args.job_id {
		println!("Filtering by job: {}", job_id);
	}
	println!("═══════════════════════════════════════════════════════");

	// First, show recent historical logs
	show_recent_historical_logs(ctx, &args).await?;

	// Subscribe to log messages via the dedicated LogBus
	match ctx
		.core
		.subscribe_logs(
			args.job_id.clone(),
			args.level.clone(),
			args.component.clone(),
		)
		.await
	{
		Ok(mut log_stream) => {
			println!("Connected to real-time log stream");

			// Listen for log messages
			while let Some(log_msg) = log_stream.recv().await {
				// Apply client-side filters (server-side filtering already applied)

				// Format and display the log message
				let formatted_time = format_timestamp(log_msg.timestamp, args.timestamps);
				let level_colored = colorize_level(&log_msg.level);
				let target_formatted = if args.verbose {
					format!(" {}", log_msg.target)
				} else {
					String::new()
				};

				let job_info = if args.show_job_id {
					log_msg
						.job_id
						.as_ref()
						.map(|id| format!(" [job:{}]", &id[..8]))
						.unwrap_or_default()
				} else {
					String::new()
				};

				let library_info = if args.show_library_id {
					log_msg
						.library_id
						.as_ref()
						.map(|id| format!(" [lib:{}]", &id.to_string()[..8]))
						.unwrap_or_default()
				} else {
					String::new()
				};

				println!(
					"{}{}{}{}{} {}",
					formatted_time,
					level_colored,
					target_formatted,
					job_info,
					library_info,
					log_msg.message
				);
			}
		}

		Err(_) => {
			println!("Real-time log streaming not available");
			println!("Make sure the daemon is running with log streaming support");
		}
	}

	Ok(())
}

/// Check if a log level matches the filter
fn level_matches(log_level: &str, filter_level: &str) -> bool {
	let level_priority = |level: &str| match level.to_uppercase().as_str() {
		"ERROR" => 0,
		"WARN" => 1,
		"INFO" => 2,
		"DEBUG" => 3,
		"TRACE" => 4,
		_ => 5,
	};

	level_priority(log_level) <= level_priority(filter_level)
}

/// Format timestamp based on user preference
fn format_timestamp(timestamp: DateTime<Utc>, show_timestamps: bool) -> String {
	if show_timestamps {
		format!("[{}] ", timestamp.format("%H:%M:%S%.3f"))
	} else {
		String::new()
	}
}

/// Colorize log level for better readability
fn colorize_level(level: &str) -> String {
	match level.to_uppercase().as_str() {
		"ERROR" => format!("{:<5}", level),
		"WARN" => format!(" {:<5}", level),
		"INFO" => format!(" {:<5}", level),
		"DEBUG" => format!("{:<5}", level),
		"TRACE" => format!("{:<5}", level),
		_ => format!("   {:<5}", level),
	}
}

/// Get the daemon log file path by querying the daemon's data directory
async fn get_daemon_log_path(_ctx: &Context) -> Result<std::path::PathBuf> {
	// Try to get the data directory from the daemon
	// For now, use the standard location
	let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
	let data_dir =
		std::path::PathBuf::from(home_dir).join("Library/Application Support/spacedrive");

	let logs_dir = data_dir.join("logs");

	// Check for today's log file first (with date suffix due to daily rotation)
	let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
	let today_log = logs_dir.join(format!("daemon.log.{}", today));

	if today_log.exists() {
		return Ok(today_log);
	}

	// Fall back to the base log file name
	let base_log = logs_dir.join("daemon.log");
	if base_log.exists() {
		return Ok(base_log);
	}

	// If neither exists, find the most recent log file
	if let Ok(entries) = std::fs::read_dir(&logs_dir) {
		let mut log_files: Vec<_> = entries
			.filter_map(|entry| entry.ok())
			.filter(|entry| {
				entry
					.file_name()
					.to_string_lossy()
					.starts_with("daemon.log")
			})
			.collect();

		// Sort by modification time, most recent first
		log_files.sort_by(|a, b| {
			let a_time = a
				.metadata()
				.and_then(|m| m.modified())
				.unwrap_or(std::time::UNIX_EPOCH);
			let b_time = b
				.metadata()
				.and_then(|m| m.modified())
				.unwrap_or(std::time::UNIX_EPOCH);
			b_time.cmp(&a_time)
		});

		if let Some(most_recent) = log_files.first() {
			return Ok(most_recent.path());
		}
	}

	// Default to today's log file path even if it doesn't exist yet
	Ok(today_log)
}

/// Read the last N lines from a file efficiently
fn read_last_lines(file_path: &std::path::Path, n: usize) -> Result<Vec<String>> {
	use std::fs::File;
	use std::io::{BufRead, BufReader, Seek, SeekFrom};

	let file = File::open(file_path)?;
	let mut reader = BufReader::new(file);

	// For simplicity, read all lines and take the last N
	// TODO: Optimize for large files by reading from the end
	let lines: Result<Vec<String>, _> = reader.lines().collect();
	let all_lines = lines?;

	let start_idx = if all_lines.len() > n {
		all_lines.len() - n
	} else {
		0
	};

	Ok(all_lines[start_idx..].to_vec())
}

/// Format a log line based on user preferences
fn format_log_line(line: &str, args: &LogsShowArgs) -> Option<String> {
	// Parse the log line format: timestamp LEVEL ThreadId(N) target: message
	// Example: 2025-09-19T02:25:54.897283Z DEBUG ThreadId(13) sd_core::infra::event: Event emitted to subscribers

	let parts: Vec<&str> = line.splitn(5, ' ').collect();
	if parts.len() < 5 {
		return Some(line.to_string()); // Return as-is if we can't parse
	}

	let timestamp = parts[0];
	let level = parts[1];
	let _thread_id = parts[2]; // ThreadId(N)
	let target = parts[3].trim_end_matches(':');
	let message = parts[4];

	// Apply filters
	if let Some(ref filter_level) = args.level {
		if !level_matches(level, filter_level) {
			return None;
		}
	}

	if let Some(ref filter_component) = args.component {
		if !target.contains(filter_component) {
			return None;
		}
	}

	// Format output
	let formatted_time = if args.timestamps {
		format!("[{}] ", timestamp)
	} else {
		String::new()
	};

	let level_colored = colorize_level(level);

	let target_formatted = if args.verbose {
		format!(" {}", target)
	} else {
		String::new()
	};

	Some(format!(
		"{}{}{} {}",
		formatted_time, level_colored, target_formatted, message
	))
}

/// Show recent historical logs before starting real-time streaming
async fn show_recent_historical_logs(ctx: &Context, args: &LogsFollowArgs) -> Result<()> {
	let log_file_path = get_daemon_log_path(ctx).await?;

	if !log_file_path.exists() {
		println!("No historical logs found");
		return Ok(());
	}

	println!("Recent logs:");
	println!("───────────────");

	// Show last 2lines of historical logs
	let lines = read_last_lines(&log_file_path, 20)?;

	for line in lines {
		// Convert LogsFollowArgs to LogsShowArgs for formatting
		let show_args = LogsShowArgs {
			lines: 20,
			level: args.level.clone(),
			component: args.component.clone(),
			job_id: args.job_id.clone(),
			timestamps: args.timestamps,
			verbose: args.verbose,
		};

		if let Some(formatted) = format_log_line(&line, &show_args) {
			println!("{}", formatted);
		}
	}

	println!("───────────────");
	println!("Now streaming live logs...");

	Ok(())
}
