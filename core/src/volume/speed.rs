use super::error::VolumeError;
use super::types::{MountType, Volume, VolumeEvent};
use std::path::PathBuf;
use std::time::Instant;
use tokio::{
	fs::{File, OpenOptions},
	io::{AsyncReadExt, AsyncWriteExt},
	sync::broadcast::Sender,
	time::{timeout, Duration},
};
use tracing::{debug, error, instrument, trace};

/// Configuration for speed tests
#[derive(Debug, Clone)]
pub struct SpeedTestConfig {
	/// Size of the test file in megabytes
	pub file_size_mb: usize,
	/// Timeout for the test in seconds
	pub timeout_secs: u64,
	/// Whether to emit events during the test
	pub emit_events: bool,
}

impl Default for SpeedTestConfig {
	fn default() -> Self {
		Self {
			file_size_mb: 10,
			timeout_secs: 30,
			emit_events: true,
		}
	}
}

/// Result of a speed test
#[derive(Debug, Clone)]
pub struct SpeedTestResult {
	/// Write speed in MB/s
	pub write_speed: f64,
	/// Read speed in MB/s
	pub read_speed: f64,
	/// Time taken for the test in seconds
	pub duration: f64,
}

/// Trait for performing speed tests on volumes
#[async_trait::async_trait]
pub trait SpeedTest {
	/// Performs a speed test on the volume
	async fn speed_test(
		&mut self,
		config: Option<SpeedTestConfig>,
		event_tx: Option<&Sender<VolumeEvent>>,
	) -> Result<SpeedTestResult, VolumeError>;
}

/// Helper for managing temporary test files and directories
struct TestLocation {
	dir: PathBuf,
	file_path: PathBuf,
	created_dir: bool,
}

impl TestLocation {
	#[instrument(skip(volume_path, mount_type))]
	async fn new(volume_path: &PathBuf, mount_type: &MountType) -> Result<Self, VolumeError> {
		let (dir, created_dir) = get_writable_directory(volume_path, mount_type).await?;
		let file_path = dir.join("sd_speed_test_file.tmp");

		Ok(Self {
			dir,
			file_path,
			created_dir,
		})
	}

	async fn cleanup(&self) -> Result<(), VolumeError> {
		trace!("Cleaning up test file: {:?}", self.file_path);
		if let Err(e) = tokio::fs::remove_file(&self.file_path).await {
			error!("Failed to remove test file: {}", e);
		}

		if self.created_dir {
			trace!("Removing created directory: {:?}", self.dir);
			if let Err(e) = tokio::fs::remove_dir_all(&self.dir).await {
				error!("Failed to remove directory: {}", e);
			}
		}

		Ok(())
	}
}

#[async_trait::async_trait]
impl SpeedTest for Volume {
	#[instrument(skip(self, config, event_tx), fields(volume_name = %self.name))]
	async fn speed_test(
		&mut self,
		config: Option<SpeedTestConfig>,
		event_tx: Option<&Sender<VolumeEvent>>,
	) -> Result<SpeedTestResult, VolumeError> {
		let config = config.unwrap_or_default();

		// if volume is not mounted or not writable, return an error
		if !self.is_mounted || self.read_only {
			return Err(VolumeError::Cancelled);
		}

		debug!("Starting speed test with config: {:?}", config);

		let test_location = TestLocation::new(&self.mount_point, &self.mount_type).await?;
		let data = vec![0u8; config.file_size_mb * 1024 * 1024];
		let timeout_duration = Duration::from_secs(config.timeout_secs);

		// Perform write test
		let write_speed =
			perform_write_test(&test_location.file_path, &data, timeout_duration).await?;

		// Perform read test
		let read_speed =
			perform_read_test(&test_location.file_path, data.len(), timeout_duration).await?;

		let result = SpeedTestResult {
			write_speed,
			read_speed,
			duration: timeout_duration.as_secs_f64(),
		};

		// Update volume speeds
		self.read_speed_mbps = Some(read_speed as u64);
		self.write_speed_mbps = Some(write_speed as u64);

		// Emit event if requested
		// if config.emit_events {
		println!("emitting event for {:?}", self.fingerprint);
		if let Some(fingerprint) = self.fingerprint.clone() {
			if let Some(tx) = event_tx {
				let _ = tx.send(VolumeEvent::VolumeSpeedTested {
					fingerprint,
					read_speed: read_speed as u64,
					write_speed: write_speed as u64,
				});
			}
		}
		// }

		// Cleanup
		test_location.cleanup().await?;

		debug!("Speed test completed: {:?}", result);
		Ok(result)
	}
}

