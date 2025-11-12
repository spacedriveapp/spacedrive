//! Model download job with progress tracking

use super::{types::ModelInfo, whisper::WhisperModel};
use crate::infra::job::{prelude::*, traits::DynJob};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelDownloadConfig {
	/// Model ID to download (e.g., "whisper-base")
	pub model_id: String,
	/// Data directory for model storage
	pub data_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelDownloadState {
	phase: DownloadPhase,
	model_id: String,
	download_url: String,
	target_path: PathBuf,
	temp_path: PathBuf,
	total_bytes: u64,
	downloaded_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DownloadPhase {
	Initializing,
	Downloading,
	Verifying,
	Complete,
}

#[derive(Serialize, Deserialize)]
pub struct ModelDownloadJob {
	config: ModelDownloadConfig,
	state: ModelDownloadState,
}

impl ModelDownloadJob {
	pub fn new(config: ModelDownloadConfig) -> Self {
		Self {
			state: ModelDownloadState {
				phase: DownloadPhase::Initializing,
				model_id: config.model_id.clone(),
				download_url: String::new(),
				target_path: PathBuf::new(),
				temp_path: PathBuf::new(),
				total_bytes: 0,
				downloaded_bytes: 0,
			},
			config,
		}
	}

	pub fn for_whisper_model(model: WhisperModel, data_dir: PathBuf) -> Self {
		Self::new(ModelDownloadConfig {
			model_id: model.id().to_string(),
			data_dir,
		})
	}
}

impl Job for ModelDownloadJob {
	const NAME: &'static str = "model_download";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Download AI/ML models");
}

#[async_trait::async_trait]
impl JobHandler for ModelDownloadJob {
	type Output = ModelDownloadOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		match self.state.phase {
			DownloadPhase::Initializing => {
				self.initialize(&ctx).await?;
				self.state.phase = DownloadPhase::Downloading;
			}
			DownloadPhase::Downloading => {}
			DownloadPhase::Verifying => {}
			DownloadPhase::Complete => {
				return Ok(ModelDownloadOutput {
					model_id: self.state.model_id.clone(),
					path: self.state.target_path.to_string_lossy().to_string(),
					size_bytes: self.state.total_bytes,
				});
			}
		}

		// Download phase
		if matches!(self.state.phase, DownloadPhase::Downloading) {
			self.download(&ctx).await?;
			self.state.phase = DownloadPhase::Verifying;
		}

		// Verify phase
		if matches!(self.state.phase, DownloadPhase::Verifying) {
			self.verify(&ctx).await?;
			self.state.phase = DownloadPhase::Complete;
		}

		ctx.log("Model download complete");

		Ok(ModelDownloadOutput {
			model_id: self.state.model_id.clone(),
			path: self.state.target_path.to_string_lossy().to_string(),
			size_bytes: self.state.total_bytes,
		})
	}
}

impl ModelDownloadJob {
	async fn initialize(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		ctx.log(format!("Initializing download for model: {}", self.config.model_id));

		// Parse model ID and get download info
		if let Some(model) = WhisperModel::from_str(&self.config.model_id.replace("whisper-", "")) {
			let models_dir = super::get_whisper_models_dir(&self.config.data_dir);
			tokio::fs::create_dir_all(&models_dir).await?;

			self.state.download_url = model.download_url().to_string();
			self.state.target_path = models_dir.join(model.filename());
			self.state.temp_path = self.state.target_path.with_extension("tmp");
			self.state.total_bytes = model.size_bytes();

			ctx.log(format!(
				"Downloading {} ({} MB) from Hugging Face",
				model.display_name(),
				self.state.total_bytes / 1024 / 1024
			));
		} else {
			return Err(JobError::execution(format!(
				"Unknown model ID: {}",
				self.config.model_id
			)));
		}

		Ok(())
	}

	async fn download(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use futures::StreamExt;

		ctx.log("Starting download...");

		// Start download
		let client = reqwest::Client::new();
		let response = client
			.get(&self.state.download_url)
			.send()
			.await
			.map_err(|e| JobError::execution(format!("Download request failed: {}", e)))?;

		if !response.status().is_success() {
			return Err(JobError::execution(format!(
				"Download failed with status: {}",
				response.status()
			)));
		}

		// Verify content length
		if let Some(content_length) = response.content_length() {
			self.state.total_bytes = content_length;
		}

		// Create temp file
		let mut file = tokio::fs::File::create(&self.state.temp_path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to create temp file: {}", e)))?;

		// Stream download with progress
		let mut stream = response.bytes_stream();
		let mut last_checkpoint = 0u64;

		while let Some(chunk) = stream.next().await {
			ctx.check_interrupt().await?;

			let chunk = chunk.map_err(|e| JobError::execution(format!("Download error: {}", e)))?;

			file.write_all(&chunk)
				.await
				.map_err(|e| JobError::execution(format!("Write error: {}", e)))?;

			self.state.downloaded_bytes += chunk.len() as u64;

			// Report progress
			ctx.progress(Progress::Bytes {
				current: self.state.downloaded_bytes,
				total: self.state.total_bytes,
			});

			// Checkpoint every 10 MB
			if self.state.downloaded_bytes - last_checkpoint > 10 * 1024 * 1024 {
				ctx.checkpoint().await?;
				last_checkpoint = self.state.downloaded_bytes;
				let progress_pct = (self.state.downloaded_bytes as f64 / self.state.total_bytes as f64) * 100.0;
				debug!(
					"Download progress: {:.1}% ({} MB / {} MB)",
					progress_pct,
					self.state.downloaded_bytes / 1024 / 1024,
					self.state.total_bytes / 1024 / 1024
				);
			}
		}

		file.flush().await?;
		drop(file);

		ctx.log(format!(
			"Downloaded {} MB",
			self.state.downloaded_bytes / 1024 / 1024
		));

		Ok(())
	}

	async fn verify(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		ctx.log("Verifying download...");

		// Check file size
		let metadata = tokio::fs::metadata(&self.state.temp_path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to read temp file: {}", e)))?;

		if metadata.len() != self.state.total_bytes {
			return Err(JobError::execution(format!(
				"Downloaded file size mismatch: expected {} bytes, got {} bytes",
				self.state.total_bytes,
				metadata.len()
			)));
		}

		// Move to final location
		tokio::fs::rename(&self.state.temp_path, &self.state.target_path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to move file: {}", e)))?;

		ctx.log("Verification complete");

		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelDownloadOutput {
	pub model_id: String,
	pub path: String,
	pub size_bytes: u64,
}

impl From<ModelDownloadOutput> for JobOutput {
	fn from(output: ModelDownloadOutput) -> Self {
		JobOutput::Custom(serde_json::json!({
			"type": "model_download",
			"model_id": output.model_id,
			"path": output.path,
			"size_mb": output.size_bytes / 1024 / 1024,
		}))
	}
}

impl DynJob for ModelDownloadJob {
	fn job_name(&self) -> &'static str {
		"Model Download"
	}
}

impl From<ModelDownloadJob> for Box<dyn DynJob> {
	fn from(job: ModelDownloadJob) -> Self {
		Box::new(job)
	}
}
