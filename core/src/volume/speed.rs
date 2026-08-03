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

	let mut test_location = TestLocation::new(&volume.mount_point, &volume.mount_type).await?;
	let result = perform_speed_test(&mut test_location, &config).await;

	test_location.cleanup().await?;
	let result = result?;

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
	test_file_created: bool,
	created_dir: Option<std::path::PathBuf>,
}

impl TestLocation {
	/// Create a new test location
	async fn new(volume_path: &std::path::Path, mount_type: &MountType) -> VolumeResult<Self> {
		let (dir, created_dir) = get_writable_directory(volume_path, mount_type).await?;
		let test_file = dir.join(format!(
			".spacedrive_speed_test-{}.tmp",
			uuid::Uuid::new_v4()
		));

		Ok(Self {
			test_file,
			test_file_created: false,
			created_dir,
		})
	}

	/// Clean up test files and directories
	async fn cleanup(&mut self) -> VolumeResult<()> {
		let mut can_remove_created_dir = false;

		// Never remove a file unless this speed test successfully created it.
		if self.test_file_created {
			match tokio::fs::remove_file(&self.test_file).await {
				Ok(()) => {
					self.test_file_created = false;
					can_remove_created_dir = true;
				}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
					self.test_file_created = false;
				}
				Err(e) => {
					warn!("Failed to remove test file: {}", e);
				}
			}
		}

		// The owned test file also acts as proof that the directory was not
		// removed and replaced while the test was running. Even then, only
		// remove an empty directory so concurrently added files are preserved.
		if can_remove_created_dir {
			if let Some(ref dir) = self.created_dir {
				if let Err(e) = tokio::fs::remove_dir(dir).await {
					warn!("Failed to remove test directory: {}", e);
				}
			}
		}

		Ok(())
	}
}

