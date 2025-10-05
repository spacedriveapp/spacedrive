//! Volume speed test operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from volume speed test operation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeSpeedTestOutput {
	/// The fingerprint of the tested volume
	pub fingerprint: VolumeFingerprint,

	/// Read speed in MB/s (if measured)
	pub read_speed_mbps: Option<u32>,

	/// Write speed in MB/s (if measured)
	pub write_speed_mbps: Option<u32>,
}

impl VolumeSpeedTestOutput {
	/// Create new volume speed test output
	pub fn new(
		fingerprint: VolumeFingerprint,
		read_speed_mbps: Option<u32>,
		write_speed_mbps: Option<u32>,
	) -> Self {
		Self {
			fingerprint,
			read_speed_mbps,
			write_speed_mbps,
		}
	}
}
