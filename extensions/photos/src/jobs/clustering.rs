use spacedrive_sdk::{job, task};

use spacedrive_sdk::prelude::*;
use spacedrive_sdk::tasks::TaskContext;
use uuid::Uuid;

use crate::models::*;
use crate::utils::*;

#[task(retries = 1, timeout_ms = 60000)]
pub async fn cluster_faces_into_people(ctx: TaskContext, photo_ids: Vec<Uuid>) -> TaskResult<()> {
	let mut all_faces: Vec<(Uuid, FaceDetection)> = Vec::new();

	for photo_id in &photo_ids {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;
		if let Some(content_uuid) = photo.content_uuid() {
			if let Ok(faces) = ctx
				.read_sidecar::<Vec<FaceDetection>>(content_uuid, "faces")
				.await
			{
				for face in faces {
					all_faces.push((*photo_id, face));
				}
			}
		}
	}

	let clusters = dbscan_clustering(
		&all_faces,
		ctx.config::<crate::PhotosConfig>()
			.face_clustering_threshold,
	);

	for cluster in clusters {
		let person_id = find_or_create_person(&ctx, &cluster).await?;

		for (photo_id, _) in cluster.faces {
			ctx.vdfs()
				.update_custom_field(photo_id, "identified_people", person_id)
				.await?;
		}
	}

	Ok(())
}

#[task]
pub async fn generate_face_tags(ctx: TaskContext, photo_ids: Vec<Uuid>) -> TaskResult<()> {
	for photo_id in &photo_ids {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;

		if let Ok(people) = photo.custom_field::<Vec<PersonId>>("identified_people") {
			for person_id in people {
				if let Ok(person) = ctx.vdfs().get_model::<Person>(person_id).await {
					if let Some(name) = person.name {
						ctx.vdfs()
							.add_tag(photo.metadata_id(), &format!("#person:{}", name))
							.await?;
					}
				}
			}
		}
	}

	Ok(())
}

async fn find_or_create_person(ctx: &TaskContext, cluster: &FaceCluster) -> TaskResult<PersonId> {
	todo!("Implement person matching")
}