/// Perform the actual speed test
async fn perform_speed_test(
	location: &mut TestLocation,
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
		let write_speed = timeout(timeout_duration, perform_write_test(location, &test_data))
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
			match tokio::fs::remove_file(&location.test_file).await {
				Ok(()) => location.test_file_created = false,
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
					location.test_file_created = false;
				}
				Err(error) => return Err(error.into()),
			}
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
async fn perform_write_test(location: &mut TestLocation, data: &[u8]) -> VolumeResult<f64> {
	let start = Instant::now();

	let mut file = OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&location.test_file)
		.await?;
	location.test_file_created = true;

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
				let created_dir = match tokio::fs::create_dir(candidate).await {
					Ok(()) => Some(candidate.clone()),
					Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
					Err(_) => continue,
				};

				let permission_file = candidate.join(format!(
					".spacedrive_write_test-{}.tmp",
					uuid::Uuid::new_v4()
				));
				let is_writable = match OpenOptions::new()
					.write(true)
					.create_new(true)
					.open(&permission_file)
					.await
				{
					Ok(mut file) => {
						let result = file.write_all(b"test").await;
						drop(file);
						let _ = tokio::fs::remove_file(&permission_file).await;
						result.is_ok()
					}
					Err(_) => false,
				};

				if is_writable {
					return Ok((candidate.clone(), created_dir));
				}

				if let Some(dir) = created_dir {
					let _ = tokio::fs::remove_dir(dir).await;
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
			let _ = tokio::fs::remove_dir(dir).await;
		}
	}

	#[tokio::test]
	async fn test_cleanup_preserves_existing_tmp_directory() {
		let volume = TempDir::new().unwrap();
		let existing_tmp = volume.path().join("tmp");
		tokio::fs::create_dir(&existing_tmp).await.unwrap();

		let sentinel = existing_tmp.join("user-data.txt");
		tokio::fs::write(&sentinel, b"keep me").await.unwrap();

		let mut location = TestLocation::new(volume.path(), &MountType::External)
			.await
			.unwrap();
		assert!(location.created_dir.is_none());

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		location.cleanup().await.unwrap();

		assert_eq!(tokio::fs::read(&sentinel).await.unwrap(), b"keep me");
		assert!(!location.test_file.exists());
	}

	#[tokio::test]
	async fn test_cleanup_preserves_existing_empty_tmp_directory() {
		let volume = TempDir::new().unwrap();
		let existing_tmp = volume.path().join("tmp");
		tokio::fs::create_dir(&existing_tmp).await.unwrap();

		let mut location = TestLocation::new(volume.path(), &MountType::External)
			.await
			.unwrap();
		assert!(location.created_dir.is_none());

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		location.cleanup().await.unwrap();

		assert!(existing_tmp.is_dir());
	}

	#[tokio::test]
	async fn test_cleanup_preserves_files_added_to_created_directory() {
		let volume = TempDir::new().unwrap();
		let mut location = TestLocation::new(volume.path(), &MountType::External)
			.await
			.unwrap();
		let created_dir = location.created_dir.clone().unwrap();

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		let sentinel = created_dir.join("created-during-test.txt");
		tokio::fs::write(&sentinel, b"keep me").await.unwrap();

		location.cleanup().await.unwrap();

		assert_eq!(tokio::fs::read(&sentinel).await.unwrap(), b"keep me");
		assert!(!location.test_file.exists());
	}

	#[tokio::test]
	async fn test_cleanup_preserves_replaced_directory() {
		let volume = TempDir::new().unwrap();
		let mut location = TestLocation::new(volume.path(), &MountType::External)
			.await
			.unwrap();
		let created_dir = location.created_dir.clone().unwrap();

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		tokio::fs::remove_dir_all(&created_dir).await.unwrap();
		tokio::fs::create_dir(&created_dir).await.unwrap();

		location.cleanup().await.unwrap();

		assert!(created_dir.is_dir());
	}

	#[tokio::test]
	async fn test_write_does_not_overwrite_or_delete_existing_file() {
		let volume = TempDir::new().unwrap();
		let existing_file = volume.path().join("existing.tmp");
		tokio::fs::write(&existing_file, b"user data")
			.await
			.unwrap();

		let mut location = TestLocation {
			test_file: existing_file.clone(),
			test_file_created: false,
			created_dir: None,
		};

		assert!(perform_write_test(&mut location, b"speed test")
			.await
			.is_err());
		location.cleanup().await.unwrap();

		assert_eq!(tokio::fs::read(existing_file).await.unwrap(), b"user data");
	}

	#[tokio::test]
	async fn test_concurrent_locations_use_different_files() {
		let volume = TempDir::new().unwrap();
		let (first, second) = tokio::join!(
			TestLocation::new(volume.path(), &MountType::External),
			TestLocation::new(volume.path(), &MountType::External)
		);
		let mut first = first.unwrap();
		let mut second = second.unwrap();

		assert_ne!(first.test_file, second.test_file);
		let (first_write, second_write) = tokio::join!(
			perform_write_test(&mut first, b"first"),
			perform_write_test(&mut second, b"second")
		);
		assert!(first_write.is_ok());
		assert!(second_write.is_ok());

		let (first_cleanup, second_cleanup) = tokio::join!(first.cleanup(), second.cleanup());
		first_cleanup.unwrap();
		second_cleanup.unwrap();
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

		let device_id = uuid::Uuid::new_v4();
		let mount_path = temp_dir.path().to_path_buf();
		let fingerprint = VolumeFingerprint::from_primary_volume(&mount_path, device_id);
		let now = chrono::Utc::now();

		let volume = Volume {
			id: uuid::Uuid::new_v4(),
			fingerprint,
			cloud_identifier: None,
			cloud_config: None,
			device_id,
			name: "Test Volume".to_string(),
			library_id: None,
			is_tracked: false,
			mount_point: mount_path.clone(),
			mount_points: vec![mount_path.clone()],
			volume_type: VolumeType::External,
			mount_type: MountType::External,
			disk_type: DiskType::Unknown,
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
			supports_block_cloning: false,
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
		assert!(!mount_path.join("tmp").exists());
	}
}
