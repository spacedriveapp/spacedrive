//! Dedicated log streaming bus for CLI clients
//!
//! This is separate from the main event bus to avoid polluting it with high-volume log events.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast;
use tracing::{field::Visit, Event as TracingEvent, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};
use uuid::Uuid;

/// A log message event (separate from main Event enum)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LogMessage {
	pub timestamp: chrono::DateTime<chrono::Utc>,
	pub level: String,
	pub target: String,
	pub message: String,
	pub job_id: Option<String>,
	pub library_id: Option<Uuid>,
}

/// Dedicated broadcast channel for log streaming
#[derive(Debug, Clone)]
pub struct LogBus {
	sender: broadcast::Sender<LogMessage>,
}

impl LogBus {
	/// Create a new log bus with specified capacity
	pub fn new(capacity: usize) -> Self {
		let (sender, _) = broadcast::channel(capacity);
		Self { sender }
	}

	/// Emit a log message to all subscribers
	pub fn emit(&self, log: LogMessage) {
		// Ignore errors - no subscribers is fine
		let _ = self.sender.send(log);
	}

	/// Subscribe to log messages
	pub fn subscribe(&self) -> broadcast::Receiver<LogMessage> {
		self.sender.subscribe()
	}

	/// Get the number of active subscribers
	pub fn subscriber_count(&self) -> usize {
		self.sender.receiver_count()
	}
}

impl Default for LogBus {
	fn default() -> Self {
		Self::new(1024)
	}
}

/// Global holder for the daemon's LogBus
static LOG_BUS: OnceLock<Arc<LogBus>> = OnceLock::new();

/// Set the global LogBus for log streaming. Safe to call once.
pub fn set_global_log_bus(log_bus: Arc<LogBus>) {
	let _ = LOG_BUS.set(log_bus);
}

/// A tracing layer that emits log messages to the log bus (if available)
pub struct LogEventLayer;

impl LogEventLayer {
	pub fn new() -> Self {
		Self
	}
}

impl<S> Layer<S> for LogEventLayer
where
	S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
	fn on_event(&self, event: &TracingEvent<'_>, ctx: Context<'_, S>) {
		// If no global bus set yet, skip
		let Some(log_bus) = LOG_BUS.get() else {
			return;
		};

		// Only emit events for INFO level and above to avoid spam
		if event.metadata().level() > &Level::INFO {
			return;
		}

		// Only emit if someone is listening (avoid overhead)
		if log_bus.subscriber_count() == 0 {
			return;
		}

		// Extract fields from the event
		let mut visitor = LogFieldVisitor::default();
		event.record(&mut visitor);

		// Try to extract job_id and library_id from span context
		let (job_id, library_id) = extract_context_ids(&ctx, event);

		// Create log message
		let log_message = LogMessage {
			timestamp: Utc::now(),
			level: event.metadata().level().to_string(),
			target: event.metadata().target().to_string(),
			message: visitor.message,
			job_id,
			library_id,
		};

		// Emit to log bus (ignore errors to avoid logging loops)
		log_bus.emit(log_message);
	}
}

/// Visitor to extract message from tracing event
#[derive(Default)]
struct LogFieldVisitor {
	message: String,
}

impl Visit for LogFieldVisitor {
	fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			self.message = format!("{:?}", value);
			// Remove quotes from debug formatting
			if self.message.starts_with('"') && self.message.ends_with('"') {
				self.message = self.message[1..self.message.len() - 1].to_string();
			}
		}
	}

	fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
		if field.name() == "message" {
			self.message = value.to_string();
		}
	}
}

/// Extract job_id and library_id from tracing span context
fn extract_context_ids<S>(
	ctx: &Context<'_, S>,
	event: &TracingEvent<'_>,
) -> (Option<String>, Option<Uuid>)
where
	S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
	let mut job_id = None;
	let mut library_id = None;

	// Check current span and parent spans for context
	if let Some(span) = ctx.event_span(event) {
		let mut current = Some(span);

		while let Some(span) = current {
			// Try to extract job_id and library_id from span fields
			if let Some(extensions) = span
				.extensions()
				.get::<tracing_subscriber::fmt::FormattedFields<
					tracing_subscriber::fmt::format::DefaultFields,
				>>() {
				let fields_str = &extensions.fields;

				// Simple field extraction (this could be more robust)
				if let Some(start) = fields_str.find("job_id=") {
					if let Some(end) = fields_str[start + 7..].find(' ') {
						job_id = Some(fields_str[start + 7..start + 7 + end].to_string());
					} else {
						job_id = Some(fields_str[start + 7..].to_string());
					}
				}

				if let Some(start) = fields_str.find("library_id=") {
					if let Some(end) = fields_str[start + 11..].find(' ') {
						if let Ok(uuid) = fields_str[start + 11..start + 11 + end].parse::<Uuid>() {
							library_id = Some(uuid);
						}
					} else if let Ok(uuid) = fields_str[start + 11..].parse::<Uuid>() {
						library_id = Some(uuid);
					}
				}
			}

			current = span.parent();
		}
	}

	(job_id, library_id)
}
