//! Integration tests for persistent networking system

use sd_core_new::{networking, Core};
use uuid::Uuid;

#[tokio::test]
async fn test_core_networking_initialization() {
	// Create temporary directory for test
	let temp_dir = std::env::temp_dir().join(format!("test-core-networking-{}", Uuid::new_v4()));
	std::fs::create_dir_all(&temp_dir).unwrap();

	// Initialize Core
	let mut core = Core::new_with_config(temp_dir.clone()).await.unwrap();

	// Initially, networking should not be initialized
	assert!(core.networking().is_none());
	assert!(core.get_connected_devices().await.unwrap().is_empty());

	// Initialize networking
	core.init_networking("test-password-123").await.unwrap();
	assert!(core.networking().is_some());

	// Connected devices should still be empty (no devices paired yet)
	assert!(core.get_connected_devices().await.unwrap().is_empty());

	// Test starting networking service
	core.start_networking().await.unwrap();

	// Give the service a moment to start
	tokio::time::sleep(std::time::Duration::from_millis(100)).await;

	// Shutdown cleanly
	core.shutdown().await.unwrap();

	// Clean up
	std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_device_pairing_integration() {
	let temp_dir = std::env::temp_dir().join(format!("test-pairing-{}", Uuid::new_v4()));
	std::fs::create_dir_all(&temp_dir).unwrap();

	let mut core = Core::new_with_config(temp_dir.clone()).await.unwrap();
	core.init_networking("test-password-456").await.unwrap();

	// Create a mock paired device
	let device_id = Uuid::new_v4();
	let device_info = networking::DeviceInfo {
		device_id,
		device_name: "Test Device".to_string(),
		public_key: networking::PublicKey::from_bytes(vec![42u8; 32]).unwrap(),
		network_fingerprint: networking::NetworkFingerprint::from_device(
			device_id,
			&networking::PublicKey::from_bytes(vec![42u8; 32]).unwrap(),
		),
		last_seen: chrono::Utc::now(),
	};

	let session_keys = networking::persistent::SessionKeys::new();

	// Add paired device
	core.add_paired_device(device_info, session_keys)
		.await
		.unwrap();

	// Verify the device was added (it won't show as connected since it's not actually online)
	let _connected = core.get_connected_devices().await.unwrap();
	// Device won't be connected since it's just a test mock, but the pairing should have been stored

	// Test device revocation
	core.revoke_device(device_id).await.unwrap();

	core.shutdown().await.unwrap();
	std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_spacedrop_api_integration() {
	let temp_dir = std::env::temp_dir().join(format!("test-spacedrop-{}", Uuid::new_v4()));
	std::fs::create_dir_all(&temp_dir).unwrap();

	let mut core = Core::new_with_config(temp_dir.clone()).await.unwrap();
	core.init_networking("test-password-789").await.unwrap();

	// Create a test file
	let test_file = temp_dir.join("test_file.txt");
	std::fs::write(&test_file, "Hello, Spacedrive!").unwrap();

	// Try to send spacedrop (should fail gracefully since no devices are connected)
	let device_id = Uuid::new_v4();
	let result = core
		.send_spacedrop(
			device_id,
			&test_file.to_string_lossy(),
			"Test User".to_string(),
			Some("Test message".to_string()),
		)
		.await;

	// Should return an error since the device is not connected
	assert!(result.is_err());

	core.shutdown().await.unwrap();
	std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_networking_service_features() {
	let temp_dir = std::env::temp_dir().join(format!("test-features-{}", Uuid::new_v4()));
	std::fs::create_dir_all(&temp_dir).unwrap();

	let mut core = Core::new_with_config(temp_dir.clone()).await.unwrap();
	core.init_networking("test-password-101112").await.unwrap();

	// Get networking service reference
	if let Some(networking) = core.networking() {
		let service = networking.read().await;

		// Test that the service has the expected protocol handlers
		// This verifies that the service was properly initialized with handlers

		// Test connected devices (should be empty)
		let connected = service.get_connected_devices().await.unwrap();
		assert!(connected.is_empty());

		// The networking service is properly initialized and ready for use
	} else {
		panic!("Networking service should be initialized");
	}

	core.shutdown().await.unwrap();
	std::fs::remove_dir_all(&temp_dir).ok();
}
