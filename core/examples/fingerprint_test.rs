//! Test volume fingerprint stability
//!
//! Run with: cargo run --example fingerprint_test

use sd_core::domain::volume::VolumeFingerprint;
use std::path::PathBuf;
use uuid::Uuid;

fn main() {
	println!("\n=== Volume Fingerprint Stability Tests ===\n");

	let device_id = Uuid::new_v4();

	// Test 1: Primary volume stability
	println!("Test 1: Primary volume - Same inputs → Same fingerprint");
	let mount_point = PathBuf::from("/System/Volumes/Data");

	let fp1 = VolumeFingerprint::from_primary_volume(&mount_point, device_id);
	let fp2 = VolumeFingerprint::from_primary_volume(&mount_point, device_id);

	println!("  First run:  {}", fp1.short_id());
	println!("  Second run: {}", fp2.short_id());
	println!("  Match: {}\n", fp1 == fp2);

	// Test 2: External volume with dotfile UUID
	println!("Test 2: External volume - Dotfile UUID provides stability");
	let spacedrive_id = Uuid::new_v4();
	let fp_ext1 = VolumeFingerprint::from_external_volume(spacedrive_id, device_id);
	let fp_ext2 = VolumeFingerprint::from_external_volume(spacedrive_id, device_id);

	println!("  With same dotfile UUID: {} == {}", fp_ext1.short_id(), fp_ext2.short_id());
	println!("  Match: {}\n", fp_ext1 == fp_ext2);

	// Test 3: Network volume stability
	println!("Test 3: Network volume - Backend ID provides stability");
	let backend_id = "s3";
	let bucket_name = "my-bucket";

	let fp_net1 = VolumeFingerprint::from_network_volume(backend_id, bucket_name);
	let fp_net2 = VolumeFingerprint::from_network_volume(backend_id, bucket_name);

	println!("  First run:  {}", fp_net1.short_id());
	println!("  Second run: {}", fp_net2.short_id());
	println!("  Match: {}\n", fp_net1 == fp_net2);

	// Test 4: Primary volume - Mount point changes break fingerprint
	println!("Test 4: Primary volume - Different mount points = Different fingerprints");
	let mount1 = PathBuf::from("/Volumes/MyDrive");
	let mount2 = PathBuf::from("/Volumes/MyDrive1"); // Remounted at different path

	let fp_mount1 = VolumeFingerprint::from_primary_volume(&mount1, device_id);
	let fp_mount2 = VolumeFingerprint::from_primary_volume(&mount2, device_id);

	println!("  Mount at /Volumes/MyDrive:  {}", fp_mount1.short_id());
	println!("  Mount at /Volumes/MyDrive1: {}", fp_mount2.short_id());
	println!("  Different: {} (expected for primary volumes)\n", fp_mount1 != fp_mount2);

	// Test 5: External volume - Same dotfile UUID, different mount points
	println!("Test 5: External volume - Dotfile UUID stable across remounts");
	let ext_uuid = Uuid::new_v4();
	let fp_at_mount1 = VolumeFingerprint::from_external_volume(ext_uuid, device_id);
	let fp_at_mount2 = VolumeFingerprint::from_external_volume(ext_uuid, device_id);

	println!("  Mounted at /Volumes/USB:  {}", fp_at_mount1.short_id());
	println!("  Mounted at /Volumes/USB1: {}", fp_at_mount2.short_id());
	println!("  Match: {} (dotfile UUID is stable!)\n", fp_at_mount1 == fp_at_mount2);

	// Summary
	println!("=== Summary ===");
	println!("Primary volumes: Use mount_point + device_id");
	println!("  - Stable for system volumes with fixed mount points");
	println!("  - Examples: /System/Volumes/Data, C:\\, /");
	println!();
	println!("External volumes: Use dotfile UUID + device_id");
	println!("  - Stable across remounts to different paths");
	println!("  - Fallback to mount_point + device_id if read-only");
	println!();
	println!("Network volumes: Use backend_id + mount_uri");
	println!("  - Stable based on cloud service and identifier");
	println!("  - Examples: S3 bucket ARN, WebDAV URL");
}
