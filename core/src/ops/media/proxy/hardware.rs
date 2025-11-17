//! Hardware acceleration detection for video encoding

use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{debug, warn};

/// Supported hardware acceleration platforms
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum HardwareAccel {
	/// Apple VideoToolbox (macOS/iOS)
	VideoToolbox,
	/// NVIDIA NVENC
	NVENC,
	/// Intel QuickSync
	QuickSync,
	/// AMD AMF
	AMF,
	/// VA-API (Linux)
	VAAPI,
}

impl HardwareAccel {
	/// Get the FFmpeg encoder name for this acceleration platform
	pub fn encoder_name(&self) -> &'static str {
		match self {
			Self::VideoToolbox => "h264_videotoolbox",
			Self::NVENC => "h264_nvenc",
			Self::QuickSync => "h264_qsv",
			Self::AMF => "h264_amf",
			Self::VAAPI => "h264_vaapi",
		}
	}

	/// Get recommended preset for this encoder
	pub fn preset(&self) -> Option<&'static str> {
		match self {
			Self::VideoToolbox => None, // VideoToolbox doesn't use presets
			Self::NVENC | Self::QuickSync | Self::AMF => Some("fast"),
			Self::VAAPI => None,
		}
	}

	/// Additional arguments for this encoder
	pub fn extra_args(&self) -> Vec<&'static str> {
		match self {
			Self::VideoToolbox => vec![],
			Self::NVENC => vec!["-rc", "vbr"],
			Self::QuickSync => vec!["-look_ahead", "0"],
			Self::AMF => vec![],
			Self::VAAPI => vec![],
		}
	}
}

/// Detect available hardware acceleration
pub fn detect_hardware_accel() -> Option<HardwareAccel> {
	// Try to run ffmpeg -encoders and check which hardware encoders are available
	let output = match Command::new("ffmpeg")
		.args(["-hide_banner", "-encoders"])
		.output()
	{
		Ok(out) => out,
		Err(e) => {
			warn!("Failed to run ffmpeg to detect hardware encoders: {}", e);
			return None;
		}
	};

	if !output.status.success() {
		warn!("ffmpeg -encoders command failed");
		return None;
	}

	let encoders = String::from_utf8_lossy(&output.stdout);

	// Platform-specific detection order (prefer native first)
	#[cfg(target_os = "macos")]
	{
		if encoders.contains("h264_videotoolbox") {
			debug!("Detected VideoToolbox hardware acceleration");
			return Some(HardwareAccel::VideoToolbox);
		}
	}

	// NVENC (NVIDIA)
	if encoders.contains("h264_nvenc") {
		debug!("Detected NVENC hardware acceleration");
		return Some(HardwareAccel::NVENC);
	}

	// QuickSync (Intel)
	if encoders.contains("h264_qsv") {
		debug!("Detected QuickSync hardware acceleration");
		return Some(HardwareAccel::QuickSync);
	}

	// AMD
	if encoders.contains("h264_amf") {
		debug!("Detected AMF hardware acceleration");
		return Some(HardwareAccel::AMF);
	}

	// VA-API (Linux)
	#[cfg(target_os = "linux")]
	{
		if encoders.contains("h264_vaapi") {
			debug!("Detected VA-API hardware acceleration");
			return Some(HardwareAccel::VAAPI);
		}
	}

	debug!("No hardware acceleration detected, will use software encoding");
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encoder_names() {
		assert_eq!(
			HardwareAccel::VideoToolbox.encoder_name(),
			"h264_videotoolbox"
		);
		assert_eq!(HardwareAccel::NVENC.encoder_name(), "h264_nvenc");
		assert_eq!(HardwareAccel::QuickSync.encoder_name(), "h264_qsv");
	}

	#[test]
	fn test_detection() {
		// This will actually detect hardware on the test system
		let hw = detect_hardware_accel();
		println!("Detected hardware: {:?}", hw);
	}
}
