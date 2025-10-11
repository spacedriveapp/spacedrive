use spacedrive_sdk::query;

use spacedrive_sdk::prelude::*;

use crate::agent::PhotosMind;
use crate::models::*;

#[query("photos of {person_name}")]
pub async fn search_person(
	ctx: &QueryContext<PhotosMind>,
	person_name: String,
) -> QueryResult<Vec<Photo>> {
	let person = ctx
		.vdfs()
		.query_models::<Person>()
		.where_field("name", equals(&person_name))
		.first()
		.await?
		.ok_or(QueryError::NotFound)?;

	let photo_ids = ctx.memory().read().await.photos_of_person(person.id).await;

	let mut photos = Vec::new();
	for photo_id in photo_ids {
		if let Ok(photo) = ctx.vdfs().get_model::<Photo>(photo_id).await {
			photos.push(photo);
		}
	}

	Ok(photos)
}
