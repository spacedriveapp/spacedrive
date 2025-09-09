//! File operations commands
//!
//! This module handles CLI commands for file operations:
//! - Copying files using the action system
//! - Indexing operations with enhanced scope options
//! - Legacy scanning operations

use crate::infra::cli::adapters::FileCopyCliArgs;
use crate::infra::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infra::cli::output::{CliOutput, Message};
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

// Re-export from the commands module for consistency
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexMode {
	/// Only metadata (fast)
	Shallow,
	/// Metadata + content hashing
	Content,
	/// Full analysis including media metadata
	Deep,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexScope {
	/// Full directory tree (default)
	Full,
	/// Only direct children
	Shallow,
	/// Custom depth
	Limited,
}

#[derive(Subcommand, Clone, Debug)]
pub enum FileCommands {
	/// Copy files using the action system
	Copy(FileCopyCliArgs),

	/// Enhanced indexing with scope and persistence options
	#[command(subcommand)]
	Index(IndexCommands),

	/// Start a traditional indexing job (legacy)
	Scan {
		/// Path to index
		path: PathBuf,

		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,

		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},
}

/// Enhanced indexing commands
#[derive(Subcommand, Clone, Debug)]
pub enum IndexCommands {
	/// Browse external paths without adding to managed locations
	Browse {
		/// Path to browse
		path: PathBuf,
		/// Scope: shallow or full
		#[arg(short, long, value_enum, default_value = "shallow")]
		scope: CliIndexScope,
		/// Enable content analysis
		#[arg(short, long)]
		content: bool,
	},

	/// Re-index all locations
	All {
		/// Force re-indexing even if up-to-date
		#[arg(short, long)]
		force: bool,
		/// Monitor jobs in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},

	/// Index a specific location
	Location {
		/// Location ID (UUID) or name (TODO: resolve name client-side to UUID)
		location: String,
		/// Force re-indexing
		#[arg(short, long)]
		force: bool,
		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},
}

pub async fn handle_file_command(
	cmd: FileCommands,
	instance_name: Option<String>,
	mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut client = DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		FileCommands::Copy(args) => handle_copy_command(args, &mut client, &mut output).await,
		FileCommands::Index(cmd) => handle_index_command(cmd, &mut client, &mut output).await,
		FileCommands::Scan { path, mode, watch } => {
			handle_scan_command(path, mode, watch, &mut client, &mut output).await
		}
	}
}

async fn handle_copy_command(
	args: FileCopyCliArgs,
	client: &mut DaemonClient,
	output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	// Convert CLI args to daemon command format
	let input = match args.validate_and_convert() {
		Ok(input) => input,
		Err(e) => {
			output.error(Message::Error(format!("Invalid copy operation: {}", e)))?;
			return Ok(());
		}
	};

	output.info(&input.summary())?;

	// Send copy command to daemon (typed input)
	match client.send_command(DaemonCommand::Copy(input)).await {
		Ok(DaemonResponse::Ok) => {
			output.success("Copy operation started successfully!")?;
		}
		Ok(DaemonResponse::Error(e)) => {
			output.error(Message::Error(format!("Failed to copy files: {}", e)))?;
		}
		Err(e) => {
			output.error(Message::Error(format!(
				"Failed to communicate with daemon: {}",
				e
			)))?;
		}
		_ => {
			output.error(Message::Error(
				"Unexpected response from daemon".to_string(),
			))?;
		}
	}

	Ok(())
}

async fn handle_index_command(
	cmd: IndexCommands,
	client: &mut DaemonClient,
	output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	match cmd {
		IndexCommands::Browse {
			path,
			scope,
			content,
		} => {
			output.info(&format!("Browsing {}...", path.display()))?;
			if content { output.info("Content analysis enabled")?; }

			let scope_core = match scope {
				CliIndexScope::Full => crate::ops::indexing::IndexScope::Recursive,
				CliIndexScope::Shallow => crate::ops::indexing::IndexScope::Current,
				CliIndexScope::Limited => crate::ops::indexing::IndexScope::Current,
			};

			let mut input = crate::ops::indexing::IndexInput::single(
				uuid::Uuid::nil(), // library_id is validated/overwritten in daemon state
				path.clone(),
			)
			.with_scope(scope_core)
			.with_mode(if content { crate::ops::indexing::IndexMode::Content } else { crate::ops::indexing::IndexMode::Shallow })
			.with_persistence(crate::ops::indexing::IndexPersistence::Ephemeral);

			match client.send_command(DaemonCommand::Index(input)).await {
				Ok(DaemonResponse::Ok) => {
					output.success("Indexing started successfully")?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Indexing failed: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
				}
				_ => {
					output.error(Message::Error("Unexpected response from daemon".to_string()))?;
				}
			}
		}
		IndexCommands::All { force, watch } => {
			output.info("Re-indexing all locations...")?;

			match client.send_command(DaemonCommand::IndexAll { force }).await {
				Ok(DaemonResponse::Ok) => {
					output.success("Re-indexing of all locations started successfully")?;
					if watch {
						output.info("Use 'spacedrive job monitor' to track progress")?;
					}
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Re-indexing failed: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}
		IndexCommands::Location {
			location,
			force,
			watch,
		} => {
			output.info(&format!("Indexing location {}...", location))?;

			// Build typed action (UUID parsing stays here for now)
			let action = match uuid::Uuid::parse_str(&location) {
				Ok(location_id) => crate::ops::locations::rescan::action::LocationRescanAction {
					library_id: uuid::Uuid::nil(), // daemon injects current library
					location_id,
					full_rescan: force,
				},
				Err(_) => {
					output.error(Message::Error("Invalid location ID; only UUID supported for now".to_string()))?;
					return Ok(());
				}
			};

			match client.send_command(DaemonCommand::LocationRescan(action)).await {
				Ok(DaemonResponse::Ok) => {
					output.success("Location indexing started successfully")?;
					if watch { output.info("Use 'spacedrive job monitor' to track progress")?; }
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Location indexing failed: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
				}
				_ => {
					output.error(Message::Error("Unexpected response from daemon".to_string()))?;
				}
			}
		}
	}
	Ok(())
}

async fn handle_scan_command(
	path: PathBuf,
	mode: CliIndexMode,
	watch: bool,
	client: &mut DaemonClient,
	output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	output.info(&format!("Scanning {}...", path.display()))?;

	let mode_str = match mode {
		CliIndexMode::Shallow => "shallow",
		CliIndexMode::Content => "content",
		CliIndexMode::Deep => "deep",
	};

	// Scan command is no longer supported - use add location and index instead
	output.error(Message::Error(
        "The 'scan' command has been removed. Please use 'location add' followed by 'file index location' instead.".to_string()
    ))?;

	Ok(())
}
