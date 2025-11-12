//! Speech-to-text job for batch audio transcription

use super::processor::SpeechToTextProcessor;
use crate::{
	infra::{db::entities::entry, job::{prelude::*, traits::DynJob}},
	ops::indexing::processor::ProcessorEntry,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpeechToTextJobConfig {
	/// Location ID to process (None = all entries in library)
	pub location_id: Option<Uuid>,
	/// Single entry UUID to process (for UI-triggered single file)
	pub entry_uuid: Option<Uuid>,
	/// Whisper model to use (tiny, base, small, medium, large)
	pub model: String,
	/// Language code (None = auto-detect)
	pub language: Option<String>,
	/// Reprocess files that already have subtitles
	pub reprocess: bool,
}

impl Default for SpeechToTextJobConfig {
	fn default() -> Self {
		Self {
			location_id: None,
			entry_uuid: None,
			model: "base".to_string(),
			language: None,
			reprocess: false,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpeechJobState {
	phase: SpeechPhase,
	entries: Vec<(i32, std::path::PathBuf, Option<String>)>,
	processed: usize,
	success_count: usize,
	error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SpeechPhase {
	Discovery,
	Processing,
	Complete,
}

#[derive(Serialize, Deserialize)]
pub struct SpeechToTextJob {
	config: SpeechToTextJobConfig,
	state: SpeechJobState,
}

impl SpeechToTextJob {
	pub fn new(config: SpeechToTextJobConfig) -> Self {
		Self {
			config,
			state: SpeechJobState {
				phase: SpeechPhase::Discovery,
				entries: Vec::new(),
				processed: 0,
				success_count: 0,
				error_count: 0,
			},
		}
	}

	pub fn from_location(location_id: Uuid) -> Self {
		Self::new(SpeechToTextJobConfig {
			location_id: Some(location_id),
			..Default::default()
		})
	}
}

impl Job for SpeechToTextJob {
	const NAME: &'static str = "speech_to_text";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Transcribe audio/video to text subtitles");
}

#[async_trait::async_trait]
impl JobHandler for SpeechToTextJob {
	type Output = SpeechToTextJobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		match self.state.phase {
			SpeechPhase::Discovery => {
				ctx.log("Starting speech-to-text discovery phase");
				self.run_discovery(&ctx).await?;
				self.state.phase = SpeechPhase::Processing;
			}
			SpeechPhase::Processing => {}
			SpeechPhase::Complete => {
				return Ok(SpeechToTextJobOutput {
					total_processed: self.state.processed,
					success_count: self.state.success_count,
					error_count: self.state.error_count,
				});
			}
		}

		ctx.log(format!(
			"Speech-to-text processing {} entries",
			self.state.entries.len()
		));

		let processor = SpeechToTextProcessor::new(ctx.library_arc())
			.with_model(self.config.model.clone())
			.with_language(self.config.language.clone());

		let total = self.state.entries.len();

		while self.state.processed < total {
			ctx.check_interrupt().await?;

			let (entry_id, path, mime_type) = &self.state.entries[self.state.processed];

			// Load entry to get content_id
			let entry_model = entry::Entity::find_by_id(*entry_id)
				.one(ctx.library_db())
				.await?
				.ok_or_else(|| JobError::execution("Entry not found"))?;

			let proc_entry = ProcessorEntry {
				id: *entry_id,
				uuid: entry_model.uuid,
				path: path.clone(),
				kind: crate::ops::indexing::state::EntryKind::File,
				size: entry_model.size as u64,
				content_id: entry_model.content_id,
				mime_type: mime_type.clone(),
			};

			if !processor.should_process(&proc_entry) {
				self.state.processed += 1;
				continue;
			}

			match processor.process(ctx.library_db(), &proc_entry).await {
				Ok(result) if result.success => {
					ctx.log(format!(
						"Transcribed {}: {} bytes",
						path.display(),
						result.bytes_processed
					));
					self.state.success_count += 1;
				}
				Ok(_) => {
					warn!("Transcription failed for {}", path.display());
					self.state.error_count += 1;
				}
				Err(e) => {
					ctx.log(format!(
						"ERROR: Transcription error for {}: {}",
						path.display(),
						e
					));
					self.state.error_count += 1;
				}
			}

			self.state.processed += 1;

			// Report progress
			ctx.progress(Progress::Count {
				current: self.state.processed,
				total,
			});

			if self.state.processed % 5 == 0 {
				ctx.checkpoint().await?;
			}
		}

		self.state.phase = SpeechPhase::Complete;
		ctx.log(format!(
			"Speech-to-text complete: {} success, {} errors",
			self.state.success_count, self.state.error_count
		));

		Ok(SpeechToTextJobOutput {
			total_processed: self.state.processed,
			success_count: self.state.success_count,
			error_count: self.state.error_count,
		})
	}
}

impl SpeechToTextJob {
	async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use crate::infra::db::entities::{content_identity, entry, mime_type};

		ctx.log("Starting speech-to-text discovery");

		// Ensure whisper model is downloaded FIRST
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| JobError::execution(format!("Failed to get data dir: {}", e)))?;

		let model = crate::ops::models::WhisperModel::from_str(&self.config.model)
			.ok_or_else(|| JobError::execution(format!("Invalid model: {}", self.config.model)))?;

		let _model_path = crate::ops::models::ensure_whisper_model(ctx, model, &data_dir).await?;

		ctx.log("Model ready, discovering files...");

		let db = ctx.library_db();

		// Check if this is single-file mode (from UI action)
		if let Some(entry_uuid) = self.config.entry_uuid {
			ctx.log(format!("Single file mode: processing entry {}", entry_uuid));

			// Load the specific entry
			let entry_model = entry::Entity::find()
				.filter(entry::Column::Uuid.eq(entry_uuid))
				.one(db)
				.await?
				.ok_or_else(|| JobError::execution("Entry not found"))?;

			if let Some(content_id) = entry_model.content_id {
				if let Ok(Some(ci)) = content_identity::Entity::find_by_id(content_id)
					.one(db)
					.await
				{
					if let Some(mime_id) = ci.mime_type_id {
						if let Ok(Some(mime)) = mime_type::Entity::find_by_id(mime_id).one(db).await {
							if super::is_speech_supported(&mime.mime_type) {
								if let Ok(path) =
									crate::ops::indexing::PathResolver::get_full_path(db, entry_model.id).await
								{
									self.state.entries.push((entry_model.id, path, Some(mime.mime_type)));
								}
							}
						}
					}
				}
			}

			ctx.log(format!("Single file discovered: {} entries", self.state.entries.len()));
			return Ok(());
		}

		// Batch mode - discover all eligible entries
		let entries = entry::Entity::find()
			.filter(entry::Column::ContentId.is_not_null())
			.all(db)
			.await?;

		ctx.log(format!("Found {} entries with content", entries.len()));

		for entry_model in entries {
			if let Some(content_id) = entry_model.content_id {
				if let Ok(Some(ci)) = content_identity::Entity::find_by_id(content_id)
					.one(db)
					.await
				{
					if let Some(mime_id) = ci.mime_type_id {
						if let Ok(Some(mime)) = mime_type::Entity::find_by_id(mime_id).one(db).await
						{
							if super::is_speech_supported(&mime.mime_type) {
								if let Ok(path) = crate::ops::indexing::PathResolver::get_full_path(
									db,
									entry_model.id,
								)
								.await
								{
									self.state.entries.push((
										entry_model.id,
										path,
										Some(mime.mime_type),
									));
								}
							}
						}
					}
				}
			}
		}

		ctx.log(format!(
			"Discovery complete: {} audio/video files",
			self.state.entries.len()
		));

		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpeechToTextJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl From<SpeechToTextJobOutput> for JobOutput {
	fn from(output: SpeechToTextJobOutput) -> Self {
		JobOutput::SpeechToText {
			total_processed: output.total_processed,
			success_count: output.success_count,
			error_count: output.error_count,
		}
	}
}

impl DynJob for SpeechToTextJob {
	fn job_name(&self) -> &'static str {
		"Speech Transcription"
	}
}

impl From<SpeechToTextJob> for Box<dyn DynJob> {
	fn from(job: SpeechToTextJob) -> Self {
		Box::new(job)
	}
}
