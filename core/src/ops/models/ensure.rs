//! Model availability helpers - ensure models are downloaded before use

use super::{download::ModelDownloadJob, whisper::WhisperModel, whisper::WhisperModelManager};
use crate::infra::{
	event::Event,
	job::{prelude::*, types::JobId},
};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Ensure a whisper model is downloaded and ready to use
///
/// This function:
/// 1. Checks if the model already exists
/// 2. If not, dispatches a ModelDownloadJob and waits for completion
/// 3. Returns the path to the model file
///
/// The calling job can be paused/cancelled during the wait.
/// The download job continues in the background for reuse.
pub async fn ensure_whisper_model(
	ctx: &JobContext<'_>,
	model: WhisperModel,
	data_dir: &Path,
) -> JobResult<PathBuf> {
	let manager = WhisperModelManager::new(data_dir);
	let model_path = manager.get_model_path(&model);

	// Check if already downloaded
	if manager.is_downloaded(&model).await {
		debug!("Model {} already downloaded", model.display_name());
		return Ok(model_path);
	}

	// Model not found - need to download
	info!(
		"Model {} not found. Dispatching download job ({} MB)...",
		model.display_name(),
		model.size_bytes() / 1024 / 1024
	);

	ctx.log(format!(
		"Downloading model {} ({} MB)...",
		model.display_name(),
		model.size_bytes() / 1024 / 1024
	));

	// Create and dispatch download job
	let download_job = ModelDownloadJob::for_whisper_model(model.clone(), data_dir.to_path_buf());
	let handle = ctx
		.library()
		.jobs()
		.dispatch(download_job)
		.await
		.map_err(|e| JobError::execution(format!("Failed to dispatch download job: {}", e)))?;

	ctx.log(format!(
		"Model download started (job {}). Waiting for completion...",
		handle.id()
	));

	// Wait for download to complete
	wait_for_job_completion(ctx, &handle.id()).await?;

	ctx.log(format!("Model {} ready", model.display_name()));

	// Verify model now exists
	if !manager.is_downloaded(&model).await {
		return Err(JobError::execution(
			"Model download completed but file not found".to_string(),
		));
	}

	Ok(model_path)
}

/// Wait for a job to reach a terminal state (completed, failed, or cancelled)
///
/// This function subscribes to job events and waits for the specified job
/// to finish. The calling job can be interrupted during the wait.
async fn wait_for_job_completion(ctx: &JobContext<'_>, job_id: &JobId) -> JobResult<()> {
	let mut event_rx = ctx.library().event_bus().subscribe();
	let job_id_str = job_id.to_string();

	debug!("Waiting for job {} to complete", job_id_str);

	loop {
		match event_rx.recv().await {
			Ok(Event::JobCompleted { job_id, .. }) if job_id == job_id_str => {
				debug!("Job {} completed successfully", job_id);
				return Ok(());
			}
			Ok(Event::JobFailed { job_id, error, .. }) if job_id == job_id_str => {
				return Err(JobError::execution(format!(
					"Dependency job failed: {}",
					error
				)));
			}
			Ok(Event::JobCancelled { job_id, .. }) if job_id == job_id_str => {
				return Err(JobError::execution(
					"Dependency job was cancelled".to_string(),
				));
			}
			Err(e) => {
				// Event bus error - continue waiting
				debug!("Event bus error while waiting: {}", e);
			}
			_ => {
				// Other event - ignore and continue waiting
			}
		}
	}
}
