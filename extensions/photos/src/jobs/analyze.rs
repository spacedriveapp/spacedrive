use spacedrive_sdk::job;

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::jobs::clustering::{cluster_faces_into_people, generate_face_tags};
use crate::tasks::*;

#[derive(Serialize, Deserialize, Default)]
pub struct AnalyzePhotosState {
	pub photo_ids: Vec<Uuid>,
	pub current_index: usize,
}

#[job]
pub async fn analyze_photos_batch(ctx: &JobContext, state: &mut AnalyzePhotosState) -> Result<()> {
	ctx.progress(Progress::indeterminate("Analyzing photos for faces..."));

	let photo_ids = state.photo_ids.clone();
	let total = photo_ids.len();

	for (idx, photo_id) in photo_ids.iter().enumerate() {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;

		if let Some(content_uuid) = photo.content_uuid() {
			if ctx.sidecar_exists(content_uuid, "faces")? {
				continue;
			}
		} else {
			continue;
		}

		let faces = ctx.run(detect_faces_in_photo, photo.clone()).await?;

		if let Some(content_uuid) = photo.content_uuid() {
			ctx.save_sidecar(content_uuid, "faces", "photos", &faces)
				.await?;
		}

		ctx.check_interrupt().await?;

		ctx.progress(Progress::simple(
			(idx + 1) as f32 / total as f32,
			format!("Analyzed {}/{} photos", idx + 1, total),
		));
	}

	let photo_ids_clone = state.photo_ids.clone();
	ctx.run(cluster_faces_into_people, photo_ids_clone.clone())
		.await?;
	ctx.run(generate_face_tags, photo_ids_clone).await?;

	ctx.progress(Progress::complete("Face analysis complete"));
	Ok(())
}
