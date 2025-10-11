use spacedrive_sdk::query;

use spacedrive_sdk::prelude::*;

use crate::agent::PhotosMind;
use crate::models::Photo;

#[query("photos with {scene_type}")]
pub async fn search_scene(
	ctx: &QueryContext<PhotosMind>,
	scene_type: String,
) -> QueryResult<Vec<Photo>> {
	ctx.vdfs()
		.query_entries()
		.with_tag(&format!("#scene:{}", scene_type))
		.of_type::<Image>()
		.map(|entry| Photo::from_entry(entry))
		.collect()
		.await
}
