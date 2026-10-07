use spacedrive_sdk::job;

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::jobs::clustering::{cluster_faces_into_people, generate_face_tags};
use crate::tasks::*;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnalyzePhotosState {
	pub photo_ids: Vec<Uuid>,
	pub current_index: usize,
	/// Photos whose faces sidecar was written by this run.
	pub analyzed: usize,
	/// Photos skipped because they have no content identity yet or the face
	/// detector is not available on this device.
	pub skipped: usize,
}

/// Detect faces in each photo, cluster them into people, and tag the photos.
///
/// A photo that already has a faces sidecar is left alone, so the job is
/// safe to re-run over a growing set. When the host has no face detection
/// provider the detection step is skipped with one warning and the photo
/// keeps no sidecar, so a later run with a provider picks it up.
#[job(name = "analyze_photos")]
pub async fn analyze_photos_batch(ctx: &JobContext, state: &mut AnalyzePhotosState) -> Result<()> {
	ctx.progress(Progress::indeterminate("Analyzing photos for faces..."));

	let photo_ids = state.photo_ids.clone();
	let total = photo_ids.len();
	let mut detector_missing = false;

	for (idx, photo_id) in photo_ids.iter().enumerate().skip(state.current_index) {
		let photo = ctx.vdfs().get_record(*photo_id).await?;

		if let Some(content_uuid) = photo.content_uuid() {
			if !ctx.sidecar_exists(content_uuid, "faces")? {
				match ctx.run(detect_faces_in_photo, photo.clone()).await {
					Ok(faces) => {
						ctx.save_sidecar(content_uuid, "faces", &faces).await?;
						state.analyzed += 1;
					}
					Err(Error::NotAvailable(reason)) => {
						if !detector_missing {
							ctx.add_warning(&format!("face detection skipped: {reason}"));
							detector_missing = true;
						}
						state.skipped += 1;
					}
					Err(e) => return Err(e),
				}
			}
		} else {
			state.skipped += 1;
		}

		state.current_index = idx + 1;
		ctx.increment_items(1);
		ctx.check_interrupt().await?;
		if state.current_index % 10 == 0 {
			ctx.checkpoint(state)?;
		}

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
