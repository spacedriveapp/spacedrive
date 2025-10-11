use spacedrive_sdk::task;

use spacedrive_sdk::prelude::*;

use crate::models::SceneTag;

#[task(requires_capability = "gpu_optional")]
pub async fn classify_scene(ctx: TaskContext, photo: Entry) -> TaskResult<Vec<SceneTag>> {
	let image_bytes = photo.read().await?;

	let classifications = ctx
		.ai()
		.from_registered("scene_classification:resnet50")
		.classify(&image_bytes)
		.await?;

	// Convert SDK SceneTag to our model's SceneTag
	let converted = classifications
		.into_iter()
		.map(|s| SceneTag {
			label: s.label,
			confidence: s.confidence,
		})
		.collect();

	Ok(converted)
}
