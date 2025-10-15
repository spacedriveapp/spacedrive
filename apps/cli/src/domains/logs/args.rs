//! Command line arguments for logs commands

use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum LogsCmd {
	/// Show recent logs
	Show(LogsShowArgs),
	/// Follow logs in real-time
	Follow(LogsFollowArgs),
}

#[derive(Args, Debug)]
pub struct LogsShowArgs {
	/// Number of lines to show
	#[arg(short = 'n', long, default_value = "100")]
	pub lines: usize,

	/// Filter by log level (error, warn, info, debug, trace)
	#[arg(long)]
	pub level: Option<String>,

	/// Filter by component/target
	#[arg(long)]
	pub component: Option<String>,

	/// Filter by job ID
	#[arg(long)]
	pub job_id: Option<String>,

	/// Show timestamps
	#[arg(short, long)]
	pub timestamps: bool,

	/// Verbose output (show full target)
	#[arg(short, long)]
	pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct LogsFollowArgs {
	/// Filter by log level (error, warn, info, debug, trace)
	#[arg(long)]
	pub level: Option<String>,

	/// Filter by component/target
	#[arg(long)]
	pub component: Option<String>,

	/// Filter by job ID
	#[arg(long)]
	pub job_id: Option<String>,

	/// Show timestamps
	#[arg(short, long)]
	pub timestamps: bool,

	/// Verbose output (show full target)
	#[arg(short, long)]
	pub verbose: bool,

	/// Show job IDs in output
	#[arg(long)]
	pub show_job_id: bool,

	/// Show library IDs in output
	#[arg(long)]
	pub show_library_id: bool,
}
