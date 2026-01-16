//! Volume speed testing functionality

use crate::volume::{
	error::{VolumeError, VolumeResult},
	types::{MountType, Volume, VolumeType},
};
use std::time::Instant;
use tokio::{
	fs::{File, OpenOptions},
	io::{AsyncReadExt, AsyncWriteExt},
	time::{timeout, Duration},
};
use tracing::{debug, instrument, warn};

/// Configuration for speed tests
#[derive(Debug, Clone)]
pub struct SpeedTestConfig {
	/// Size of the test file in megabytes
	pub file_size_mb: usize,
	/// Timeout for the test in seconds
	pub timeout_secs: u64,
	/// Number of test iterations for averaging
	pub iterations: usize,
}

impl Default for SpeedTestConfig {
	fn default() -> Self {
		Self {
			file_size_mb: 10,
			timeout_secs: 30,
			iterations: 1,
		}
	}
}

/// Result of a speed test
#[derive(Debug, Clone)]
pub struct SpeedTestResult {
	/// Write speed in MB/s
	pub write_speed_mbps: f64,
	/// Read speed in MB/s
	pub read_speed_mbps: f64,
	/// Total time taken for the test
	pub duration_secs: f64,
}

/// Run a speed test on the given volume
#[instrument(skip(volume), fields(volume_name = %volume.name))]
pub async fn run_speed_test(volume: &Volume) -> VolumeResult<(u64, u64)> {
	run_speed_test_with_config(volume, SpeedTestConfig::default()).await
}

/// Run a speed test with custom configuration
#[instrument(skip(volume, config), fields(volume_name = %volume.name))]
pub async fn run_speed_test_with_config(
	volume: &Volume,
	config: SpeedTestConfig,
) -> VolumeResult<(u64, u64)> {
	if !volume.is_mounted {
		return Err(VolumeError::NotMounted(volume.name.clone()));
	}

	if volume.is_read_only {
		return Err(VolumeError::ReadOnly(volume.name.clone()));
	}

	debug!("Starting speed test with config: {:?}", config);

	let test_location = TestLocation::new(&volume.mount_point, &volume.mount_type).await?;
	let result = perform_speed_test(&test_location, &config).await?;

	// Cleanup
	test_location.cleanup().await?;

	debug!(
		"Speed test completed: {:.2} MB/s write, {:.2} MB/s read",
		result.write_speed_mbps, result.read_speed_mbps
	);

	Ok((
		result.read_speed_mbps as u64,
		result.write_speed_mbps as u64,
	))
}

/// Helper for managing test files and directories
struct TestLocation {
	test_file: std::path::PathBuf,
	created_dir: Option<std::path::PathBuf>,
}

impl TestLocation {
	/// Create a new test location
	async fn new(volume_path: &std::path::Path, mount_type: &MountType) -> VolumeResult<Self> {
		let (dir, created_dir) = get_writable_directory(volume_path, mount_type).await?;
		let test_file = dir.join("spacedrive_speed_test.tmp");

		Ok(Self {
			test_file,
			created_dir,
		})
	}

	/// Clean up test files and directories
	async fn cleanup(&self) -> VolumeResult<()> {
		// Remove test file
		if self.test_file.exists() {
			if let Err(e) = tokio::fs::remove_file(&self.test_file).await {
				warn!("Failed to remove test file: {}", e);
			}
		}

		// Remove created directory if we created it
		if let Some(ref dir) = self.created_dir {
			if let Err(e) = tokio::fs::remove_dir_all(dir).await {
				warn!("Failed to remove test directory: {}", e);
			}
		}

		Ok(())
	}
}

/// Perform the actual speed test
async fn perform_speed_test(
	location: &TestLocation,
	config: &SpeedTestConfig,
) -> VolumeResult<SpeedTestResult> {
	let test_data = generate_test_data(config.file_size_mb);
	let timeout_duration = Duration::from_secs(config.timeout_secs);

	let mut write_speeds = Vec::new();
	let mut read_speeds = Vec::new();
	let overall_start = Instant::now();

	for iteration in 0..config.iterations {
		debug!(
			"Speed test iteration {}/{}",
			iteration + 1,
			config.iterations
		);

		// Write test
		let write_speed = timeout(
			timeout_duration,
			perform_write_test(&location.test_file, &test_data),
		)
		.await
		.map_err(|_| VolumeError::Timeout)??;

		write_speeds.push(write_speed);

		// Read test
		let read_speed = timeout(
			timeout_duration,
			perform_read_test(&location.test_file, test_data.len()),
		)
		.await
		.map_err(|_| VolumeError::Timeout)??;

		read_speeds.push(read_speed);

		// Clean up test file between iterations
		if iteration < config.iterations - 1 {
			let _ = tokio::fs::remove_file(&location.test_file).await;
		}
	}

	let avg_write_speed = write_speeds.iter().sum::<f64>() / write_speeds.len() as f64;
	let avg_read_speed = read_speeds.iter().sum::<f64>() / read_speeds.len() as f64;

	Ok(SpeedTestResult {
		write_speed_mbps: avg_write_speed,
		read_speed_mbps: avg_read_speed,
		duration_secs: overall_start.elapsed().as_secs_f64(),
	})
}

/// Generate test data for speed testing
fn generate_test_data(size_mb: usize) -> Vec<u8> {
	let size_bytes = size_mb * 1024 * 1024;

	// Use a pattern instead of zeros to avoid compression optimizations
	let pattern = b"SpacedriveSpeedTest0123456789ABCDEF";
	let mut data = Vec::with_capacity(size_bytes);

	for i in 0..size_bytes {
		data.push(pattern[i % pattern.len()]);
	}

	data
}

