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

	test_location.cleanup().await;
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

async fn remove_file_best_effort(path: &std::path::Path, artifact: &'static str) -> bool {
	match tokio::fs::remove_file(path).await {
		Ok(()) => true,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
		Err(error) => {
			warn!(
				error = %error,
				path = %path.display(),
				artifact = artifact,
				"Failed to remove speed test artifact"
			);
			false
		}
	}
}

/// Helper for managing test files
struct TestLocation {
	test_file: std::path::PathBuf,
	test_file_created: bool,
}

impl TestLocation {
	/// Create a new test location
	async fn new(volume_path: &std::path::Path, mount_type: &MountType) -> VolumeResult<Self> {
		let dir = get_writable_directory(volume_path, mount_type).await?;
		let test_file = dir.join(format!(
			".spacedrive_speed_test-{}.tmp",
			uuid::Uuid::new_v4()
		));

		Ok(Self {
			test_file,
			test_file_created: false,
		})
	}

	/// Clean up the test file
	async fn cleanup(&mut self) {
		// Never remove a file unless this speed test successfully created it.
		if self.test_file_created
			&& remove_file_best_effort(&self.test_file, "speed test file").await
		{
			self.test_file_created = false;
		}
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
) -> VolumeResult<std::path::PathBuf> {
	match mount_type {
		MountType::System => {
			// For system volumes, prefer using temp directory
			Ok(std::env::temp_dir())
		}
		_ => {
			// Only use directories that already exist. A speed test must never
			// create or delete a root-level directory on the volume.
			let candidates = [
				volume_path.join("tmp"),
				volume_path.join(".spacedrive_temp"),
				volume_path.to_path_buf(),
			];

			for candidate in &candidates {
				if !matches!(tokio::fs::metadata(candidate).await, Ok(metadata) if metadata.is_dir())
				{
					continue;
				}

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
						let write_result = file.write_all(b"test").await;
						drop(file);
						let cleanup_succeeded = remove_file_best_effort(
							&permission_file,
							"speed test permission probe",
						)
						.await;
						write_result.is_ok() && cleanup_succeeded
					}
					Err(_) => false,
				};

				if is_writable {
					return Ok(candidate.clone());
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

		let writable_dir = get_writable_directory(volume_path, &MountType::External)
			.await
			.unwrap();

		assert_eq!(writable_dir, volume_path);
		assert!(!volume_path.join("tmp").exists());
		assert!(!volume_path.join(".spacedrive_temp").exists());
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

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		location.cleanup().await;

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

		perform_write_test(&mut location, b"speed test")
			.await
			.unwrap();
		location.cleanup().await;

		assert!(existing_tmp.is_dir());
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
		};

		assert!(perform_write_test(&mut location, b"speed test")
			.await
			.is_err());
		location.cleanup().await;

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

		let first_file = first.test_file.clone();
		let second_file = second.test_file.clone();
		tokio::join!(first.cleanup(), second.cleanup());

		assert!(!first_file.exists());
		assert!(!second_file.exists());
		assert!(!volume.path().join("tmp").exists());
		assert!(!volume.path().join(".spacedrive_temp").exists());
	}

	#[tokio::test]
	async fn test_writable_directory_system() {
		let writable_dir =
			get_writable_directory(&std::path::PathBuf::from("/"), &MountType::System)
				.await
				.unwrap();

		assert!(writable_dir.exists());
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
