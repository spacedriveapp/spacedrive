use async_trait::async_trait;
use blake3::Hasher;
use sd_core_heavy_lifting::job_system::job::{JobContext, JobError};
use std::{path::Path, time::Instant};
use tokio::{
	fs,
	io::{AsyncReadExt, AsyncWriteExt},
};

use super::{delete::DeleteBehavior, utils::PROGRESS_UPDATE_INTERVAL, CopyBehavior, MAX_RETRIES};
use crate::copier::progress::CopyProgress;

const BLOCK_LEN: usize = 1048576; // 1MB blocks for hashing

/// Stream copy with progress reporting, suitable for remote files or when progress tracking is needed
pub struct StreamCopyBehavior<D: DeleteBehavior> {
	buffer_size: usize,
	max_retries: u32,
	delete_on_success: bool,
	verify_hash: bool,
	delete_behavior: D,
}

impl<D: DeleteBehavior> StreamCopyBehavior<D> {
	pub fn new(
		buffer_size: usize,
		max_retries: u32,
		delete_on_success: bool,
		verify_hash: bool,
		delete_behavior: D,
	) -> Self {
		Self {
			buffer_size,
			max_retries,
			delete_on_success,
			verify_hash,
			delete_behavior,
		}
	}

	pub fn default(delete_behavior: D) -> Self {
		Self::new(8192, MAX_RETRIES, false, true, delete_behavior) // 8KB default buffer, hash verification on by default
	}

	async fn compute_file_hash(path: &Path) -> Result<String, JobError> {
		let mut file = fs::File::open(path)
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		let mut context = Hasher::new();
		let mut buffer = vec![0; BLOCK_LEN].into_boxed_slice();

		loop {
			let read_count = file
				.read(&mut buffer)
				.await
				.map_err(|e| JobError::IO(e.into()))?;

			context.update(&buffer[..read_count]);
			if read_count != BLOCK_LEN {
				break;
			}
		}

		Ok(context.finalize().to_hex().to_string())
	}

	async fn try_copy_file(
		&self,
		source: &Path,
		target: &Path,
		ctx: &impl JobContext,
		start: Instant,
		last_progress: &mut Instant,
	) -> Result<(), JobError> {
		let mut source_file = fs::File::open(source)
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		let mut target_file = fs::File::create(target)
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		let total_size = source_file
			.metadata()
			.await
			.map_err(|e| JobError::IO(e.into()))?
			.len();

		let file_name = source
			.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();

		let mut buffer = vec![0; self.buffer_size];
		let mut bytes_copied = 0;
		let mut hasher = if self.verify_hash {
			Some(Hasher::new())
		} else {
			None
		};

		loop {
			let n = source_file
				.read(&mut buffer)
				.await
				.map_err(|e| JobError::IO(e.into()))?;

			if n == 0 {
				break;
			}

			if let Some(ref mut hasher) = hasher {
				hasher.update(&buffer[..n]);
			}

			target_file
				.write_all(&buffer[..n])
				.await
				.map_err(|e| JobError::IO(e.into()))?;

			bytes_copied += n as u64;

			let now = Instant::now();
			if now.duration_since(*last_progress) >= PROGRESS_UPDATE_INTERVAL {
				let elapsed = now.duration_since(start);
				let bytes_per_sec = if elapsed.as_secs() > 0 {
					bytes_copied / elapsed.as_secs()
				} else {
					bytes_copied
				};

				let remaining_bytes = total_size - bytes_copied;
				let eta = if bytes_per_sec > 0 {
					elapsed.mul_f32(remaining_bytes as f32 / bytes_copied as f32)
				} else {
					elapsed.mul_f32(0.0)
				};

				ctx.progress(CopyProgress::FileProgress {
					name: file_name.clone(),
					bytes_copied,
					total_bytes: total_size,
					speed_bytes_per_sec: bytes_per_sec,
					eta,
				})
				.await;

				*last_progress = now;
			}
		}

		// Ensure all data is written to disk
		target_file
			.sync_all()
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		// If hash verification is enabled, verify the target file
		if self.verify_hash {
			let source_hash = Self::compute_file_hash(source).await?;
			let target_hash = Self::compute_file_hash(target).await?;

			if source_hash != target_hash {
				return Err(JobError::IO(
					std::io::Error::new(
						std::io::ErrorKind::Other,
						"Hash verification failed - source and target files do not match",
					)
					.into(),
				));
			}

			// TODO: If the file is in the index and doesn't have a hash, persist the hash
			// This requires access to the database context which we don't have here
			// Consider adding this as a callback or moving it to a higher level
		}

		Ok(())
	}
}

#[async_trait]
impl<D: DeleteBehavior> CopyBehavior for StreamCopyBehavior<D> {
	async fn copy_file(
		&self,
		source: impl AsRef<Path> + Send,
		target: impl AsRef<Path> + Send,
		ctx: &impl JobContext,
	) -> Result<(), JobError> {
		let mut retries = self.max_retries;
		let start = Instant::now();
		let mut last_progress = start;

		loop {
			match self
				.try_copy_file(
					source.as_ref(),
					target.as_ref(),
					ctx,
					start,
					&mut last_progress,
				)
				.await
			{
				Ok(()) => {
					// If copy and verification succeeded and delete_on_success is true, delete the source
					if self.delete_on_success && self.delete_behavior.is_suitable(source.as_ref()) {
						if let Err(e) = self.delete_behavior.delete_file(source.as_ref()).await {
							ctx.progress(CopyProgress::Error {
								file: source.as_ref().to_string_lossy().to_string(),
								error: format!(
									"Failed to delete source file after successful copy: {}",
									e
								),
								retries_remaining: 0,
							})
							.await;
							// Don't return error here as the copy itself was successful
						}
					}
					break;
				}
				Err(e) => {
					if retries == 0 {
						ctx.progress(CopyProgress::Error {
							file: source.as_ref().to_string_lossy().to_string(),
							error: e.to_string(),
							retries_remaining: 0,
						})
						.await;
						return Err(e);
					}

					retries -= 1;
					ctx.progress(CopyProgress::Error {
						file: source.as_ref().to_string_lossy().to_string(),
						error: e.to_string(),
						retries_remaining: retries,
					})
					.await;

					tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
				}
			}
		}

		Ok(())
	}

	fn is_suitable(&self, _source: impl AsRef<Path>, _target: impl AsRef<Path>) -> bool {
		// StreamCopyBehavior is our fallback, so it's always suitable
		true
	}
}
