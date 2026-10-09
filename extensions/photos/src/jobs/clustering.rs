use spacedrive_sdk::task;

use spacedrive_sdk::prelude::*;
use spacedrive_sdk::tasks::TaskContext;
use uuid::Uuid;

use crate::models::{FaceDetection, *};
use crate::utils::*;

#[task(retries = 1, timeout_ms = 60000)]
pub async fn cluster_faces_into_people(ctx: TaskContext, photo_ids: Vec<Uuid>) -> TaskResult<()> {
	let mut all_faces: Vec<(Uuid, FaceDetection)> = Vec::new();

	for photo_id in &photo_ids {
		let photo = ctx.vdfs().get_record(*photo_id).await?;
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
			let mut people = ctx
				.vdfs()
				.custom_field::<Vec<PersonId>>(photo_id, PEOPLE_FIELD)
				.await?
				.unwrap_or_default();
			if !people.contains(&person_id) {
				people.push(person_id);
				ctx.vdfs()
					.update_custom_field(photo_id, PEOPLE_FIELD, &people)
					.await?;
			}
		}
	}

	Ok(())
}

/// The custom field listing the people identified in a photo.
pub const PEOPLE_FIELD: &str = "photos.identified_people";

#[task]
pub async fn generate_face_tags(ctx: TaskContext, photo_ids: Vec<Uuid>) -> TaskResult<()> {
	for photo_id in &photo_ids {
		let photo = ctx.vdfs().get_record(*photo_id).await?;

		let people = photo
			.custom_field::<Vec<PersonId>>(PEOPLE_FIELD)
			.await?
			.unwrap_or_default();
		for person_id in people {
			if let Ok(person) = ctx.vdfs().get_model::<Person>(person_id).await {
				if let Some(name) = person.name {
					ctx.vdfs()
						.add_tag(photo.id(), &format!("People/{}", name))
						.await?;
				}
			}
		}
	}

	Ok(())
}

/// The person whose representative embedding is nearest the cluster's
/// centroid, when it is within the clustering threshold; otherwise a new,
/// unnamed person built from the cluster.
async fn find_or_create_person(ctx: &TaskContext, cluster: &FaceCluster) -> TaskResult<PersonId> {
	let threshold = ctx
		.config::<crate::PhotosConfig>()
		.face_clustering_threshold;
	let people = ctx.vdfs().query_models::<Person>().collect().await?;
	let nearest = people
		.iter()
		.filter(|person| !person.representative_embedding.is_empty())
		.map(|person| {
			(
				person,
				cosine_distance(
					&person.representative_embedding,
					&cluster.centroid_embedding,
				),
			)
		})
		.filter(|(_, distance)| *distance <= threshold)
		.min_by(|a, b| a.1.total_cmp(&b.1));

	if let Some((person, _)) = nearest {
		let embeddings: Vec<Vec<f32>> = cluster
			.faces
			.iter()
			.map(|(_, face)| face.embedding.clone())
			.collect();
		let photos = cluster.faces.len();
		ctx.vdfs()
			.update_model(person.id, |mut p: Person| {
				p.embeddings.extend(embeddings);
				p.photo_count += photos;
				p.representative_embedding = mean_embedding(&p.embeddings);
				Ok(p)
			})
			.await?;
		return Ok(person.id);
	}

	let person = Person {
		id: Uuid::new_v4(),
		name: None,
		thumbnail_photo_id: cluster.faces.first().map(|(photo, _)| *photo),
		embeddings: cluster
			.faces
			.iter()
			.map(|(_, face)| face.embedding.clone())
			.collect(),
		photo_count: cluster.faces.len(),
		representative_embedding: cluster.centroid_embedding.clone(),
	};
	let id = person.id;
	ctx.vdfs().create_model(person).await?;
	Ok(id)
}
