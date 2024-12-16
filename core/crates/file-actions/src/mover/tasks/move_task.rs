use std::path::PathBuf;

use async_trait::async_trait;
use heavy_lifting::{job::JobError, task::Task};
use tokio::fs;

#[derive(Debug)]
pub struct MoveTask {
	source: PathBuf,
	target: PathBuf,
}

impl MoveTask {
	pub fn new(source: PathBuf, target: PathBuf) -> Self {
		Self { source, target }
	}
}

#[async_trait]
impl Task for MoveTask {
	type Error = JobError;

	fn name(&self) -> &'static str {
		"move"
	}

	async fn run(&self) -> Result<(), JobError> {
		// Check if target exists
		if self.target.exists() {
			return Err(JobError::InvalidInput(format!(
				"Target path {} already exists",
				self.target.display()
			)));
		}

		// Perform the move operation
		fs::rename(&self.source, &self.target)
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		Ok(())
	}
}
