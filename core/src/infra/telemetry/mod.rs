//! # OpenTelemetry Integration
//!
//! Provides distributed tracing via OpenTelemetry, exporting spans to OTLP-compatible
//! collectors like Jaeger, Grafana Tempo, or any OpenTelemetry Collector instance.
//! This module bridges the `tracing` ecosystem with OpenTelemetry exporters.
//!
//! ## Usage
//!
//! Enable the `telemetry` feature and configure in `spacedrive.json`:
//!
//! ```json
//! {
//!   "telemetry": {
//!     "enabled": true,
//!     "endpoint": "http://localhost:4318",
//!     "service_name": "spacedrive-daemon"
//!   }
//! }
//! ```
//!
//! Then run a collector, e.g., Jaeger:
//! ```bash
//! docker run -p 16686:16686 -p 4318:4318 jaegertracing/all-in-one:latest
//! ```

use std::time::Duration;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::span_processor_with_async_runtime::BatchSpanProcessor;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use thiserror::Error;
use tracing::span::Attributes;
use tracing::{Id, Subscriber};
use tracing_subscriber::layer::{Context, Filter};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::config::app_config::TelemetryConfig;

/// Crate prefixes that identify Spacedrive code.
/// Traces are only exported if their root span originates from these crates.
const SPACEDRIVE_CRATE_PREFIXES: &[&str] = &["sd_core", "sd_daemon", "spacedrive"];

/// Key used to mark spans that belong to a Spacedrive-rooted trace.
const SPACEDRIVE_TRACE_MARKER: &str = "sd.trace";

/// Check if a target belongs to Spacedrive code.
fn is_spacedrive_target(target: &str) -> bool {
	SPACEDRIVE_CRATE_PREFIXES
		.iter()
		.any(|prefix| target.starts_with(prefix))
}

/// A filter that only allows spans belonging to traces rooted in Spacedrive code.
///
/// When a root span (no parent) is created from a Spacedrive crate, all its
/// descendants are included regardless of their origin. This allows database
/// queries, HTTP calls, and other dependency spans to appear in traces that
/// were initiated by Spacedrive code, while filtering out entire traces that
/// originate from dependencies like `acto`.
#[derive(Debug, Clone)]
pub struct SpacedriveTraceFilter;

impl<S> Filter<S> for SpacedriveTraceFilter
where
	S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
	fn enabled(&self, meta: &tracing::Metadata<'_>, cx: &Context<'_, S>) -> bool {
		// Check if this is a root span (no parent in the current context)
		let is_root = cx.lookup_current().is_none();

		if is_root {
			// Root spans are only enabled if they're from Spacedrive code
			is_spacedrive_target(meta.target())
		} else {
			// Non-root spans: check if any ancestor is marked as a Spacedrive trace
			if let Some(current) = cx.lookup_current() {
				// Walk up the span tree to find if we're in a Spacedrive-rooted trace
				let mut span_ref = Some(current);
				while let Some(span) = span_ref {
					let extensions = span.extensions();
					if extensions.get::<SpacedriveTraceMarker>().is_some() {
						return true;
					}
					span_ref = span.parent();
				}
			}
			false
		}
	}

	fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
		// If this is a root span from Spacedrive code, mark it
		let is_root = ctx.lookup_current().is_none();

		if is_root && is_spacedrive_target(attrs.metadata().target()) {
			if let Some(span) = ctx.span(id) {
				span.extensions_mut().insert(SpacedriveTraceMarker);
			}
		} else if !is_root {
			// Propagate the marker from parent to child
			if let Some(current) = ctx.lookup_current() {
				let has_marker = current
					.extensions()
					.get::<SpacedriveTraceMarker>()
					.is_some();
				if has_marker {
					if let Some(span) = ctx.span(id) {
						span.extensions_mut().insert(SpacedriveTraceMarker);
					}
				}
			}
		}
	}
}

