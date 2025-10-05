//! Log event emitter for streaming logs to CLI clients

use super::{Event, EventBus};
use chrono::Utc;
use std::sync::{Arc, OnceLock};
use tracing::{field::Visit, Event as TracingEvent, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};
use uuid::Uuid;

/// Global holder for the daemon's EventBus used for log streaming
static LOG_EVENT_BUS: OnceLock<Arc<EventBus>> = OnceLock::new();

/// Set the global EventBus for log streaming. Safe to call once.
pub fn set_global_log_event_bus(event_bus: Arc<EventBus>) {
	let _ = LOG_EVENT_BUS.set(event_bus);
}

/// A tracing layer that emits log events to the event bus (if available)
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
		let Some(event_bus) = LOG_EVENT_BUS.get() else {
			return;
		};

		// Only emit events for INFO level and above to avoid spam
		if event.metadata().level() > &Level::INFO {
			return;
		}

		// Extract fields from the event
		let mut visitor = LogFieldVisitor::default();
		event.record(&mut visitor);

		// Try to extract job_id and library_id from span context
		let (job_id, library_id) = extract_context_ids(&ctx, event);

		// Create log event
		let log_event = Event::LogMessage {
			timestamp: Utc::now(),
			level: event.metadata().level().to_string(),
			target: event.metadata().target().to_string(),
			message: visitor.message,
			job_id,
			library_id,
		};

		// Emit to event bus (ignore errors to avoid logging loops)
		let _ = event_bus.emit(log_event);
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
