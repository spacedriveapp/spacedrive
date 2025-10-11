use spacedrive_sdk::job;

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;
use uuid::Uuid;

use crate::models::*;
use crate::tasks::*;

#[derive(Serialize, Deserialize, Default)]
pub struct AnalyzeScenesState {
	pub photo_ids: Vec<Uuid>,
}

#[job]
pub async fn analyze_scenes(ctx: &JobContext, state: &mut AnalyzeScenesState) -> JobResult<()> {
	for photo_id in &state.photo_ids {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;

		let scenes = ctx.run(classify_scene, photo.clone()).await?;

		if let Some(content_uuid) = photo.content_uuid() {
			ctx.save_sidecar(content_uuid, "scene", "photos", &scenes)
				.await?;
		}

		for scene in &scenes {
			if scene.confidence
				> ctx
					.config::<crate::PhotosConfig>()
					.scene_confidence_threshold
			{
				ctx.vdfs()
					.add_tag(photo.metadata_id(), &format!("#scene:{}", scene.label))
					.await?;
			}
		}
	}

	Ok(())
}
