use spacedrive_sdk::query;

use spacedrive_sdk::prelude::*;

use crate::agent::PhotosMind;
use crate::models::Photo;

#[query("photos with {scene_type}")]
pub async fn search_scene(
	ctx: &QueryContext<PhotosMind>,
	scene_type: String,
) -> QueryResult<Vec<Photo>> {
	let records = ctx
		.vdfs()
		.query_records()
		.with_tag(&format!("#scene:{}", scene_type))
		.of_type::<Image>()
		.collect()
		.await?;
	Ok(records.into_iter().map(Photo::from_record).collect())
}
