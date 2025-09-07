//! Response types returned by the daemon

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::common::{
	BrowseEntry, ConnectedDeviceInfo, JobInfo, LibraryInfo, LocationInfo, PairingRequestInfo,
	VolumeListItem,
};
use crate::{infrastructure::actions::output::ActionOutput, volume::Volume};

/// Responses from the daemon
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
	Ok,
	Error(String),
	Pong,
	Status(DaemonStatus),
	LibraryCreated {
		id: Uuid,
		name: String,
		path: PathBuf,
	},
	Libraries(Vec<LibraryInfo>),
	CurrentLibrary(Option<LibraryInfo>),
	LocationAdded {
		location_id: Uuid,
		job_id: String,
	},
	LocationIndexed {
		location_id: Uuid,
	},
	Locations(Vec<LocationInfo>),
	BrowseResults {
		path: PathBuf,
		entries: Vec<BrowseEntry>,
		total_files: usize,
		total_dirs: usize,
	},
	Jobs(Vec<JobInfo>),
	JobInfo(Option<JobInfo>),
	CopyStarted {
		job_id: Uuid,
		sources_count: usize,
	},
	Event(String), // Serialized event

	// Networking responses
	ConnectedDevices(Vec<ConnectedDeviceInfo>),
	SpacedropStarted {
		transfer_id: Uuid,
	},

	// Pairing responses
	PairingCodeGenerated {
		code: String,
		expires_in_seconds: u32,
	},
	PairingInProgress,
	PairingStatus {
		status: String,
		remote_device: Option<ConnectedDeviceInfo>,
	},
	PendingPairings(Vec<PairingRequestInfo>),

	// Volume responses
	VolumeList(Vec<Volume>),
	VolumeListWithTracking(Vec<VolumeListItem>),
	Volume(Volume),

	// Action output (generic for all action results)
	ActionOutput(ActionOutput),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonStatus {
	pub version: String,
	pub uptime_secs: u64,
	pub current_library: Option<Uuid>,
	pub active_jobs: usize,
	pub total_locations: usize,
}
