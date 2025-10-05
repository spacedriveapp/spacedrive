//! Job-specific logging implementation
//!
//! This module provides a custom tracing subscriber that captures logs
//! for individual jobs and writes them to separate log files.

use super::types::JobId;
use crate::config::JobLoggingConfig;
use std::{
	fs::{File, OpenOptions},
	io::{Seek, Write},
	path::PathBuf,
	sync::{Arc, Mutex},
};
use tracing::{
	field::{Field, Visit},
	span::{Attributes, Record},
	Event, Id, Level, Metadata, Subscriber,
};
use tracing_subscriber::{
	fmt::{
		format::{self, FormatEvent, FormatFields},
		FmtContext, FormattedFields,
	},
	registry::LookupSpan,
	Layer,
};

/// A tracing layer that writes logs to a job-specific file
pub struct JobLogLayer {
	job_id: JobId,
	file: Arc<Mutex<File>>,
	config: JobLoggingConfig,
	max_file_size: u64,
	current_size: Arc<Mutex<u64>>,
}

impl JobLogLayer {
	/// Create a new job log layer
	pub fn new(
		job_id: JobId,
		log_path: PathBuf,
		config: JobLoggingConfig,
	) -> std::io::Result<Self> {
		// Create or append to the log file
		let file = OpenOptions::new()
			.create(true)
			.append(true)
			.open(&log_path)?;

		// Get current file size
		let current_size = file.metadata()?.len();

		Ok(Self {
			job_id,
			file: Arc::new(Mutex::new(file)),
			max_file_size: config.max_file_size,
			current_size: Arc::new(Mutex::new(current_size)),
			config,
		})
	}

	/// Check if this event should be logged based on job context
	fn should_log(&self, metadata: &Metadata<'_>) -> bool {
		// Filter by log level
		if !self.config.include_debug && metadata.level() > &Level::INFO {
			return false;
		}

		// Always log ERROR and WARN
		if metadata.level() <= &Level::WARN {
			return true;
		}

		// For other levels, only log if it's from job-related modules
		let target = metadata.target();
		target.contains("job")
			|| target.contains("executor")
			|| target.contains("infrastructure::jobs")
			|| target.contains("operations")
	}

	/// Write a log entry to the file
	fn write_log(&self, message: String) -> std::io::Result<()> {
		let mut file = self.file.lock().unwrap();
		let mut size = self.current_size.lock().unwrap();

		// Check file size limit
		if self.max_file_size > 0 && *size + message.len() as u64 > self.max_file_size {
			// File too large, truncate and start fresh
			file.set_len(0)?;
			file.seek(std::io::SeekFrom::Start(0))?;
			*size = 0;

			// Write truncation notice
			let notice = format!(
				"[{}] Log file truncated due to size limit\n",
				chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
			);
			file.write_all(notice.as_bytes())?;
			*size += notice.len() as u64;
		}

		// Write the log message
		file.write_all(message.as_bytes())?;
		file.flush()?;
		*size += message.len() as u64;

		Ok(())
	}
}

impl<S> Layer<S> for JobLogLayer
where
	S: Subscriber + for<'a> LookupSpan<'a>,
{
	fn on_event(&self, event: &Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
		// Check if we should log this event
		if !self.should_log(event.metadata()) {
			return;
		}

		// Check if this event is from our job's span
		let current_span = ctx.event_span(event);
		if let Some(span) = current_span {
			// Look for job_id field in the span or its parents
			let mut found_job = false;
			let mut current = Some(span);

			while let Some(span) = current {
				if let Some(fields) = span
					.extensions()
					.get::<FormattedFields<format::DefaultFields>>()
				{
					if fields.fields.contains(&format!("job_id={}", self.job_id)) {
						found_job = true;
						break;
					}
				}
				current = span.parent();
			}

			// If this isn't from our job, skip it
			if !found_job {
				return;
			}
		}

		// Format the log message
		let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
		let level = event.metadata().level();
		let target = event.metadata().target();

		// Extract the message from the event
		let mut visitor = MessageVisitor::default();
		event.record(&mut visitor);
		let message = visitor.message;

		// Format: [timestamp] LEVEL target: message
		let formatted = format!("[{}] {:5} {}: {}\n", timestamp, level, target, message);

		// Write to file
		if let Err(e) = self.write_log(formatted) {
			eprintln!("Failed to write to job log: {}", e);
		}
	}

	fn on_new_span(
		&self,
		_attrs: &Attributes<'_>,
		_id: &Id,
		_ctx: tracing_subscriber::layer::Context<'_, S>,
	) {
		// We don't need to do anything special for new spans
	}
}

