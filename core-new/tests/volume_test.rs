// //! Volume system integration tests

// use sd_core_new::{
// 	infrastructure::events::{EventBus, EventFilter},
// 	volume::{
// 		types::{DiskType, FileSystem, MountType, VolumeDetectionConfig},
// 		VolumeExt, VolumeManager,
// 	},
// };
// use std::sync::Arc;
// use std::time::Duration;
// use tempfile::TempDir;
// use tokio::time::timeout;

// #[tokio::test]
// async fn test_volume_manager_initialization() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	// Should initialize without error
// 	let result = manager.initialize().await;
// 	assert!(result.is_ok());

// 	// Should have detected some volumes (unless running in very minimal environment)
// 	let volumes = manager.get_all_volumes().await;
// 	println!("Detected {} volumes", volumes.len());

// 	for volume in &volumes {
// 		println!(
// 			"Volume: {} - {} - {} ({:?})",
// 			volume.name,
// 			volume.mount_point.display(),
// 			volume.file_system,
// 			volume.disk_type
// 		);
// 	}
// }

// #[tokio::test]
// async fn test_volume_detection_config() {
// 	let events = Arc::new(EventBus::default());

// 	// Test with system volumes excluded
// 	let config = VolumeDetectionConfig {
// 		include_system: false,
// 		include_virtual: false,
// 		run_speed_test: false,
// 		refresh_interval_secs: 0, // No monitoring
// 	};

// 	let manager = VolumeManager::new(config, events.clone());
// 	manager.initialize().await.unwrap();

// 	let volumes = manager.get_all_volumes().await;

// 	// Verify no system volumes are included
// 	for volume in &volumes {
// 		assert_ne!(volume.mount_type, MountType::System);
// 		assert_ne!(volume.mount_type, MountType::Virtual);
// 	}
// }

// #[tokio::test]
// async fn test_volume_path_lookup() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	// Test looking up volume for a common path
// 	let test_paths = [
// 		std::path::PathBuf::from("/"),
// 		std::path::PathBuf::from("/tmp"),
// 		std::path::PathBuf::from("/usr"),
// 		std::env::temp_dir(),
// 		std::env::current_dir().unwrap_or_default(),
// 	];

// 	for path in &test_paths {
// 		if path.exists() {
// 			let volume = manager.volume_for_path(path).await;
// 			if let Some(vol) = volume {
// 				println!("Path {} is on volume: {}", path.display(), vol.name);

// 				// Verify the volume actually contains this path
// 				assert!(vol.contains_path(path));
// 			} else {
// 				println!("No volume found for path: {}", path.display());
// 			}
// 		}
// 	}
// }

// #[tokio::test]
// async fn test_volume_events() {
// 	let events = Arc::new(EventBus::default());
// 	let mut subscriber = events.subscribe();

// 	let config = VolumeDetectionConfig {
// 		include_system: true,
// 		include_virtual: false,
// 		run_speed_test: false,
// 		refresh_interval_secs: 0,
// 	};

// 	let manager = VolumeManager::new(config, events.clone());

// 	// Initialize and wait for events
// 	manager.initialize().await.unwrap();

// 	// Try to receive events with a timeout
// 	let event_result = timeout(Duration::from_millis(100), async {
// 		loop {
// 			match subscriber.recv().await {
// 				Ok(event) => {
// 					if event.is_volume_event() {
// 						return Some(event);
// 					}
// 				}
// 				Err(_) => return None,
// 			}
// 		}
// 	})
// 	.await;

// 	match event_result {
// 		Ok(Some(event)) => {
// 			println!("Received volume event: {:?}", event);
// 		}
// 		Ok(None) => {
// 			println!("No volume events received");
// 		}
// 		Err(_) => {
// 			println!("Timeout waiting for volume events");
// 		}
// 	}
// }

// #[tokio::test]
// async fn test_volume_statistics() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let stats = manager.get_statistics().await;

// 	println!("Volume Statistics:");
// 	println!("  Total volumes: {}", stats.total_volumes);
// 	println!("  Mounted volumes: {}", stats.mounted_volumes);
// 	println!(
// 		"  Total capacity: {:.2} GB",
// 		stats.total_capacity as f64 / 1024.0 / 1024.0 / 1024.0
// 	);
// 	println!(
// 		"  Total available: {:.2} GB",
// 		stats.total_available as f64 / 1024.0 / 1024.0 / 1024.0
// 	);

// 	println!("  By disk type:");
// 	for (disk_type, count) in &stats.by_type {
// 		println!("    {:?}: {}", disk_type, count);
// 	}

// 	println!("  By filesystem:");
// 	for (fs, count) in &stats.by_filesystem {
// 		println!("    {}: {}", fs, count);
// 	}

// 	assert!(stats.total_volumes > 0 || cfg!(target_os = "unknown"));
// }

// #[tokio::test]
// async fn test_same_volume_check() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let temp_dir = std::env::temp_dir();
// 	let current_dir = std::env::current_dir().unwrap_or_default();

// 	if temp_dir.exists() && current_dir.exists() {
// 		let same_volume = manager.same_volume(&temp_dir, &current_dir).await;
// 		println!("Temp dir and current dir on same volume: {}", same_volume);

