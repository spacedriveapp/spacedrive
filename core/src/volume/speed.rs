use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::time::error::Elapsed;
use tokio::time::{timeout, Duration};
use tracing::{error, trace};

use super::{MountType, Volume};

const TEST_TIMEOUT_SECS: u64 = 30;
const TEST_FILE_SIZE_MB: usize = 10; // Adjusted file size for testing

// Define the SpeedTest trait
#[async_trait::async_trait]
pub trait SpeedTest {
	async fn speed_test(&mut self) -> Result<(f64, f64), VolumeError>;
}

/// Custom error type for consistent error handling using ThisError
#[derive(Error, Debug)]
pub enum VolumeError {
	#[error("I/O error: {0}")]
	Io(#[from] io::Error),
	#[error("Timeout error: {0}")]
	Timeout(#[from] Elapsed),
	#[error("No mount point found for volume")]
	NoMountPoint,
	#[error("Directory error: {0}")]
	DirectoryError(String),
}

/// Helper function to get a writable directory within a volume and track if it was created.
async fn get_writable_directory(
	volume_path: &PathBuf,
	mount_type: &MountType,
) -> Result<(PathBuf, bool), VolumeError> {
	// For system volumes, use the system-wide temp directory
	if *mount_type == MountType::System {
		trace!("System volume detected, using system temp directory");
		return Ok((std::env::temp_dir(), false));
	}

	let writable_dirs = vec![
		volume_path.join("tmp"),             // Common temp folder in a volume
		volume_path.join("var").join("tmp"), // /var/tmp
	];

	// Try each directory and return the first one that is writable, along with whether it was created
	for dir in &writable_dirs {
		trace!("Checking directory: {:?}", dir);
		if tokio::fs::metadata(dir).await.is_ok() {
			trace!("Directory exists: {:?}", dir);
			return Ok((dir.clone(), false));
		}

		trace!("Directory does not exist, attempting to create: {:?}", dir);
		if tokio::fs::create_dir_all(dir).await.is_ok() {
			trace!("Created directory: {:?}", dir);
			return Ok((dir.clone(), true));
		} else {
			error!("Failed to create directory: {:?}", dir);
		}
	}

	error!("No writable directory found in the volume");
	Err(VolumeError::DirectoryError(
		"No writable directory found in the volume".to_string(),
	))
}

#[async_trait::async_trait]
impl SpeedTest for Volume {
	async fn speed_test(&mut self) -> Result<(f64, f64), VolumeError> {
		if self.mount_points.is_empty() {
			error!("No mount point found for volume: {}", self.name);
			return Err(VolumeError::NoMountPoint);
		}

		trace!("Starting speed test for volume: {}", self.name);

		let volume_path: &PathBuf = &self.mount_points[0];
		let (writable_dir, created_dir) =
			get_writable_directory(volume_path, &self.mount_type).await?;
		trace!("Using writable directory: {:?}", writable_dir);

		let test_file_path = writable_dir.join("sd_speed_test_file.tmp");

		// Prepare buffer for testing
		let data: Vec<u8> = vec![0u8; TEST_FILE_SIZE_MB * 1024 * 1024];

		// Write test
		let write_speed = {
			trace!("Starting write test...");
			let start = Instant::now();

			let mut file = OpenOptions::new()
				.write(true)
				.create(true)
				.open(&test_file_path)
				.await?;

			trace!("Opened file for writing: {:?}", test_file_path);
			timeout(
				Duration::from_secs(TEST_TIMEOUT_SECS),
				file.write_all(&data),
			)
			.await??;
			trace!("Write completed");

			let duration = start.elapsed();
			let speed = (TEST_FILE_SIZE_MB as f64) / duration.as_secs_f64();
			trace!("Write speed: {} MB/s", speed);
			speed
		};

		// Read test
		let read_speed = {
			trace!("Starting read test...");
			let start = Instant::now();

			let mut file = File::open(&test_file_path).await?;
			trace!("Opened file for reading: {:?}", test_file_path);

			let mut buffer = vec![0u8; TEST_FILE_SIZE_MB * 1024 * 1024];
			timeout(
				Duration::from_secs(TEST_TIMEOUT_SECS),
				file.read_exact(&mut buffer),
			)
			.await??;
			trace!("Read completed");

			let duration = start.elapsed();
			let speed = (TEST_FILE_SIZE_MB as f64) / duration.as_secs_f64();
			trace!("Read speed: {} MB/s", speed);
			speed
		};

		// Cleanup
		trace!("Cleaning up test file: {:?}", test_file_path);
		tokio::fs::remove_file(&test_file_path).await?;

		if created_dir {
			trace!("Removing created directory: {:?}", writable_dir);
			let _ = tokio::fs::remove_dir_all(&writable_dir).await;
		}

		// Update volume with speeds
		self.read_speed_mbps = Some(read_speed as u64);
		self.write_speed_mbps = Some(write_speed as u64);
		trace!("Speed test completed for volume: {}", self.name);

		Ok((write_speed, read_speed))
	}
}
