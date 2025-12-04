use clap::Args;

#[derive(Args, Debug)]
pub struct SyncEventsArgs {
	/// Export format (json, sql, markdown)
	#[arg(
		short,
		long,
		default_value = "json",
		help = "Output format: json, sql, or markdown"
	)]
	pub format: String,

	/// Output file (defaults to stdout)
	#[arg(short, long, help = "Write output to file instead of stdout")]
	pub output: Option<String>,

	/// Time range: events since this time
	#[arg(
		long,
		help = "Show events since (e.g., '1 hour ago', '2025-12-03 10:00:00')"
	)]
	pub since: Option<String>,

	/// Filter by event type
	#[arg(
		long,
		help = "Filter by event type (state_transition, backfill_session_started, etc.)"
	)]
	pub event_type: Option<String>,

	/// Filter by correlation ID (show all events from a session)
	#[arg(
		long,
		help = "Filter by correlation ID to trace a specific backfill session"
	)]
	pub correlation_id: Option<String>,

	/// Filter by severity (debug, info, warning, error)
	#[arg(long, help = "Filter by severity level")]
	pub severity: Option<String>,

	/// Maximum number of events to return
	#[arg(long, default_value = "1000", help = "Maximum number of events")]
	pub limit: u32,

	/// Include device name in output
	#[arg(long, help = "Include device name/ID in output")]
	pub with_device: bool,

	/// Fetch events from all connected devices
	#[arg(
		long,
		help = "Fetch events from all connected devices (cross-device timeline)"
	)]
	pub all_devices: bool,
}

#[derive(Args, Debug)]
pub struct SyncMetricsArgs {
	/// Show metrics for a specific time range
	#[arg(
		long,
		help = "Show metrics since this time (e.g., '1 hour ago', '2025-10-23 10:00:00')"
	)]
	pub since: Option<String>,

	/// Show metrics for a specific peer device
	#[arg(long, help = "Filter metrics by peer device ID")]
	pub peer: Option<String>,

	/// Show metrics for a specific model type
	#[arg(long, help = "Filter metrics by model type (e.g., 'entry', 'tag')")]
	pub model: Option<String>,

	/// Watch metrics in real-time
	#[arg(short, long, help = "Watch metrics updates in real-time")]
	pub watch: bool,

	/// Output as JSON
	#[arg(long, help = "Output metrics as JSON")]
	pub json: bool,

	/// Show only state metrics
	#[arg(long, help = "Show only state transition metrics")]
	pub state: bool,

	/// Show only operation metrics
	#[arg(long, help = "Show only operation counter metrics")]
	pub operations: bool,

	/// Show only error metrics
	#[arg(long, help = "Show only error metrics")]
	pub errors: bool,
}
