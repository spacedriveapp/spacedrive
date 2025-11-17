//! Thumbstrip generation job for batch operations

use super::processor::ThumbstripProcessor;
use super::{ThumbstripJobConfig, ThumbstripPhase, ThumbstripState};
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

/// Thumbstrip generation job
#[derive(Serialize, Deserialize)]
pub struct ThumbstripJob {
	config: ThumbstripJobConfig,
	state: ThumbstripState,
}

impl ThumbstripJob {
	pub fn new(config: ThumbstripJobConfig) -> Self {
		Self {
			config,
			state: ThumbstripState::new(),
		}
	}

	pub fn with_defaults() -> Self {
		Self::new(ThumbstripJobConfig::default())
	}

	async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use crate::infra::db::entities::{content_identity, entry, mime_type};
		use sea_orm::{
			ColumnTrait, EntityTrait, FromQueryResult, JoinType, QueryFilter, QuerySelect,
			RelationTrait,
		};

		ctx.log("Starting thumbstrip discovery phase");

		let db = ctx.library_db();

		#[derive(Debug, FromQueryResult)]
		struct EntryWithMimeType {
			entry_id: i32,
			mime_type: Option<String>,
		}

		// Query for video entries with MIME types in one go
		let results = entry::Entity::find()
			.select_only()
			.column_as(entry::Column::Id, "entry_id")
			.column_as(mime_type::Column::MimeType, "mime_type")
			.filter(entry::Column::Kind.eq(0)) // Files only
			.join(JoinType::InnerJoin, entry::Relation::ContentIdentity.def())
			.join(
				JoinType::LeftJoin,
				content_identity::Relation::MimeType.def(),
			)
			.filter(content_identity::Column::KindId.eq(2)) // Video kind
			.filter(content_identity::Column::Uuid.is_not_null())
			.into_model::<EntryWithMimeType>()
			.all(db)
			.await
			.map_err(|e| JobError::execution(format!("Database query failed: {}", e)))?;

		ctx.log(format!("Found {} video entries", results.len()));

		// Build entry list
		for result in results {
			let path = crate::ops::indexing::PathResolver::get_full_path(db, result.entry_id)
				.await
				.map_err(|e| JobError::execution(format!("Failed to resolve path: {}", e)))?;

			self.state
				.entries
				.push((result.entry_id, path, result.mime_type));
		}

		ctx.log(format!(
			"Discovery complete: {} entries to process",
			self.state.entries.len()
		));

		Ok(())
	}
}

impl Job for ThumbstripJob {
	const NAME: &'static str = "thumbstrip_generation";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> =
		Some("Generate video thumbstrips (storyboard thumbnails)");
}

impl DynJob for ThumbstripJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbstripJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
}

impl From<ThumbstripJobOutput> for JobOutput {
	fn from(output: ThumbstripJobOutput) -> Self {
		JobOutput::Custom(serde_json::json!({
			"type": "thumbstrip_generation",
			"total_processed": output.total_processed,
			"success_count": output.success_count,
			"error_count": output.error_count,
		}))
	}
}

#[async_trait::async_trait]
impl JobHandler for ThumbstripJob {
	type Output = ThumbstripJobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Discovery phase
		if self.state.phase == ThumbstripPhase::Discovery {
			ctx.log("Starting thumbstrip discovery phase");
			self.run_discovery(&ctx).await?;
			self.state.phase = ThumbstripPhase::Processing;
		}

		// Processing phase
		ctx.log(format!(
			"Thumbstrip processing phase starting with {} entries",
			self.state.entries.len()
		));

		// Create processor instance
		let processor = ThumbstripProcessor::new(ctx.library_arc())
			.with_variants(self.config.variants.clone())
			.with_regenerate(self.config.regenerate);

		let total = self.state.entries.len();

		// Process each entry
		while self.state.processed < total {
			ctx.check_interrupt().await?;

			let (entry_id, path, mime_type) = &self.state.entries[self.state.processed];

			// Load entry model
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

			// Check if processor should run
			if !processor.should_process(&proc_entry) {
				ctx.log(format!(
					"Processor.should_process returned false for {} (mime: {:?})",
					path.display(),
					mime_type
				));
				self.state.processed += 1;
				continue;
			}

			ctx.log(format!("Processor will process: {}", path.display()));

			// Process entry using processor
			match processor.process(ctx.library_db(), &proc_entry).await {
				Ok(result) if result.success => {
					ctx.log(format!(
						"Generated {} thumbstrip variants for {}",
						result.artifacts_created,
						path.display()
					));
					self.state.success_count += 1;
				}
				Ok(_) => {
					warn!("Thumbstrip generation failed for {}", path.display());
					self.state.error_count += 1;
				}
				Err(e) => {
					ctx.log(format!(
						"ERROR: Thumbstrip error for {}: {}",
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

			// Checkpoint every 10 files
			if self.state.processed % 10 == 0 {
				ctx.checkpoint().await?;
			}
		}

		self.state.phase = ThumbstripPhase::Complete;
		ctx.log(format!(
			"Thumbstrip complete: {} success, {} errors",
			self.state.success_count, self.state.error_count
		));

		Ok(ThumbstripJobOutput {
			total_processed: self.state.processed,
			success_count: self.state.success_count,
			error_count: self.state.error_count,
		})
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log(format!(
			"Resuming thumbstrip job at {}/{}",
			self.state.processed,
			self.state.entries.len()
		));
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing thumbstrip generation job - state will be preserved");
		Ok(())
	}

	async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log(format!(
			"Cancelling thumbstrip job - generated {} thumbstrips",
			self.state.success_count
		));
		Ok(())
	}
}