/// Helper to extract message from event fields
#[derive(Default)]
struct MessageVisitor {
	message: String,
}

impl Visit for MessageVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			self.message = format!("{:?}", value);
		} else {
			if !self.message.is_empty() {
				self.message.push_str(", ");
			}
			self.message
				.push_str(&format!("{}={:?}", field.name(), value));
		}
	}

	fn record_str(&mut self, field: &Field, value: &str) {
		if field.name() == "message" {
			self.message = value.to_string();
		} else {
			if !self.message.is_empty() {
				self.message.push_str(", ");
			}
			self.message
				.push_str(&format!("{}=\"{}\"", field.name(), value));
		}
	}
}

/// A simple file-based job logger
pub struct FileJobLogger {
	job_id: JobId,
	file: Arc<Mutex<File>>,
	config: JobLoggingConfig,
}

impl FileJobLogger {
	pub fn new(
		job_id: JobId,
		log_path: PathBuf,
		config: JobLoggingConfig,
	) -> std::io::Result<Self> {
		let file = OpenOptions::new()
			.create(true)
			.append(true)
			.open(&log_path)?;

		Ok(Self {
			job_id,
			file: Arc::new(Mutex::new(file)),
			config,
		})
	}

	pub fn log(&self, level: &str, message: &str) -> std::io::Result<()> {
		if level == "DEBUG" && !self.config.include_debug {
			return Ok(());
		}

		let mut file = self.file.lock().unwrap();
		writeln!(
			file,
			"[{}] {} {}: {}",
			chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
			level,
			self.job_id,
			message
		)?;
		file.flush()
	}
}

/// Create a job-specific logger that writes to a file
pub fn create_job_logger(
	job_id: JobId,
	log_dir: PathBuf,
	config: JobLoggingConfig,
) -> std::io::Result<JobLogLayer> {
	// Create log file path
	let log_file = log_dir.join(format!("{}.log", job_id));

	// Write initial log entry
	let mut file = OpenOptions::new()
		.create(true)
		.append(true)
		.open(&log_file)?;

	writeln!(
		file,
		"[{}] === Job {} started ===",
		chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
		job_id
	)?;
	file.flush()?;
	drop(file);

	// Create the job log layer
	JobLogLayer::new(job_id, log_file, config)
}

/// Setup job logging for async execution
pub fn setup_job_logging(
	job_id: JobId,
	log_dir: PathBuf,
	config: JobLoggingConfig,
) -> std::io::Result<Option<impl Drop>> {
	// Create log file path
	let log_file = log_dir.join(format!("{}.log", job_id));

	// Write initial log entry directly
	let mut file = OpenOptions::new()
		.create(true)
		.append(true)
		.open(&log_file)?;

	writeln!(
		file,
		"[{}] === Job {} started ===",
		chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
		job_id
	)?;
	file.flush()?;
	drop(file);

	// For now, we'll write logs directly in the job context
	// This avoids conflicts with existing tracing subscribers

	// Return a guard that will write the final log entry
	struct JobLoggingGuard {
		log_file: PathBuf,
		job_id: JobId,
	}

	impl Drop for JobLoggingGuard {
		fn drop(&mut self) {
			// Write final log entry
			if let Ok(mut file) = OpenOptions::new().append(true).open(&self.log_file) {
				let _ = writeln!(
					file,
					"[{}] === Job {} finished ===",
					chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
					self.job_id
				);
				let _ = file.flush();
			}
		}
	}

	Ok(Some(JobLoggingGuard { log_file, job_id }))
}
