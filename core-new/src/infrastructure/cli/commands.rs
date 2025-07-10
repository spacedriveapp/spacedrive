//! CLI command definitions
//! 
//! This module only contains the command enum definitions used by clap.
//! All command handling is done through the daemon in mod.rs

use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;
use uuid::Uuid;

// Wrapper for clap ValueEnum since the original IndexMode doesn't have it
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexMode {
	/// Only metadata (fast)
	Shallow,
	/// Metadata + content hashing
	Content,
	/// Full analysis including media metadata
	Deep,
}

impl From<CliIndexMode> for crate::operations::indexing::IndexMode {
	fn from(mode: CliIndexMode) -> Self {
		match mode {
			CliIndexMode::Shallow => crate::operations::indexing::IndexMode::Shallow,
			CliIndexMode::Content => crate::operations::indexing::IndexMode::Content,
			CliIndexMode::Deep => crate::operations::indexing::IndexMode::Deep,
		}
	}
}

impl From<CliIndexMode> for crate::location::IndexMode {
	fn from(mode: CliIndexMode) -> Self {
		match mode {
			CliIndexMode::Shallow => crate::location::IndexMode::Shallow,
			CliIndexMode::Content => crate::location::IndexMode::Content,
			CliIndexMode::Deep => crate::location::IndexMode::Deep,
		}
	}
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

impl From<CliIndexScope> for crate::operations::indexing::IndexScope {
	fn from(scope: CliIndexScope) -> Self {
		match scope {
			CliIndexScope::Full => crate::operations::indexing::IndexScope::Recursive,
			CliIndexScope::Shallow => crate::operations::indexing::IndexScope::Current,
			CliIndexScope::Limited => crate::operations::indexing::IndexScope::Recursive, // No depth limit available
		}
	}
}

/// Enhanced indexing commands
#[derive(Subcommand, Clone, Debug)]
pub enum IndexCommands {
	/// Index a specific path
	Path {
		/// Path to index
		path: PathBuf,
		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,
		/// Indexing scope
		#[arg(short, long, value_enum, default_value = "full")]
		scope: CliIndexScope,
		/// Maximum depth (for limited scope)
		#[arg(short, long)]
		depth: Option<u32>,
		/// Create location if path doesn't exist in any
		#[arg(short, long)]
		create_location: bool,
		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
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
		/// Location ID or name
		location: String,
		/// Force re-indexing
		#[arg(short, long)]
		force: bool,
		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},
}

#[derive(Subcommand, Clone, Debug)]
pub enum LibraryCommands {
	/// Create a new library
	Create {
		/// Library name
		name: String,
		/// Path where to create the library
		#[arg(short, long)]
		path: Option<PathBuf>,
	},

	/// Switch to a different library
	Switch {
		/// Library ID or name
		identifier: String,
	},

	/// List all libraries
	List,

	/// Show current library
	Current,
}

#[derive(Subcommand, Clone, Debug)]
pub enum LocationCommands {
	/// Add a new location to the current library
	Add {
		/// Path to add as a location
		path: PathBuf,
		/// Custom name for the location
		#[arg(short, long)]
		name: Option<String>,
		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: CliIndexMode,
	},

	/// List all locations in the current library
	List,

	/// Get information about a specific location
	Info {
		/// Location ID or path
		identifier: String,
	},

	/// Remove a location from the library
	Remove {
		/// Location ID or path
		identifier: String,
	},

	/// Rescan a location for changes
	Rescan {
		/// Location ID or path
		identifier: String,
		/// Force full rescan
		#[arg(short, long)]
		force: bool,
	},
}

#[derive(Subcommand, Clone, Debug)]
pub enum JobCommands {
	/// List all jobs
	List {
		/// Filter by status
		#[arg(short, long)]
		status: Option<String>,
		/// Show only recent jobs
		#[arg(short, long)]
		recent: bool,
	},

	/// Get detailed information about a job
	Info {
		/// Job ID (can be partial)
		id: String,
	},

	/// Monitor job progress in real-time
	Monitor {
		/// Specific job ID to monitor
		job_id: Option<String>,
	},
}

#[derive(Subcommand, Clone, Debug)]
pub enum NetworkCommands {
	/// Initialize networking using master key
	Init,

	/// Start networking services
	Start,

	/// Stop networking services  
	Stop,

	/// List discovered devices
	Devices,

	/// Pairing operations
	Pair {
		#[command(subcommand)]
		action: PairingCommands,
	},

	/// Revoke a paired device
	Revoke {
		/// Device ID to revoke
		device_id: String,
	},

	/// Spacedrop operations
	Spacedrop {
		/// Device ID to send to
		device_id: String,
		/// File path to send
		file_path: PathBuf,
		/// Sender name
		#[arg(short, long)]
		sender: Option<String>,
		/// Optional message
		#[arg(short, long)]
		message: Option<String>,
	},
}

#[derive(Subcommand, Clone, Debug)]
pub enum PairingCommands {
	/// Generate a pairing code and wait for another device to connect (initiator)
	Generate,

	/// Join another device using their pairing code
	Join {
		/// The pairing code from the other device
		code: String,
	},

	/// Show pairing status
	Status,

	/// List pending pairing requests
	ListPending,

	/// Accept a pairing request
	Accept {
		/// Request ID
		request_id: String,
	},

	/// Reject a pairing request
	Reject {
		/// Request ID
		request_id: String,
	},
}