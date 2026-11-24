use spacedrive_sdk::job;

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;
use uuid::Uuid;

use crate::models::SceneTag;
use crate::tasks::*;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnalyzeScenesState {
	/// Photos to classify; empty means every image the grant reaches.
	pub photo_ids: Vec<Uuid>,
	pub current_index: usize,
	pub classified: usize,
	pub captioned: usize,
	pub skipped: usize,
}

/// Classify each photo's scene, tag it `Scenes/<label>` above the
/// configured confidence, and describe it in a `caption` sidecar.
///
/// Classification and captioning are inference, so with no provider on
/// the host each is skipped after one warning and writes no sidecar; a
/// photo that already has one is left alone, so a later run with a model
/// installed fills in only what is missing. The sidecar, the tags and
/// the resume bookkeeping need no provider.
#[job(name = "analyze_scenes")]
pub async fn analyze_scenes(ctx: &JobContext, state: &mut AnalyzeScenesState) -> JobResult<()> {
	if state.photo_ids.is_empty() {
		state.photo_ids = ctx
			.vdfs()
			.query_records()
			.of_type::<Image>()
			.collect()
			.await?
			.into_iter()
			.map(|photo| photo.id())
			.collect();
	}
	let threshold = ctx
		.config::<crate::PhotosConfig>()
		.scene_confidence_threshold;
	let photo_ids = state.photo_ids.clone();
	let total = photo_ids.len();
	let mut classifier_missing = false;
	let mut captioner_missing = false;

	for (idx, photo_id) in photo_ids.iter().enumerate().skip(state.current_index) {
		let photo = ctx.vdfs().get_record(*photo_id).await?;
		let Some(content_uuid) = photo.content_uuid() else {
			state.skipped += 1;
			state.current_index = idx + 1;
			continue;
		};

		let scenes: Vec<SceneTag> = if ctx.sidecar_exists(content_uuid, "scene")? {
			ctx.read_sidecar(content_uuid, "scene").await?
		} else {
			match ctx.run(classify_scene, photo.clone()).await {
				Ok(scenes) => {
					ctx.save_sidecar(content_uuid, "scene", &scenes).await?;
					state.classified += 1;
					scenes
				}
				Err(Error::NotAvailable(reason)) => {
					if !classifier_missing {
						ctx.add_warning(&format!("scene classification skipped: {reason}"));
						classifier_missing = true;
					}
					state.skipped += 1;
					Vec::new()
				}
				Err(e) => return Err(e),
			}
		};

		if !captioner_missing && !ctx.sidecar_exists(content_uuid, "caption")? {
			match ctx.run(caption_photo, photo.clone()).await {
				Ok(caption) => {
					ctx.save_sidecar(content_uuid, "caption", &caption).await?;
					state.captioned += 1;
				}
				Err(Error::NotAvailable(reason)) => {
					ctx.add_warning(&format!("captioning skipped: {reason}"));
					captioner_missing = true;
				}
				Err(e) => ctx.add_warning(&format!("caption {}: {e}", photo.name())),
			}
		}

		for scene in scenes.iter().filter(|s| s.confidence > threshold) {
			ctx.vdfs()
				.add_tag(photo.id(), &format!("Scenes/{}", scene.label))
				.await?;
		}

		state.current_index = idx + 1;
		ctx.increment_items(1);
		ctx.check_interrupt().await?;
		if state.current_index % 10 == 0 {
			ctx.checkpoint(state)?;
		}
		ctx.progress(Progress::simple(
			(idx + 1) as f32 / total as f32,
			format!("Classified {}/{} photos", idx + 1, total),
		));
	}

	ctx.log(&format!(
		"Scenes: {} classified, {} skipped",
		state.classified, state.skipped
	));
	ctx.log(&format!("Captions: {} written", state.captioned));
	Ok(())
}
