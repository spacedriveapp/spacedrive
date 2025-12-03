use clap::Args;

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