// 		// Test with same path (should always be true if volume is found)
// 		let same_self = manager.same_volume(&temp_dir, &temp_dir).await;
// 		if manager.volume_for_path(&temp_dir).await.is_some() {
// 			assert!(same_self);
// 		}
// 	}
// }

// #[tokio::test]
// async fn test_volume_space_check() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let volumes = manager.get_all_volumes().await;

// 	for volume in &volumes {
// 		// Test VolumeExt trait methods
// 		let available = volume.is_available().await;
// 		let has_1gb = volume.has_space(1024 * 1024 * 1024); // 1GB
// 		let has_1tb = volume.has_space(1024u64.pow(4)); // 1TB

// 		println!(
// 			"Volume {}: available={}, has_1gb={}, has_1tb={}",
// 			volume.name, available, has_1gb, has_1tb
// 		);
// 	}

// 	// Test finding volumes with specific space requirements
// 	let volumes_with_1gb = manager.volumes_with_space(1024 * 1024 * 1024).await;
// 	println!(
// 		"Volumes with at least 1GB space: {}",
// 		volumes_with_1gb.len()
// 	);
// }

// #[tokio::test]
// async fn test_volume_capabilities() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let volumes = manager.get_all_volumes().await;

// 	for volume in &volumes {
// 		println!(
// 			"Volume {}: filesystem={}, supports_fast_copy={}, optimal_chunk_size={}KB",
// 			volume.name,
// 			volume.file_system,
// 			volume.supports_fast_copy(),
// 			volume.optimal_chunk_size() / 1024
// 		);

// 		// Test filesystem capabilities
// 		let supports_reflink = volume.file_system.supports_reflink();
// 		let supports_sendfile = volume.file_system.supports_sendfile();

// 		println!(
// 			"  Reflink support: {}, Sendfile support: {}",
// 			supports_reflink, supports_sendfile
// 		);
// 	}
// }

// #[tokio::test]
// async fn test_volume_monitoring() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig {
// 		include_system: true,
// 		include_virtual: false,
// 		run_speed_test: false,
// 		refresh_interval_secs: 1, // Very short interval for test
// 	};

// 	let manager = VolumeManager::new(config, events.clone());
// 	manager.initialize().await.unwrap();

// 	// Let monitoring run for a short time
// 	tokio::time::sleep(Duration::from_millis(1500)).await;

// 	// Stop monitoring
// 	manager.stop_monitoring().await;

// 	// Verify manager still works after stopping monitoring
// 	let volumes = manager.get_all_volumes().await;
// 	println!("After monitoring test: {} volumes", volumes.len());
// }

// #[cfg(not(target_os = "unknown"))]
// #[tokio::test]
// async fn test_volume_speed_test() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let volumes = manager.get_all_volumes().await;

// 	// Find a writable volume to test
// 	for volume in &volumes {
// 		if !volume.read_only && volume.is_mounted {
// 			println!("Running speed test on volume: {}", volume.name);

// 			let result = manager.run_speed_test(&volume.fingerprint).await;

// 			match result {
// 				Ok(()) => {
// 					// Get updated volume info
// 					if let Some(updated_volume) = manager.get_volume(&volume.fingerprint).await {
// 						if let (Some(read_speed), Some(write_speed)) = (
// 							updated_volume.read_speed_mbps,
// 							updated_volume.write_speed_mbps,
// 						) {
// 							println!(
// 								"Speed test results: {}MB/s read, {}MB/s write",
// 								read_speed, write_speed
// 							);
// 							assert!(read_speed > 0);
// 							assert!(write_speed > 0);
// 						}
// 					}

// 					// Only test one volume to keep test time reasonable
// 					break;
// 				}
// 				Err(e) => {
// 					println!("Speed test failed for {}: {}", volume.name, e);
// 					// Continue to next volume
// 				}
// 			}
// 		}
// 	}
// }

// #[tokio::test]
// async fn test_volume_fingerprinting() {
// 	let events = Arc::new(EventBus::default());
// 	let config = VolumeDetectionConfig::default();
// 	let manager = VolumeManager::new(config, events);

// 	manager.initialize().await.unwrap();

// 	let volumes = manager.get_all_volumes().await;

// 	for volume in &volumes {
// 		// Verify fingerprint is not empty
// 		assert!(!volume.fingerprint.to_string().is_empty());

// 		// Verify fingerprint is consistent
// 		let fingerprint1 = volume.fingerprint.clone();
// 		let fingerprint2 = crate::volume::types::VolumeFingerprint::new(volume);
// 		assert_eq!(fingerprint1, fingerprint2);

// 		println!("Volume {} fingerprint: {}", volume.name, volume.fingerprint);
// 	}

// 	// Verify that different volumes have different fingerprints
// 	let mut fingerprints = std::collections::HashSet::new();
// 	for volume in &volumes {
// 		assert!(
// 			fingerprints.insert(volume.fingerprint.clone()),
// 			"Duplicate fingerprint found for volume: {}",
// 			volume.name
// 		);
// 	}
// }
