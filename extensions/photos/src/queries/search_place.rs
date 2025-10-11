use spacedrive_sdk::query;

use spacedrive_sdk::prelude::*;

use crate::agent::PhotosMind;
use crate::models::*;

#[query("photos from {place_name}")]
pub async fn search_place(
	ctx: &QueryContext<PhotosMind>,
	place_name: String,
) -> QueryResult<Vec<Photo>> {
	let place = ctx
		.vdfs()
		.query_models::<Place>()
		.search_semantic("name", similar_to(&place_name))
		.first()
		.await?
		.ok_or(QueryError::NotFound)?;

	let photo_ids = ctx.memory().read().await.photos_at_place(place.id).await;

	let mut photos = Vec::new();
	for photo_id in photo_ids {
		if let Ok(photo) = ctx.vdfs().get_model::<Photo>(photo_id).await {
			photos.push(photo);
		}
	}

	Ok(photos)
}
