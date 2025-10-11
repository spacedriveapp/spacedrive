use chrono::Duration;
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{agent, agent_trail, filter, on_event, on_startup, scheduled};

use crate::agent::{PhotoEvent, PhotosMind};
use crate::jobs::*;

#[agent]
#[agent_trail(level = "debug", rotation = "daily")]
impl crate::Photos {
	#[on_startup]
	pub async fn initialize(ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		tracing::info!("Photos extension initialized");

		if !ctx.models().is_registered("face_detection:photos_v1") {
			ctx.trace("Face detection model not found - will register on first use");
		}

		Ok(())
	}

	#[on_event(EntryCreated)]
	#[filter(".extension().is_image()")]
	pub async fn on_new_photo(entry: Entry, ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		ctx.trace(format!("New photo detected: {}", entry.name()));

		if !ctx.in_granted_scope(&entry.path()) {
			ctx.trace("Photo not in granted scope - skipping");
			return Ok(());
		}

		let mut memory = ctx.memory().write().await;
		memory
			.plan
			.update(|mut plan| {
				plan.photos_needing_faces.push(entry.id());
				Ok(plan)
			})
			.await?;

		if memory.plan.read().await.photos_needing_faces.len() >= 50 {
			ctx.jobs()
				.dispatch(
					analyze_photos_batch,
					memory.plan.read().await.photos_needing_faces.clone(),
				)
				.priority(Priority::Low)
				.when_idle()
				.execute()
				.await?;

			memory
				.plan
				.update(|mut plan| {
					plan.photos_needing_faces.clear();
					Ok(plan)
				})
				.await?;
		}

		Ok(())
	}

	#[scheduled(cron = "0 9 * * SUN")]
	pub async fn generate_weekly_memories(ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		ctx.trace("Generating weekly memories");

		let memory = ctx.memory().read().await;
		let last_week = memory
			.history
			.query()
			.since(Duration::days(7))
			.where_field("location", is_not_null())
			.collect()
			.await?;

		if !last_week.is_empty() {
			ctx.jobs()
				.dispatch(create_moments, last_week)
				.execute()
				.await?;
		}

		Ok(())
	}
}
