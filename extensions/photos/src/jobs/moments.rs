use spacedrive_sdk::job;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;
use uuid::Uuid;

use crate::agent::{PhotoEvent, PhotosMind};
use crate::models::*;
use crate::utils::*;

#[derive(Serialize, Deserialize, Default)]
pub struct CreateMomentsState {
	pub photo_events: Vec<PhotoEvent>,
}

#[job]
pub async fn create_moments(ctx: &JobContext, state: &mut CreateMomentsState) -> JobResult<()> {
	let photo_events = &state.photo_events;
	let moment_groups = cluster_into_moments(&photo_events);

	for group in moment_groups {
		let moment = Moment {
			id: Uuid::new_v4(),
			title: generate_moment_title(ctx, &group).await?,
			start_date: group.start_date,
			end_date: group.end_date,
			location: group.place_id,
			photo_ids: group.photo_ids.clone(),
			photo_count: group.photo_ids.len(),
		};

		ctx.vdfs().create_model(moment).await?;

		// Note: Memory updates should be done by agent handlers, not jobs
		// The agent can observe moment creation and update its memory accordingly
	}

	Ok(())
}

async fn generate_moment_title(ctx: &JobContext, group: &MomentGroup) -> JobResult<String> {
	#[derive(Serialize)]
	struct MomentPrompt {
		location: Option<String>,
		scenes: Vec<String>,
		date: String,
	}

	let title = ctx
		.ai()
		.from_registered("llm:local")
		.prompt_template("generate_moment_title.jinja")
		.render_with(&MomentPrompt {
			location: group.place_name.clone(),
			scenes: group.common_scenes.clone(),
			date: group.start_date.format("%B %Y").to_string(),
		})?
		.generate_text()
		.await?;

	Ok(title)
}
