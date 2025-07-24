//! Command types that can be sent to the daemon

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;
use crate::infrastructure::cli::commands::{
    LibraryCommands, LocationCommands, JobCommands, FileCommands, 
    NetworkCommands, SystemCommands, VolumeCommands
};

/// Commands that can be sent to the daemon
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonCommand {
	// Core management
	Ping,
	Shutdown,
	GetStatus,

	// Library commands
	CreateLibrary {
		name: String,
		path: Option<PathBuf>,
	},
	ListLibraries,
	SwitchLibrary {
		id: Uuid,
	},
	GetCurrentLibrary,

	// Location commands
	AddLocation {
		path: PathBuf,
		name: Option<String>,
	},
	ListLocations,
	RescanLocation {
		id: Uuid,
	},
	RemoveLocation {
		id: Uuid,
	},

	// Job commands
	ListJobs {
		status: Option<String>,
	},
	GetJobInfo {
		id: Uuid,
	},
	PauseJob {
		id: Uuid,
	},
	ResumeJob {
		id: Uuid,
	},
	CancelJob {
		id: Uuid,
	},

	// File operations
	Copy {
		sources: Vec<PathBuf>,
		destination: PathBuf,
		overwrite: bool,
		verify: bool,
		preserve_timestamps: bool,
		move_files: bool,
	},

	// Indexing operations
	Browse {
		path: PathBuf,
		scope: String,
		content: bool,
	},
	IndexAll {
		force: bool,
	},
	IndexLocation {
		location: String,
		force: bool,
	},

	// Subscribe to events
	SubscribeEvents,

	// Networking commands
	InitNetworking,
	StartNetworking,
	StopNetworking,
	ListConnectedDevices,
	RevokeDevice {
		device_id: Uuid,
	},
	SendSpacedrop {
		device_id: Uuid,
		file_path: String,
		sender_name: String,
		message: Option<String>,
	},

	// Pairing commands
	StartPairingAsInitiator,
	StartPairingAsJoiner {
		code: String,
	},
	GetPairingStatus,
	ListPendingPairings,
	AcceptPairing {
		request_id: Uuid,
	},
	RejectPairing {
		request_id: Uuid,
	},

	// Volume commands
	Volume(VolumeCommands),
}