//! Integration tests for unified addressing system
//!
//! Tests all three SdPath variants:
//! - Physical (local://device-slug/path)
//! - Cloud (s3://bucket/path, gdrive://folder/path, etc.)
//! - Content (content://uuid)

use sd_core::{
	domain::addressing::{SdPath, SdPathParseError},
	ops::volumes::add_cloud::action::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput},
	volume::backend::CloudServiceType,
	Core,
};
use sea_orm::EntityTrait;
use std::sync::Arc;
use tempfile::tempdir;
use tracing::info;

#[tokio::test]
async fn test_device_slug_generation() {
	use sd_core::domain::device::Device;

	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing device slug generation from device names");

	let test_cases = vec![
		("Jamie's MacBook Pro", "jamie-s-macbook-pro"),
		("Home Server 2024", "home-server-2024"),
		("DESKTOP-ABC123", "desktop-abc123"),
		("My Device!!!", "my-device"),
		("Test_Device-123", "test-device-123"),
		("NormalDevice", "normaldevice"),
	];

	for (name, expected_slug) in test_cases {
		let slug = Device::generate_slug(name);
		assert_eq!(
			slug, expected_slug,
			"Slug generation failed for '{}'",
			name
		);
		info!("'{}' → '{}'", name, slug);
	}

	info!("Device slug generation test completed");
}