/// Helper function to get a writable directory within a volume
#[instrument(skip(volume_path, mount_type))]
async fn get_writable_directory(
	volume_path: &PathBuf,
	mount_type: &MountType,
) -> Result<(PathBuf, bool), VolumeError> {
	match mount_type {
		MountType::System => {
			trace!("Using system temp directory for system volume");
			Ok((std::env::temp_dir(), false))
		}
		_ => {
			let candidates = [
				volume_path.join("tmp"),
				volume_path.join("var").join("tmp"),
				volume_path.clone(),
			];

			for dir in &candidates {
				trace!("Checking directory: {:?}", dir);

				if let Ok(metadata) = tokio::fs::metadata(dir).await {
					if metadata.is_dir() {
						return Ok((dir.clone(), false));
					}
				}

				trace!("Attempting to create directory: {:?}", dir);
				if tokio::fs::create_dir_all(dir).await.is_ok() {
					return Ok((dir.clone(), true));
				}
			}

			Err(VolumeError::DirectoryError(
				"No writable directory found".to_string(),
			))
		}
	}
}

/// Performs the write speed test
#[instrument(skip(path, data))]
async fn perform_write_test(
	path: &PathBuf,
	data: &[u8],
	timeout_duration: Duration,
) -> Result<f64, VolumeError> {
	trace!("Starting write test");
	let start = Instant::now();

	let mut file = OpenOptions::new()
		.write(true)
		.create(true)
		.open(path)
		.await?;

	timeout(timeout_duration, file.write_all(data)).await??;

	let duration = start.elapsed();
	let speed = (data.len() as f64 / 1024.0 / 1024.0) / duration.as_secs_f64();

	trace!("Write test completed: {} MB/s", speed);
	Ok(speed)
}

/// Performs the read speed test
#[instrument(skip(path))]
async fn perform_read_test(
	path: &PathBuf,
	size: usize,
	timeout_duration: Duration,
) -> Result<f64, VolumeError> {
	trace!("Starting read test");
	let start = Instant::now();

	let mut file = File::open(path).await?;
	let mut buffer = vec![0u8; size];

	timeout(timeout_duration, file.read_exact(&mut buffer)).await??;

	let duration = start.elapsed();
	let speed = (size as f64 / 1024.0 / 1024.0) / duration.as_secs_f64();

	trace!("Read test completed: {} MB/s", speed);
	Ok(speed)
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	#[tokio::test]
	async fn test_speed_test() {
		let temp_dir = tempdir().unwrap();
		let mut volume = Volume::new(
			"test".to_string(),
			MountType::External,
			temp_dir.path().to_path_buf(),
			vec![],
			super::super::types::DiskType::Unknown,
			super::super::types::FileSystem::Other("test".to_string()),
			1000000,
			1000000,
			false,
		);

		let config = SpeedTestConfig {
			file_size_mb: 1,
			timeout_secs: 5,
			emit_events: false,
		};

		let result = volume.speed_test(Some(config), None).await.unwrap();

		assert!(result.read_speed > 0.0);
		assert!(result.write_speed > 0.0);
		assert_eq!(volume.read_speed_mbps, Some(result.read_speed as u64));
		assert_eq!(volume.write_speed_mbps, Some(result.write_speed as u64));
	}
}
