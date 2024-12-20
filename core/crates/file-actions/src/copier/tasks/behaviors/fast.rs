use async_trait::async_trait;
use sd_core_job_system::job::{JobContext, JobError};
use std::{path::Path, time::Instant};
use tokio::fs;

use super::{utils::is_same_filesystem, CopyBehavior, FAST_COPY_SIZE_THRESHOLD};
use crate::copier::progress::CopyProgress;

/// Fast copy using fs::copy, suitable for local files
#[derive(Default)]
pub struct FastCopyBehavior;

#[async_trait]
impl CopyBehavior for FastCopyBehavior {
	async fn copy_file(
		&self,
		source: impl AsRef<Path> + Send,
		target: impl AsRef<Path> + Send,
		ctx: &impl JobContext,
	) -> Result<(), JobError> {
		let start = Instant::now();
		let metadata = fs::metadata(&source)
			.await
			.map_err(|e| JobError::IO(e.into()))?;
		let total_bytes = metadata.len();
		let file_name = source
			.as_ref()
			.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();

		fs::copy(&source, &target)
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		let duration = start.elapsed();
		let speed = if duration.as_secs() > 0 {
			total_bytes / duration.as_secs()
		} else {
			total_bytes
		};

		ctx.progress(CopyProgress::FileProgress {
			name: file_name,
			bytes_copied: total_bytes,
			total_bytes,
			speed_bytes_per_sec: speed,
			eta: duration.mul_f32(0.0), // No ETA needed since we're done
		})
		.await;

		Ok(())
	}

	fn is_suitable(&self, source: impl AsRef<Path>, target: impl AsRef<Path>) -> bool {
		// Check if both paths are on the same filesystem and file is small enough
		if let Ok(metadata) = std::fs::metadata(source.as_ref()) {
			metadata.len() <= FAST_COPY_SIZE_THRESHOLD && is_same_filesystem(source, target)
		} else {
			false
		}
	}
}