/// Marker type stored in span extensions to indicate this span belongs to a Spacedrive trace.
#[derive(Debug, Clone, Copy)]
struct SpacedriveTraceMarker;

#[derive(Error, Debug)]
pub enum TelemetryError {
	#[error("Failed to build OTLP exporter: {0}")]
	ExporterBuild(#[from] opentelemetry_otlp::ExporterBuildError),

	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),
}

/// Handle to the OpenTelemetry tracer provider, required for graceful shutdown.
/// Drop this or call `shutdown()` to flush pending spans before exit.
pub struct TelemetryHandle {
	provider: SdkTracerProvider,
}

impl TelemetryHandle {
	/// Gracefully shutdown the tracer provider, flushing any pending spans.
	/// Call this before application exit to ensure all traces are exported.
	pub fn shutdown(self) -> opentelemetry_sdk::error::OTelSdkResult {
		self.provider.shutdown()
	}
}

/// Creates an OpenTelemetry tracing layer and returns a handle for shutdown.
///
/// The layer integrates with `tracing_subscriber::Registry` and exports spans
/// to the configured OTLP endpoint via HTTP/protobuf. The returned handle
/// must be kept alive and shut down on application exit to flush pending spans.
///
/// Only spans originating from Spacedrive crates (sd_core, sd_daemon, spacedrive)
/// are exported. Spans from third-party dependencies are filtered out to reduce
/// noise in traces.
pub fn create_otel_layer<S>(
	config: &TelemetryConfig,
) -> Result<(Box<dyn Layer<S> + Send + Sync + 'static>, TelemetryHandle), TelemetryError>
where
	S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
{
	if config.endpoint.is_empty() {
		return Err(TelemetryError::InvalidConfig(
			"endpoint cannot be empty".to_string(),
		));
	}

	// Build the OTLP HTTP exporter
	let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder()
		.with_http()
		.with_endpoint(&config.endpoint)
		.with_timeout(Duration::from_secs(config.timeout_secs));

	// Add custom headers if configured (e.g., for authentication)
	if !config.headers.is_empty() {
		let headers: Vec<(String, String)> = config
			.headers
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect();
		exporter_builder = exporter_builder.with_headers(headers.into_iter().collect());
	}

	let exporter = exporter_builder.build()?;

	// Build resource attributes identifying this service
	let resource = Resource::builder_empty()
		.with_attributes([
			KeyValue::new("service.name", config.service_name.clone()),
			KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
		])
		.build();

	// Create batch span processor with Tokio runtime for async export
	let batch_processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();

	// Create the tracer provider with the batch processor
	let provider = SdkTracerProvider::builder()
		.with_span_processor(batch_processor)
		.with_resource(resource)
		.build();

	// Get a tracer from the provider
	let tracer = provider.tracer("spacedrive");

	// Create the tracing layer with a filter that only exports traces rooted in Spacedrive code.
	// This allows dependency spans (like database queries) to appear in Spacedrive traces,
	// while filtering out entire traces that originate from dependencies (like acto::poll).
	let otel_layer = tracing_opentelemetry::layer()
		.with_tracer(tracer)
		.with_filter(SpacedriveTraceFilter)
		.boxed();

	let handle = TelemetryHandle { provider };

	Ok((otel_layer, handle))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config_creates_layer() {
		// Default config has enabled=false, but we can still create a layer
		let mut config = TelemetryConfig::default();
		config.enabled = true;

		// This should succeed (won't actually connect without a collector)
		let result = create_otel_layer::<tracing_subscriber::Registry>(&config);
		assert!(result.is_ok());
	}

	#[test]
	fn test_empty_endpoint_fails() {
		let config = TelemetryConfig {
			enabled: true,
			endpoint: "".to_string(),
			service_name: "test".to_string(),
			headers: Default::default(),
			timeout_secs: 10,
		};

		let result = create_otel_layer::<tracing_subscriber::Registry>(&config);
		assert!(matches!(result, Err(TelemetryError::InvalidConfig(_))));
	}
}
