//! Volume system demonstration

use sd_core::{
	infra::event::EventFilter,
	volume::types::{FileSystem, MountType},
	Core,
};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter("sd_core_new=info")
		.init();

	println!("=== Spacedrive Volume System Demo ===\n");

	// Initialize core (which includes volume manager)
	println!("1. Initializing Spacedrive Core with volume detection...");
	let core = Core::new().await?;
	println!("   ✓ Core initialized");

	// Get volume statistics
	println!("\n2. Volume Statistics:");
	let stats = core.volumes.get_statistics().await;
	println!("   • Total volumes: {}", stats.total_volumes);
	println!("   • Mounted volumes: {}", stats.mounted_volumes);
	println!(
		"   • Total capacity: {:.2} GB",
		stats.total_capacity as f64 / 1024.0 / 1024.0 / 1024.0
	);
	println!(
		"   • Total available: {:.2} GB",
		stats.total_available as f64 / 1024.0 / 1024.0 / 1024.0
	);

	// Show disk type breakdown
	if !stats.by_type.is_empty() {
		println!("   • By disk type:");
		for (disk_type, count) in &stats.by_type {
			println!("     - {:?}: {}", disk_type, count);
		}
	}

	// Show filesystem breakdown
	if !stats.by_filesystem.is_empty() {
		println!("   • By filesystem:");
		for (fs, count) in &stats.by_filesystem {
			println!("     - {}: {}", fs, count);
		}
	}

	// List all detected volumes
	println!("\n3. Detected Volumes:");
	let volumes = core.volumes.get_all_volumes().await;

	if volumes.is_empty() {
		println!("   No volumes detected (possibly running in restricted environment)");
	} else {
		for (i, volume) in volumes.iter().enumerate() {
			println!("   {}. {} ({})", i + 1, volume.name, volume.fingerprint);
			println!("      Path: {}", volume.mount_point.display());
			println!(
				"      Type: {} | Filesystem: {} | Disk: {:?}",
				volume.mount_type, volume.file_system, volume.disk_type
			);
			println!(
				"      Capacity: {:.2} GB | Available: {:.2} GB",
				volume.total_bytes_capacity as f64 / 1024.0 / 1024.0 / 1024.0,
				volume.total_bytes_available as f64 / 1024.0 / 1024.0 / 1024.0
			);
			println!(
				"      Mounted: {} | Read-only: {}",
				volume.is_mounted, volume.read_only
			);

			// Show capabilities
			let supports_fast_copy = volume.supports_fast_copy();
			let optimal_chunk = volume.optimal_chunk_size();
			println!(
				"      Fast copy support: {} | Optimal chunk: {}KB",
				supports_fast_copy,
				optimal_chunk / 1024
			);

			if let (Some(read), Some(write)) = (volume.read_speed_mbps, volume.write_speed_mbps) {
				println!("      Speed: {}MB/s read, {}MB/s write", read, write);
			}

			println!();
		}
	}

	// Test path lookup
	println!("4. Testing Path-to-Volume Lookup:");
	let test_paths = [
		std::env::temp_dir(),
		std::env::current_dir().unwrap_or_default(),
		std::path::PathBuf::from("/"),
		std::path::PathBuf::from("/tmp"),
		std::path::PathBuf::from("/Users"),
		std::path::PathBuf::from("/home"),
		std::path::PathBuf::from("C:\\"),
		std::path::PathBuf::from("C:\\Windows"),
	];

	for path in &test_paths {
		if path.exists() {
			if let Some(volume) = core.volumes.volume_for_path(path).await {
				println!(
					"   {} → {} ({})",
					path.display(),
					volume.name,
					volume.file_system
				);
			} else {
				println!("   {} → No volume found", path.display());
			}
		}
	}

	// Test same volume detection
	println!("\n5. Testing Same-Volume Detection:");
	let temp_dir = std::env::temp_dir();
	let current_dir = std::env::current_dir().unwrap_or_default();

	if temp_dir.exists() && current_dir.exists() {
		let same_volume = core.volumes.same_volume(&temp_dir, &current_dir).await;
		println!(
			"   Temp directory and current directory on same volume: {}",
			same_volume
		);
	}

	// Test volume space queries
	println!("\n6. Testing Volume Space Queries:");
	let space_requirements = [
		(1024 * 1024 * 1024, "1 GB"),            // 1GB
		(10 * 1024 * 1024 * 1024u64, "10 GB"),   // 10GB
		(100 * 1024 * 1024 * 1024u64, "100 GB"), // 100GB
	];

	for (bytes, description) in &space_requirements {
		let volumes_with_space = core.volumes.volumes_with_space(*bytes).await;
		println!(
			"   Volumes with at least {}: {}",
			description,
			volumes_with_space.len()
		);
	}

	// Test volume monitoring events (if we have volumes)
	if !volumes.is_empty() {
		println!("\n7. Testing Volume Events (5 second window):");
		let mut subscriber = core.events.subscribe();

		// Force a volume refresh to generate events
		let _ = core.volumes.refresh_volumes().await;

		let event_timeout = timeout(Duration::from_secs(5), async {
			let mut event_count = 0;
			loop {
				match subscriber.recv().await {
					Ok(event) => {
						if event.is_volume_event() {
							event_count += 1;
							println!("   Volume event received: {:?}", event);

							if event_count >= 3 {
								break; // Don't wait for too many events
							}
						}
					}
					Err(_) => break,
				}
			}
			event_count
		})
		.await;

		match event_timeout {
			Ok(count) => println!("   Received {} volume events", count),
			Err(_) => println!("   No volume events received in timeout window"),
		}
	}

	// Run speed test on a suitable volume (if any)
	println!("\n8. Testing Volume Speed (optional):");
	let writable_volumes: Vec<_> = volumes
		.iter()
		.filter(|v| v.is_mounted && !v.read_only && v.mount_type != MountType::Network)
		.collect();

	if let Some(volume) = writable_volumes.first() {
		println!("   Running speed test on: {}", volume.name);

		match core.volumes.run_speed_test(&volume.fingerprint).await {
			Ok(()) => {
				if let Some(updated_volume) = core.volumes.get_volume(&volume.fingerprint).await {
					if let (Some(read), Some(write)) = (
						updated_volume.read_speed_mbps,
						updated_volume.write_speed_mbps,
					) {
						println!(
							"   ✓ Speed test completed: {}MB/s read, {}MB/s write",
							read, write
						);
					}
				}
			}
			Err(e) => {
				println!("   ⚠ Speed test failed: {}", e);
			}
		}
	} else {
		println!("   No suitable volumes found for speed testing");
	}

	// Show platform-specific information
	println!("\n9. Platform Information:");
	println!("   Operating System: {}", std::env::consts::OS);
	println!("   Architecture: {}", std::env::consts::ARCH);

	// Platform-specific notes
	match std::env::consts::OS {
		"macos" => {
			println!("   Note: macOS APFS volumes support instant cloning (copy-on-write)");
			let apfs_volumes = volumes
				.iter()
				.filter(|v| v.file_system == FileSystem::APFS)
				.count();
			if apfs_volumes > 0 {
				println!(
					"   Found {} APFS volumes with fast copy support",
					apfs_volumes
				);
			}
		}
		"linux" => {
			println!("   Note: Btrfs and ZFS support instant copying via reflinks");
			let cow_volumes = volumes
				.iter()
				.filter(|v| matches!(v.file_system, FileSystem::Btrfs | FileSystem::ZFS))
				.count();
			if cow_volumes > 0 {
				println!("   Found {} CoW filesystem volumes", cow_volumes);
			}
		}
		"windows" => {
			println!("   Note: ReFS supports block cloning for fast copies");
			let refs_volumes = volumes
				.iter()
				.filter(|v| v.file_system == FileSystem::ReFS)
				.count();
			if refs_volumes > 0 {
				println!(
					"   Found {} ReFS volumes with fast copy support",
					refs_volumes
				);
			}
		}
		_ => {
			println!("   Volume detection may be limited on this platform");
		}
	}

	println!("\n10. Integration with Copy Operations:");
	println!("   The volume system enables:");
	println!("   • Automatic copy strategy selection (instant vs streaming)");
	println!("   • Optimal chunk size determination based on disk type");
	println!("   • Cross-volume operation detection");
	println!("   • Performance-aware routing");

	if !volumes.is_empty() {
		// Show copy strategy examples
		let first_volume = &volumes[0];
		if volumes.len() > 1 {
			let second_volume = &volumes[1];
			println!("\n   Example copy strategies:");
			println!(
				"   {} → {} (same volume): {}",
				first_volume.name,
				first_volume.name,
				if first_volume.supports_fast_copy() {
					"Instant clone"
				} else {
					"Optimized copy"
				}
			);
			println!(
				"   {} → {} (cross-volume): Streaming copy with {}KB chunks",
				first_volume.name,
				second_volume.name,
				first_volume.optimal_chunk_size() / 1024
			);
		}
	}

	println!("\n✅ Volume system demo completed!");
	println!("\nThe volume system provides:");
	println!("• Cross-platform volume detection");
	println!("• Real-time monitoring and event emission");
	println!("• Performance testing and optimization");
	println!("• Integration with Core event bus");
	println!("• Foundation for intelligent copy operations");

	Ok(())
}
