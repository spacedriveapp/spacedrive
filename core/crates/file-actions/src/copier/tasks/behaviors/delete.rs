use async_trait::async_trait;
use sd_core_job_system::job::JobError;
use std::path::Path;

/// Behavior for deleting files, can be implemented for different storage backends
#[async_trait]
pub trait DeleteBehavior: Send + Sync {
	/// Delete a file at the given path
	async fn delete_file(&self, path: impl AsRef<Path> + Send) -> Result<(), JobError>;

	/// Check if this behavior is suitable for the given path
	fn is_suitable(&self, path: impl AsRef<Path>) -> bool;
}

/// Local filesystem delete behavior
pub struct LocalDeleteBehavior;

#[async_trait]
impl DeleteBehavior for LocalDeleteBehavior {
	async fn delete_file(&self, path: impl AsRef<Path> + Send) -> Result<(), JobError> {
		tokio::fs::remove_file(path.as_ref())
			.await
			.map_err(|e| JobError::IO(e.into()))
	}

	fn is_suitable(&self, _path: impl AsRef<Path>) -> bool {
		// LocalDeleteBehavior is our fallback, so it's always suitable
		true
	}
}
