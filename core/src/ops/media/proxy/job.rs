//! Proxy generation job for batch operations

use super::processor::ProxyProcessor;
use super::{ProxyJobConfig, ProxyPhase, ProxyState};
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

/// Proxy generation job
#[derive(Serialize, Deserialize)]
pub struct ProxyJob {
	config: ProxyJobConfig,
	state: ProxyState,
}

impl ProxyJob {
	pub fn new(config: ProxyJobConfig) -> Self {
		Self {
			config,
			state: ProxyState::new(),
		}
	}

	pub fn with_defaults() -> Self {
		Self::new(ProxyJobConfig::default())
	}

	async fn run_discovery(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		use crate::infra::db::entities::{content_identity, entry};
		use sea_orm::{
			ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
		};

		ctx.log("Starting proxy discovery phase");

		let db = ctx.library_db();

		// Query for video entries with content
		let results = entry::Entity::find()
			.filter(entry::Column::Kind.eq(0)) // Files only
			.join(JoinType::InnerJoin, entry::Relation::ContentIdentity.def())
			.filter(content_identity::Column::KindId.eq(2)) // Video kind
			.filter(content_identity::Column::Uuid.is_not_null())
			.all(db)
			.await
			.map_err(|e| JobError::execution(format!("Database query failed: {}", e)))?;

		ctx.log(format!("Found {} video entries", results.len()));

		// Build entry list
		for entry_model in results {
			let path = crate::ops::indexing::PathResolver::get_full_path(db, entry_model.id)
				.await
				.map_err(|e| JobError::execution(format!("Failed to resolve path: {}", e)))?;

			self.state
				.entries
				.push((entry_model.id, path, entry_model.extension));
		}

		ctx.log(format!(
			"Discovery complete: {} entries to process",
			self.state.entries.len()
		));

		Ok(())
	}
}

impl Job for ProxyJob {
	const NAME: &'static str = "proxy_generation";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Generate video proxies for smooth playback");
}

impl DynJob for ProxyJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProxyJobOutput {
	pub total_processed: usize,
	pub success_count: usize,
	pub error_count: usize,
	pub total_encoding_time_secs: u64,
}

impl From<ProxyJobOutput> for JobOutput {
	fn from(output: ProxyJobOutput) -> Self {
		JobOutput::Custom(serde_json::json!({
			"type": "proxy_generation",
			"total_processed": output.total_processed,
			"success_count": output.success_count,
			"error_count": output.error_count,
			"encoding_time_secs": output.total_encoding_time_secs,
		}))
	}
}

#[async_trait::async_trait]
impl JobHandler for ProxyJob {
	type Output = ProxyJobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Discovery phase
		if self.state.phase == ProxyPhase::Discovery {
			ctx.log("Starting proxy discovery phase");
			self.run_discovery(&ctx).await?;
			self.state.phase = ProxyPhase::Processing;
		}

		// Processing phase
		ctx.log(format!(
			"Proxy processing phase starting with {} entries",
			self.state.entries.len()
		));

		// Create processor instance
		let processor = ProxyProcessor::new(ctx.library_arc())
			.with_enabled(true) // Enable for job execution
			.with_hardware_accel(self.config.use_hardware_accel);

		let total = self.state.entries.len();

		// Process each entry ONE AT A TIME (proxies are long operations)
		while self.state.processed < total {
			ctx.check_interrupt().await?;

			let (entry_id, path, mime_type) = &self.state.entries[self.state.processed];

			ctx.log(format!(
				"Processing {}/{}: {}",
				self.state.processed + 1,
				total,
				path.display()
			));

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
				self.state.processed += 1;
				continue;
			}

			// Time the encoding
			let start = std::time::Instant::now();

			// Process entry using processor
			match processor.process(ctx.library_db(), &proc_entry).await {
				Ok(result) if result.success => {
					let encoding_time = start.elapsed().as_secs();
					self.state.total_encoding_time_secs += encoding_time;

					ctx.log(format!(
						"Generated {} proxy variants for {} in {}s",
						result.artifacts_created,
						path.display(),
						encoding_time
					));
					self.state.success_count += 1;
				}
				Ok(_) => {
					warn!("Proxy generation failed for {}", path.display());
					self.state.error_count += 1;
				}
				Err(e) => {
					ctx.log(format!("ERROR: Proxy error for {}: {}", path.display(), e));
					self.state.error_count += 1;
				}
			}

			self.state.processed += 1;

			// Report progress
			ctx.progress(Progress::Count {
				current: self.state.processed,
				total,
			});

			// Checkpoint after EACH video (proxies are long operations)
			ctx.checkpoint().await?;
		}

		self.state.phase = ProxyPhase::Complete;
		ctx.log(format!(
			"Proxy generation complete: {} success, {} errors, total encoding time: {}s",
			self.state.success_count, self.state.error_count, self.state.total_encoding_time_secs
		));

		Ok(ProxyJobOutput {
			total_processed: self.state.processed,
			success_count: self.state.success_count,
			error_count: self.state.error_count,
			total_encoding_time_secs: self.state.total_encoding_time_secs,
		})
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log(format!(
			"Resuming proxy job at {}/{} ({}s total encoding time)",
			self.state.processed,
			self.state.entries.len(),
			self.state.total_encoding_time_secs
		));
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing proxy generation job - state will be preserved");
		Ok(())
	}

	async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log(format!(
			"Cancelling proxy job - generated {} proxies",
			self.state.success_count
		));
		Ok(())
	}
}
