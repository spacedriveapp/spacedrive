//! OCR job for batch text extraction

use super::processor::OcrProcessor;
use crate::{
	infra::{
		db::entities::entry,
		job::{prelude::*, traits::DynJob},
	},
	ops::indexing::processor::ProcessorEntry,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OcrJobConfig {
	/// Location ID to process (None = all entries in library)
	pub location_id: Option<Uuid>,
	/// Single entry UUID to process (for UI-triggered single file)
	pub entry_uuid: Option<Uuid>,
	/// Languages for OCR (e.g., ["eng", "spa"])
	pub languages: Vec<String>,
	/// Minimum confidence threshold (0.0 - 1.0)
	pub min_confidence: f32,
	/// Reprocess files that already have text
	pub reprocess: bool,
}

impl Default for OcrJobConfig {
	fn default() -> Self {
		Self {
			location_id: None,
			entry_uuid: None,
			languages: vec!["eng".to_string()],
			min_confidence: 0.6,
			reprocess: false,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OcrJobState {
	phase: OcrPhase,
	entries: Vec<(i32, std::path::PathBuf, Option<String>)>, // (entry_id, path, mime_type)
	processed: usize,
	success_count: usize,
	error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum OcrPhase {
	Discovery,
	Processing,
	Complete,
}

#[derive(Serialize, Deserialize)]
pub struct OcrJob {
	config: OcrJobConfig,
	state: OcrJobState,
}

impl OcrJob {
	pub fn new(config: OcrJobConfig) -> Self {
		Self {
			config,
			state: OcrJobState {
				phase: OcrPhase::Discovery,
				entries: Vec::new(),
				processed: 0,
				success_count: 0,
				error_count: 0,
			},
		}
	}

	pub fn from_location(location_id: Uuid) -> Self {
		Self::new(OcrJobConfig {
			location_id: Some(location_id),
			..Default::default()
		})
	}
}

impl Job for OcrJob {
	const NAME: &'static str = "ocr";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Extract text from images and PDFs using OCR");
}

#[async_trait::async_trait]
impl JobHandler for OcrJob {
	type Output = OcrJobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		match self.state.phase {
			OcrPhase::Discovery => {
				ctx.log("Starting OCR discovery phase");
				self.run_discovery(&ctx).await?;
				self.state.phase = OcrPhase::Processing;
			}
			OcrPhase::Processing => {}
			OcrPhase::Complete => {
				return Ok(OcrJobOutput {
					total_processed: self.state.processed,
					success_count: self.state.success_count,
					error_count: self.state.error_count,
				});
			}
		}

		// Processing phase
		ctx.log(format!(
			"OCR processing phase starting with {} entries",
			self.state.entries.len()
		));

		let processor = OcrProcessor::new(ctx.library_arc())
			.with_languages(self.config.languages.clone())
			.with_min_confidence(self.config.min_confidence);

		let total = self.state.entries.len();

		while self.state.processed < total {
			ctx.check_interrupt().await?;

			let (entry_id, path, mime_type) = &self.state.entries[self.state.processed];

			// Load entry to get content_id
			let entry_model = entry::Entity::find_by_id(*entry_id)
				.one(ctx.library_db())
				.await?
				.ok_or_else(|| JobError::execution("Entry not found"))?;

			// Build processor entry
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

			// Process entry
			match processor.process(ctx.library_db(), &proc_entry).await {
				Ok(result) if result.success => {
					ctx.log(format!(
						"Extracted text from {}: {} chars",
						path.display(),
						result.bytes_processed
					));
					self.state.success_count += 1;
				}
				Ok(_) => {
					warn!("OCR failed for {}", path.display());
					self.state.error_count += 1;
				}
				Err(e) => {
					ctx.log(format!("ERROR: OCR error for {}: {}", path.display(), e));
					self.state.error_count += 1;
				}
			}

			self.state.processed += 1;

			// Report progress
			ctx.progress(Progress::Count {
				current: self.state.processed,
				total,
			});

			// Checkpoint every 10 files
			if self.state.processed % 10 == 0 {
				ctx.checkpoint().await?;
			}
		}

		self.state.phase = OcrPhase::Complete;
		ctx.log(format!(
			"OCR complete: {} success, {} errors",
			self.state.success_count, self.state.error_count
		));

		Ok(OcrJobOutput {
			total_processed: self.state.processed,
			success_count: self.state.success_count,
			error_count: self.state.error_count,
		})
	}
}

impl OcrJob {
	async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use crate::infra::db::entities::{content_identity, entry, mime_type};

		ctx.log("Starting OCR discovery");

		// Note: Tesseract uses system-installed language data
		// Future: Implement tesseract language data download similar to whisper
		ctx.log(format!(
			"Using tesseract with languages: {} (system installation)",
			self.config.languages.join(", ")
		));

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
					// Skip if already has text (unless forcing)
					if !self.config.reprocess
						&& ci.text_content.is_some()
						&& !ci.text_content.as_ref().unwrap().is_empty()
					{
						ctx.log("Entry already has extracted text, skipping");
						return Ok(());
					}

					if let Some(mime_id) = ci.mime_type_id {
						if let Ok(Some(mime)) = mime_type::Entity::find_by_id(mime_id).one(db).await
						{
							if super::is_ocr_supported(&mime.mime_type) {
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

			ctx.log(format!(
				"Single file discovered: {} entries",
				self.state.entries.len()
			));
			return Ok(());
		}

		// Batch mode - build query for entries that support OCR
		let mut query = entry::Entity::find().filter(entry::Column::ContentId.is_not_null());

		// Filter by location if specified
		if let Some(location_id) = self.config.location_id {
			// TODO: Add location scoping via entry_closure
			info!("Location filtering not yet implemented, processing all entries");
		}

		let entries = query.all(db).await?;

		ctx.log(format!("Found {} entries with content", entries.len()));

		// Load MIME types for filtering
		for entry_model in entries {
			if let Some(content_id) = entry_model.content_id {
				// Check if already has text (unless reprocessing)
				if !self.config.reprocess {
					if let Ok(Some(ci)) = content_identity::Entity::find_by_id(content_id)
						.one(db)
						.await
					{
						if ci.text_content.is_some()
							&& !ci.text_content.as_ref().unwrap().is_empty()
						{
							continue; // Skip - already has text
						}

						// Get MIME type
						if let Some(mime_id) = ci.mime_type_id {
							if let Ok(Some(mime)) =
								mime_type::Entity::find_by_id(mime_id).one(db).await
							{
								if super::is_ocr_supported(&mime.mime_type) {
									// Get full path
									if let Ok(path) =
										crate::ops::indexing::PathResolver::get_full_path(
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
		}

		ctx.log(format!(
			"Discovery complete: {} entries ready for OCR",
			self.state.entries.len()
		));

		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OcrJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl From<OcrJobOutput> for JobOutput {
	fn from(output: OcrJobOutput) -> Self {
		JobOutput::OcrExtraction {
			total_processed: output.total_processed,
			success_count: output.success_count,
			error_count: output.error_count,
		}
	}
}

impl DynJob for OcrJob {
	fn job_name(&self) -> &'static str {
		"OCR Text Extraction"
	}
}

impl From<OcrJob> for Box<dyn DynJob> {
	fn from(job: OcrJob) -> Self {
		Box::new(job)
	}
}
