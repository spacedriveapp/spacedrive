//! Gaussian splat generation job for batch image processing

use super::processor::GaussianSplatProcessor;
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
pub struct GaussianSplatJobConfig {
	/// Location ID to process (None = all entries in library)
	pub location_id: Option<Uuid>,
	/// Single entry UUID to process (for UI-triggered single file)
	pub entry_uuid: Option<Uuid>,
	/// Path to SHARP model checkpoint (None = auto-download)
	pub model_path: Option<String>,
	/// Reprocess files that already have splats
	pub reprocess: bool,
}

impl Default for GaussianSplatJobConfig {
	fn default() -> Self {
		Self {
			location_id: None,
			entry_uuid: None,
			model_path: None,
			reprocess: false,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplatJobState {
	phase: SplatPhase,
	entries: Vec<(i32, std::path::PathBuf, Option<String>)>,
	processed: usize,
	success_count: usize,
	error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SplatPhase {
	Discovery,
	Processing,
	Complete,
}

#[derive(Serialize, Deserialize)]
pub struct GaussianSplatJob {
	config: GaussianSplatJobConfig,
	state: SplatJobState,
}

impl GaussianSplatJob {
	pub fn new(config: GaussianSplatJobConfig) -> Self {
		Self {
			config,
			state: SplatJobState {
				phase: SplatPhase::Discovery,
				entries: Vec::new(),
				processed: 0,
				success_count: 0,
				error_count: 0,
			},
		}
	}

	pub fn from_location(location_id: Uuid) -> Self {
		Self::new(GaussianSplatJobConfig {
			location_id: Some(location_id),
			..Default::default()
		})
	}
}

impl Job for GaussianSplatJob {
	const NAME: &'static str = "gaussian_splat";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> =
		Some("Generate 3D Gaussian splats from images for view synthesis");
}

#[async_trait::async_trait]
impl JobHandler for GaussianSplatJob {
	type Output = GaussianSplatJobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		match self.state.phase {
			SplatPhase::Discovery => {
				ctx.log("Starting Gaussian splat discovery phase");
				self.run_discovery(&ctx).await?;
				self.state.phase = SplatPhase::Processing;
			}
			SplatPhase::Processing => {}
			SplatPhase::Complete => {
				return Ok(GaussianSplatJobOutput {
					total_processed: self.state.processed,
					success_count: self.state.success_count,
					error_count: self.state.error_count,
				});
			}
		}

		ctx.log(format!(
			"Gaussian splat processing {} images",
			self.state.entries.len()
		));

		let processor = GaussianSplatProcessor::new(ctx.library_arc());
		let processor = if let Some(ref model_path) = self.config.model_path {
			processor.with_model_path(model_path.clone())
		} else {
			processor
		};

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

			// Report progress
			ctx.progress(Progress::Indeterminate(format!(
				"Generating splat for {}...",
				path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
			)));

			let result = processor.process(ctx.library_db(), &proc_entry).await;

			match result {
				Ok(result) if result.success => {
					ctx.log(format!(
						"Generated splat for {}: {} bytes",
						path.display(),
						result.bytes_processed
					));
					self.state.success_count += 1;
				}
				Ok(_) => {
					warn!("Splat generation failed for {}", path.display());
					self.state.error_count += 1;
				}
				Err(e) => {
					ctx.log(format!(
						"ERROR: Splat generation error for {}: {}",
						path.display(),
						e
					));
					self.state.error_count += 1;
				}
			}

			self.state.processed += 1;

			// Report progress with count
			ctx.progress(Progress::Count {
				current: self.state.processed,
				total,
			});

			if self.state.processed % 5 == 0 {
				ctx.checkpoint().await?;
			}
		}

		self.state.phase = SplatPhase::Complete;
		ctx.log(format!(
			"Gaussian splat generation complete: {} success, {} errors",
			self.state.success_count, self.state.error_count
		));

		Ok(GaussianSplatJobOutput {
			total_processed: self.state.processed,
			success_count: self.state.success_count,
			error_count: self.state.error_count,
		})
	}
}

impl GaussianSplatJob {
	async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use crate::infra::db::entities::{content_identity, entry, mime_type};

		ctx.log("Starting Gaussian splat discovery");

		// Check if SHARP CLI is available
		if !super::check_sharp_available().await.unwrap_or(false) {
			return Err(JobError::execution(
				"SHARP CLI not found. Please install ml-sharp (pip install -e /path/to/ml-sharp)",
			));
		}

		ctx.log("SHARP CLI available");

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
						if let Ok(Some(mime)) = mime_type::Entity::find_by_id(mime_id).one(db).await
						{
							if super::is_splat_supported(&mime.mime_type) {
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
							if super::is_splat_supported(&mime.mime_type) {
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
			"Discovery complete: {} image files",
			self.state.entries.len()
		));

		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GaussianSplatJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl From<GaussianSplatJobOutput> for JobOutput {
	fn from(output: GaussianSplatJobOutput) -> Self {
		JobOutput::GaussianSplat {
			total_processed: output.total_processed,
			success_count: output.success_count,
			error_count: output.error_count,
		}
	}
}

impl DynJob for GaussianSplatJob {
	fn job_name(&self) -> &'static str {
		"Gaussian Splat Generation"
	}
}

impl From<GaussianSplatJob> for Box<dyn DynJob> {
	fn from(job: GaussianSplatJob) -> Self {
		Box::new(job)
	}
}