/// Perform write speed test
async fn perform_write_test(file_path: &std::path::Path, data: &[u8]) -> VolumeResult<f64> {
	let start = Instant::now();

	let mut file = OpenOptions::new()
		.write(true)
		.create(true)
		.truncate(true)
		.open(file_path)
		.await?;

	file.write_all(data).await?;
	file.sync_all().await?; // Ensure data is written to disk

	let duration = start.elapsed();
	let speed_mbps = (data.len() as f64 / 1024.0 / 1024.0) / duration.as_secs_f64();

	Ok(speed_mbps)
}

/// Perform read speed test
async fn perform_read_test(file_path: &std::path::Path, expected_size: usize) -> VolumeResult<f64> {
	let start = Instant::now();

	let mut file = File::open(file_path).await?;
	let mut buffer = Vec::with_capacity(expected_size);
	file.read_to_end(&mut buffer).await?;

	let duration = start.elapsed();
	let speed_mbps = (buffer.len() as f64 / 1024.0 / 1024.0) / duration.as_secs_f64();

	Ok(speed_mbps)
}

/// Get a writable directory within the volume
async fn get_writable_directory(
	volume_path: &std::path::Path,
	mount_type: &MountType,
) -> VolumeResult<(std::path::PathBuf, Option<std::path::PathBuf>)> {
	match mount_type {
		MountType::System => {
			// For system volumes, prefer using temp directory
			let temp_dir = std::env::temp_dir();
			Ok((temp_dir, None))
		}
		_ => {
			// For external volumes, try to write in the root or create a temp directory
			let candidates = [
				volume_path.join("tmp"),
				volume_path.join(".spacedrive_temp"),
				volume_path.to_path_buf(),
			];

			for candidate in &candidates {
				// Try to create the directory
				if let Ok(()) = tokio::fs::create_dir_all(candidate).await {
					// Test if we can write to it
					let test_file = candidate.join("test_write_permissions");
					if tokio::fs::write(&test_file, b"test").await.is_ok() {
						let _ = tokio::fs::remove_file(&test_file).await;

						// If we created a directory specifically for this test, mark it for cleanup
						let created_dir = if candidate
							.file_name()
							.map_or(false, |name| name == "tmp" || name == ".spacedrive_temp")
						{
							Some(candidate.clone())
						} else {
							None
						};

						return Ok((candidate.clone(), created_dir));
					}
				}
			}

			Err(VolumeError::PermissionDenied(format!(
				"No writable directory found in volume: {}",
				volume_path.display()
			)))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::volume::{
		types::{DiskType, FileSystem},
		VolumeFingerprint,
	};
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_speed_test_config() {
		let config = SpeedTestConfig::default();
		assert_eq!(config.file_size_mb, 10);
		assert_eq!(config.timeout_secs, 30);
		assert_eq!(config.iterations, 1);
	}

	#[tokio::test]
	async fn test_generate_test_data() {
		let data = generate_test_data(1); // 1MB
		assert_eq!(data.len(), 1024 * 1024);

		// Verify pattern is not all zeros
		assert!(data.iter().any(|&b| b != 0));
	}

	#[tokio::test]
	async fn test_writable_directory_external() {
		let temp_dir = TempDir::new().unwrap();
		let volume_path = temp_dir.path();

		let (writable_dir, created_dir) = get_writable_directory(volume_path, &MountType::External)
			.await
			.unwrap();

		assert!(writable_dir.exists());

		// Cleanup if we created a directory
		if let Some(dir) = created_dir {
			let _ = tokio::fs::remove_dir_all(dir).await;
		}
	}

	#[tokio::test]
	async fn test_writable_directory_system() {
		let (writable_dir, created_dir) =
			get_writable_directory(&std::path::PathBuf::from("/"), &MountType::System)
				.await
				.unwrap();

		assert!(writable_dir.exists());
		assert!(created_dir.is_none()); // Should use system temp, not create new dir
	}

	#[tokio::test]
	async fn test_full_speed_test() {
		let temp_dir = TempDir::new().unwrap();

		let fingerprint = VolumeFingerprint::new("Test Volume", 1000000000, "test");
		let now = chrono::Utc::now();
		let mount_path = temp_dir.path().to_path_buf();

		let volume = Volume {
			id: uuid::Uuid::new_v4(),
			fingerprint,
			cloud_identifier: None,
			cloud_config: None,
			device_id: uuid::Uuid::new_v4(),
			name: "Test Volume".to_string(),
			library_id: None,
			is_tracked: false,
			mount_point: mount_path.clone(),
			mount_points: vec![mount_path],
			volume_type: VolumeType::External,
			mount_type: MountType::External,
			disk_type: DiskType::Unknown,
			encryption: None,
			file_system: FileSystem::Other("test".to_string()),
			total_capacity: 1000000000,
			available_space: 500000000,
			is_read_only: false,
			is_mounted: true,
			hardware_id: None,
			backend: None,
			apfs_container: None,
			container_volume_id: None,
			path_mappings: Vec::new(),
			is_user_visible: true,
			auto_track_eligible: false,
			read_speed_mbps: None,
			write_speed_mbps: None,
			created_at: now,
			updated_at: now,
			last_seen_at: now,
			total_files: None,
			total_directories: None,
			last_stats_update: None,
			display_name: Some("Test Volume".to_string()),
			is_favorite: false,
			color: None,
			icon: None,
			error_message: None,
		};

		let config = SpeedTestConfig {
			file_size_mb: 1, // Small test file
			timeout_secs: 10,
			iterations: 1,
		};

		let result = run_speed_test_with_config(&volume, config).await;
		assert!(result.is_ok());

		let (read_speed, write_speed) = result.unwrap();
		assert!(read_speed > 0);
		assert!(write_speed > 0);
	}
}
