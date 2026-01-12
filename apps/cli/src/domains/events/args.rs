use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub enum EventsCmd {
	/// Monitor events in real-time
	Monitor(EventsMonitorArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
	/// Human-readable summary (default)
	Human,
	/// Compact JSON output
	Json,
	/// Pretty-printed JSON output
	JsonPretty,
}

#[derive(Debug, Args)]
pub struct EventsMonitorArgs {
	/// Filter by event type(s) (comma-separated, e.g., "JobProgress,JobCompleted")
	/// Leave empty to see all events
	#[arg(short = 't', long)]
	pub event_type: Option<String>,

	/// Filter by library ID
	#[arg(short = 'l', long)]
	pub library_id: Option<uuid::Uuid>,

	/// Filter by job ID
	#[arg(short = 'j', long)]
	pub job_id: Option<String>,

	/// Filter by device ID
	#[arg(short = 'd', long)]
	pub device_id: Option<uuid::Uuid>,

	/// Show timestamps
	#[arg(long)]
	pub timestamps: bool,

	/// Output format
	#[arg(short = 'f', long, value_enum, default_value = "human")]
	pub format: OutputFormat,

	/// Show full event JSON (verbose mode)
	#[arg(short = 'v', long)]
	pub verbose: bool,

	/// Pretty print JSON output
	#[arg(short = 'p', long)]
	pub pretty: bool,
}