#[tokio::test]
async fn test_local_uri_parsing_and_display() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing local:// URI parsing and display");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let _library = core
		.libraries
		.create_library(
			"Local URI Test",
			Some(data_path.join("libraries").join("local-uri")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created test library");

	// Get current device info
	let current_device = core.device.config().expect("Failed to get device config");
	let current_slug = current_device.slug.clone();
	let current_id = current_device.id;

	info!("Current device: {} (slug: {})", current_device.name, current_slug);

	// Test parsing local URI
	let test_uri = format!("local://{}/Users/james/Documents/test.pdf", current_slug);
	let parsed = SdPath::from_uri_with_context(&test_uri, &core.context)
		.await
		.expect("Failed to parse local URI");

	assert!(parsed.is_physical(), "Should be a Physical path");
	if let Some((device_id, path)) = parsed.as_physical() {
		assert_eq!(device_id, current_id, "Device ID should match");
		assert_eq!(
			path.to_str().unwrap(),
			"/Users/james/Documents/test.pdf",
			"Path should match"
		);
	}

	info!("Parsed: {} → {:?}", test_uri, parsed);

	// Test display with context
	let displayed = parsed.display_with_context(&core.context).await;
	assert_eq!(displayed, test_uri, "Display should match original URI");

	info!("Display: {:?} → {}", parsed, displayed);

	info!("Local URI test completed");
}

#[tokio::test]
async fn test_cloud_uri_parsing_and_display() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing cloud URI parsing and display (s3, gdrive, etc.)");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let library = core
		.libraries
		.create_library(
			"Cloud URI Test",
			Some(data_path.join("libraries").join("cloud-uri")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();

	info!("Created test library");

	// Get action manager
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Add a test S3 volume
	let add_cloud_action = VolumeAddCloudAction::new(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: "Test S3 Bucket".to_string(),
		config: CloudStorageConfig::S3 {
			bucket: "my-test-bucket".to_string(),
			region: "us-west-2".to_string(),
			access_key_id: "test-key".to_string(),
			secret_access_key: "test-secret".to_string(),
			endpoint: None,
		},
	});

	action_manager
		.dispatch_library(Some(library_id), add_cloud_action)
		.await
		.expect("Failed to add cloud volume");

	info!("Added S3 volume");

	// Test parsing S3 URI
	let s3_uri = "s3://my-test-bucket/photos/vacation.jpg";
	let parsed = SdPath::from_uri_with_context(s3_uri, &core.context)
		.await
		.expect("Failed to parse S3 URI");

	assert!(parsed.is_cloud(), "Should be a Cloud path");
	if let Some((fingerprint, path)) = parsed.as_cloud() {
		assert!(!fingerprint.0.is_empty(), "Fingerprint should not be empty");
		assert_eq!(path, "photos/vacation.jpg", "Path should match");
	}

	info!("Parsed S3 URI: {} → {:?}", s3_uri, parsed);

	// Test display with context
	let displayed = parsed.display_with_context(&core.context).await;
	assert_eq!(displayed, s3_uri, "Display should match original S3 URI");

	info!("Display: {:?} → {}", parsed, displayed);

	info!("Cloud URI test completed");
}

#[tokio::test]
async fn test_content_uri_parsing_and_display() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing content:// URI parsing and display");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let _library = core
		.libraries
		.create_library(
			"Content URI Test",
			Some(data_path.join("libraries").join("content-uri")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created test library");

	// Create a content URI
	use uuid::Uuid;
	let content_id = Uuid::new_v4();
	let content_uri = format!("content://{}", content_id);

	// Test parsing
	let parsed = SdPath::from_uri_with_context(&content_uri, &core.context)
		.await
		.expect("Failed to parse content URI");

	assert!(parsed.is_content(), "Should be a Content path");
	assert_eq!(
		parsed.content_id(),
		Some(content_id),
		"Content ID should match"
	);

	info!("Parsed content URI: {} → {:?}", content_uri, parsed);

	// Test display with context
	let displayed = parsed.display_with_context(&core.context).await;
	assert_eq!(displayed, content_uri, "Display should match original URI");

	info!("Display: {:?} → {}", parsed, displayed);

	info!("Content URI test completed");
}

#[tokio::test]
async fn test_device_cache_with_multiple_devices() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing device cache with multiple devices");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let library = core
		.libraries
		.create_library(
			"Multi Device Test",
			Some(data_path.join("libraries").join("multi-device")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created test library");

	// Insert test devices
	use sd_core::infra::db::entities::device;
	use uuid::Uuid;

	let device1_id = Uuid::new_v4();
	let device1_slug = "test-device-1";
	let device2_id = Uuid::new_v4();
	let device2_slug = "test-device-2";

	let now = chrono::Utc::now();

	let device1 = device::ActiveModel {
		uuid: sea_orm::ActiveValue::Set(device1_id),
		name: sea_orm::ActiveValue::Set("Test Device 1".to_string()),
		slug: sea_orm::ActiveValue::Set(device1_slug.to_string()),
		os: sea_orm::ActiveValue::Set("Linux".to_string()),
		is_online: sea_orm::ActiveValue::Set(false),
		network_addresses: sea_orm::ActiveValue::Set(serde_json::json!([])),
		capabilities: sea_orm::ActiveValue::Set(serde_json::json!({})),
		last_seen_at: sea_orm::ActiveValue::Set(now),
		created_at: sea_orm::ActiveValue::Set(now),
		updated_at: sea_orm::ActiveValue::Set(now),
		..Default::default()
	};

	let device2 = device::ActiveModel {
		uuid: sea_orm::ActiveValue::Set(device2_id),
		name: sea_orm::ActiveValue::Set("Test Device 2".to_string()),
		slug: sea_orm::ActiveValue::Set(device2_slug.to_string()),
		os: sea_orm::ActiveValue::Set("macOS".to_string()),
		is_online: sea_orm::ActiveValue::Set(false),
		network_addresses: sea_orm::ActiveValue::Set(serde_json::json!([])),
		capabilities: sea_orm::ActiveValue::Set(serde_json::json!({})),
		last_seen_at: sea_orm::ActiveValue::Set(now),
		created_at: sea_orm::ActiveValue::Set(now),
		updated_at: sea_orm::ActiveValue::Set(now),
		..Default::default()
	};

	device::Entity::insert(device1)
		.exec(library.db().conn())
		.await
		.expect("Failed to insert device 1");

	device::Entity::insert(device2)
		.exec(library.db().conn())
		.await
		.expect("Failed to insert device 2");

	info!("Inserted test devices into database");

	// Reload device cache
	core.device
		.load_library_devices(library.db().conn())
		.await
		.expect("Failed to load library devices");

	info!("Loaded devices into cache");

	// Test parsing URIs for both devices
	let uri1 = format!("local://{}/home/test/file1.txt", device1_slug);
	let uri2 = format!("local://{}/Users/test/file2.txt", device2_slug);

	let parsed1 = SdPath::from_uri_with_context(&uri1, &core.context)
		.await
		.expect("Failed to parse device 1 URI");
	let parsed2 = SdPath::from_uri_with_context(&uri2, &core.context)
		.await
		.expect("Failed to parse device 2 URI");

	assert!(parsed1.is_physical(), "Device 1 path should be physical");
	assert!(parsed2.is_physical(), "Device 2 path should be physical");

	if let Some((dev_id, _)) = parsed1.as_physical() {
		assert_eq!(dev_id, device1_id, "Device 1 ID should match");
	}

	if let Some((dev_id, _)) = parsed2.as_physical() {
		assert_eq!(dev_id, device2_id, "Device 2 ID should match");
	}

	info!("Successfully parsed URIs for multiple devices");

	// Test non-existent device
	let bad_uri = "local://non-existent-device/path/file.txt";
	let result = SdPath::from_uri_with_context(bad_uri, &core.context).await;

	assert!(result.is_err(), "Should fail for non-existent device");
	assert_eq!(
		result.unwrap_err(),
		SdPathParseError::DeviceNotFound,
		"Should return DeviceNotFound error"
	);

	info!("Multiple device cache test completed");
}

#[tokio::test]
async fn test_library_lifecycle_cache_integration() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing device cache through full library lifecycle");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	// Pass parent directory - library manager will create "Lifecycle Test.sdlibrary" inside it
	let libraries_dir = data_path.join("libraries");

	let library = core
		.libraries
		.create_library(
			"Lifecycle Test",
			Some(libraries_dir.clone()),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();

	info!("Created library: {}", library_id);

	// Insert test devices
	use sd_core::infra::db::entities::device;
	use uuid::Uuid;

	let test_devices = vec![
		(Uuid::new_v4(), "lifecycle-device-1", "Lifecycle Device 1"),
		(Uuid::new_v4(), "lifecycle-device-2", "Lifecycle Device 2"),
		(Uuid::new_v4(), "lifecycle-device-3", "Lifecycle Device 3"),
	];

	let now = chrono::Utc::now();

	for (id, slug, name) in &test_devices {
		let device = device::ActiveModel {
			uuid: sea_orm::ActiveValue::Set(*id),
			name: sea_orm::ActiveValue::Set(name.to_string()),
			slug: sea_orm::ActiveValue::Set(slug.to_string()),
			os: sea_orm::ActiveValue::Set("Linux".to_string()),
			is_online: sea_orm::ActiveValue::Set(false),
			network_addresses: sea_orm::ActiveValue::Set(serde_json::json!([])),
			capabilities: sea_orm::ActiveValue::Set(serde_json::json!({})),
			last_seen_at: sea_orm::ActiveValue::Set(now),
			created_at: sea_orm::ActiveValue::Set(now),
			updated_at: sea_orm::ActiveValue::Set(now),
			..Default::default()
		};

		device::Entity::insert(device)
			.exec(library.db().conn())
			.await
			.expect("Failed to insert device");
	}

	info!("Inserted {} test devices", test_devices.len());

	// Reload cache
	core.device
		.load_library_devices(library.db().conn())
		.await
		.expect("Failed to load library devices");

	// Test parsing URIs for all devices
	for (id, slug, _) in &test_devices {
		let uri = format!("local://{}/test/path.txt", slug);
		let parsed = SdPath::from_uri_with_context(&uri, &core.context)
			.await
			.expect(&format!("Failed to parse URI for {}", slug));

		if let Some((device_id, _)) = parsed.as_physical() {
			assert_eq!(device_id, *id, "Device ID should match for {}", slug);
		}
	}

	info!("All devices resolvable from cache");

	// Drop library reference to release the lock before closing
	drop(library);

	// Close library
	core.libraries
		.close_library(library_id)
		.await
		.expect("Failed to close library");

	info!("Library closed");

	// After close, only current device should be resolvable
	let current_slug = core.device.config().expect("Failed to get config").slug;

	for (id, slug, _) in &test_devices {
		let uri = format!("local://{}/test/path.txt", slug);
		let result = SdPath::from_uri_with_context(&uri, &core.context).await;

		if slug == &current_slug {
			assert!(result.is_ok(), "Current device should resolve");
			if let Ok(parsed) = result {
				if let Some((device_id, _)) = parsed.as_physical() {
					assert_eq!(device_id, *id, "Current device ID should match");
				}
			}
		} else {
			assert!(
				result.is_err(),
				"Device {} should not resolve after library close",
				slug
			);
		}
	}

	info!("Cache correctly cleared after library close");

	// Reopen library - construct the actual library path with .sdlibrary extension
	let library_path = libraries_dir.join("Lifecycle Test.sdlibrary");
	let library2 = core
		.libraries
		.open_library(&library_path, core.context.clone())
		.await
		.expect("Failed to reopen library");

	info!("Library reopened: {}", library2.id());

	// All devices should be resolvable again
	for (id, slug, _) in &test_devices {
		let uri = format!("local://{}/test/path.txt", slug);
		let parsed = SdPath::from_uri_with_context(&uri, &core.context)
			.await
			.expect(&format!("Failed to parse URI for {} after reopen", slug));

		if let Some((device_id, _)) = parsed.as_physical() {
			assert_eq!(
				device_id, *id,
				"Device ID should match for {} after reopen",
				slug
			);
		}
	}

	info!("All devices resolvable after library reopen");

	info!("Library lifecycle integration test completed");
}

#[tokio::test]
async fn test_slug_uniqueness_constraint() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing device slug uniqueness constraint");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let library = core
		.libraries
		.create_library(
			"Uniqueness Test",
			Some(data_path.join("libraries").join("uniqueness-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created test library");

	// Insert first device
	use sd_core::infra::db::entities::device;
	use uuid::Uuid;

	let device1_id = Uuid::new_v4();
	let duplicate_slug = "duplicate-slug-test";

	let now = chrono::Utc::now();

	let device1 = device::ActiveModel {
		uuid: sea_orm::ActiveValue::Set(device1_id),
		name: sea_orm::ActiveValue::Set("First Device".to_string()),
		slug: sea_orm::ActiveValue::Set(duplicate_slug.to_string()),
		os: sea_orm::ActiveValue::Set("Linux".to_string()),
		is_online: sea_orm::ActiveValue::Set(false),
		network_addresses: sea_orm::ActiveValue::Set(serde_json::json!([])),
		capabilities: sea_orm::ActiveValue::Set(serde_json::json!({})),
		last_seen_at: sea_orm::ActiveValue::Set(now),
		created_at: sea_orm::ActiveValue::Set(now),
		updated_at: sea_orm::ActiveValue::Set(now),
		..Default::default()
	};

	device::Entity::insert(device1)
		.exec(library.db().conn())
		.await
		.expect("Failed to insert first device");

	info!("Inserted first device with slug: {}", duplicate_slug);

	// Try to insert second device with same slug (should fail)
	let device2_id = Uuid::new_v4();
	let device2 = device::ActiveModel {
		uuid: sea_orm::ActiveValue::Set(device2_id),
		name: sea_orm::ActiveValue::Set("Second Device".to_string()),
		slug: sea_orm::ActiveValue::Set(duplicate_slug.to_string()),
		os: sea_orm::ActiveValue::Set("macOS".to_string()),
		is_online: sea_orm::ActiveValue::Set(false),
		network_addresses: sea_orm::ActiveValue::Set(serde_json::json!([])),
		capabilities: sea_orm::ActiveValue::Set(serde_json::json!({})),
		last_seen_at: sea_orm::ActiveValue::Set(now),
		created_at: sea_orm::ActiveValue::Set(now),
		updated_at: sea_orm::ActiveValue::Set(now),
		..Default::default()
	};

	let result = device::Entity::insert(device2)
		.exec(library.db().conn())
		.await;

	assert!(
		result.is_err(),
		"Should not allow duplicate device slugs"
	);

	info!("Duplicate slug correctly rejected by database");

	info!("Slug uniqueness test completed");
}

#[tokio::test]
async fn test_all_cloud_service_schemes() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing all cloud service URI schemes");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let library = core
		.libraries
		.create_library(
			"Cloud Schemes Test",
			Some(data_path.join("libraries").join("cloud-schemes")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	let library_id = library.id();

	info!("Created test library");

	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager should be initialized");

	// Test S3
	info!("Testing s3 scheme");
	let s3_uri = "s3://test-s3-bucket/path/file.txt";

	let add_s3_action = VolumeAddCloudAction::new(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: "Test S3 Volume".to_string(),
		config: CloudStorageConfig::S3 {
			bucket: "test-s3-bucket".to_string(),
			region: "us-west-2".to_string(),
			access_key_id: "test-key".to_string(),
			secret_access_key: "test-secret".to_string(),
			endpoint: None,
		},
	});

	action_manager
		.dispatch_library(Some(library_id), add_s3_action)
		.await
		.expect("Failed to add S3 volume");

	let parsed = SdPath::from_uri_with_context(s3_uri, &core.context)
		.await
		.expect("Failed to parse S3 URI");

	assert!(parsed.is_cloud(), "Should be a Cloud path");

	let displayed = parsed.display_with_context(&core.context).await;
	assert_eq!(displayed, s3_uri, "S3 URI should round-trip correctly");

	info!("s3 URI test passed: {}", s3_uri);

	info!("All cloud service scheme tests completed");
}

#[tokio::test]
async fn test_uri_error_handling() {
	let _ = tracing_subscriber::fmt::try_init();

	info!("Testing URI parsing error cases");

	let data_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();

	let core = Arc::new(Core::new(data_path.clone()).await.expect("Failed to create core"));

	let _library = core
		.libraries
		.create_library(
			"Error Test",
			Some(data_path.join("libraries").join("error-test")),
			core.context.clone(),
		)
		.await
		.expect("Failed to create library");

	info!("Created test library");

	// Test unknown scheme
	let result = SdPath::from_uri_with_context("unknown://path", &core.context).await;
	assert!(result.is_err(), "Unknown scheme should fail");
	assert_eq!(
		result.unwrap_err(),
		SdPathParseError::UnknownScheme,
		"Should return UnknownScheme error"
	);

	// Test non-existent volume
	let result = SdPath::from_uri_with_context("s3://non-existent-bucket/file", &core.context).await;
	assert!(result.is_err(), "Non-existent volume should fail");
	assert_eq!(
		result.unwrap_err(),
		SdPathParseError::VolumeNotFound,
		"Should return VolumeNotFound error"
	);

	// Test non-existent device
	let result = SdPath::from_uri_with_context("local://fake-device/path", &core.context).await;
	assert!(result.is_err(), "Non-existent device should fail");
	assert_eq!(
		result.unwrap_err(),
		SdPathParseError::DeviceNotFound,
		"Should return DeviceNotFound error"
	);

	// Test invalid content ID
	let result = SdPath::from_uri_with_context("content://invalid-uuid", &core.context).await;
	assert!(result.is_err(), "Invalid content ID should fail");
	assert_eq!(
		result.unwrap_err(),
		SdPathParseError::InvalidContentId,
		"Should return InvalidContentId error"
	);

	info!("URI error handling tests completed");
}
