//! Debug volume detection
//!
//! Run with: cargo run --example debug_volumes

use sd_core::volume::{
	detection::detect_volumes,
	types::{VolumeDetectionConfig, VolumeType},
};
use uuid::Uuid;

#[tokio::main]
async fn main() {
	// Enable debug logging
	tracing_subscriber::fmt()
		.with_max_level(tracing::Level::DEBUG)
		.with_target(true)
		.init();

	println!("\n=== Volume Detection Debug ===\n");

	let device_id = Uuid::new_v4();
	let config = VolumeDetectionConfig {
		include_system: true,
		include_virtual: false,
		run_speed_test: false,
		refresh_interval_secs: 0,
	};

	match detect_volumes(device_id, &config).await {
		Ok(volumes) => {
			println!("Detected {} volumes:\n", volumes.len());

			for vol in &volumes {
				println!("Volume: {}", vol.name);
				println!("  Display name: {}", vol.display_name.as_ref().unwrap_or(&"None".to_string()));
				println!("  Mount point: {}", vol.mount_point.display());
				println!("  Type: {:?}", vol.volume_type);
				println!("  Filesystem: {}", vol.file_system);
				println!("  Fingerprint: {} ({})", vol.fingerprint.short_id(), vol.fingerprint.0);
				println!("  Is user visible: {}", vol.is_user_visible);
				println!("  Auto-track eligible: {}", vol.auto_track_eligible);
				println!("  Is tracked: {}", vol.is_tracked);
				println!();
			}

			// Show specifically which volumes are auto-track eligible
			let auto_track: Vec<_> = volumes
				.iter()
				.filter(|v| v.auto_track_eligible)
				.collect();

			println!("=== Auto-Track Eligible Volumes ({}) ===", auto_track.len());
			for vol in auto_track {
				println!("  - {} ({})", vol.display_name.as_ref().unwrap_or(&vol.name), vol.mount_point.display());
			}

			// Show Primary volumes specifically
			let primary: Vec<_> = volumes
				.iter()
				.filter(|v| matches!(v.volume_type, VolumeType::Primary))
				.collect();

			println!("\n=== Primary Volumes ({}) ===", primary.len());
			for vol in primary {
				println!("  - {} at {}", vol.name, vol.mount_point.display());
				println!("    Auto-track eligible: {}", vol.auto_track_eligible);
				println!("    Is user visible: {}", vol.is_user_visible);
			}
		}
		Err(e) => {
			eprintln!("Error detecting volumes: {}", e);
		}
	}
}
