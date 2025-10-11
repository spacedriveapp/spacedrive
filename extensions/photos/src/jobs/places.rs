use spacedrive_sdk::{job, task};

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;

use crate::models::*;
use crate::utils::*;

#[derive(Serialize, Deserialize, Default)]
pub struct IdentifyPlacesState {
	pub location: String,
}

#[job]
pub async fn identify_places_in_location(
	ctx: &JobContext,
	state: &mut IdentifyPlacesState,
) -> JobResult<()> {
	let location = SdPath::from(&state.location);
	ctx.progress(Progress::indeterminate("Finding photos with GPS..."));

	let photos = ctx
		.vdfs()
		.query_entries()
		.in_location(location)
		.of_type::<Image>()
		.where_metadata("exif.gps", is_not_null())
		.collect()
		.await?;

	let place_clusters = cluster_by_location(&photos, 500.0);

	for cluster in place_clusters {
		let place = find_or_create_place(ctx, &cluster).await?;

		if place.name == "Unknown Location" {
			let name = ctx.run(reverse_geocode, cluster.center.clone()).await?;
			ctx.vdfs()
				.update_model(place.id, |mut p: Place| {
					p.name = name;
					Ok(p)
				})
				.await?;
		}

		for photo in &cluster.photos {
			ctx.vdfs()
				.update_custom_field(photo.id(), "place_id", place.id)
				.await?;

			ctx.vdfs()
				.add_tag(photo.metadata_id(), &format!("#place:{}", place.name))
				.await?;
		}
	}

	ctx.progress(Progress::complete("Places identified"));
	Ok(())
}

#[task]
async fn reverse_geocode(ctx: TaskContext, coords: GpsCoordinates) -> TaskResult<String> {
	#[derive(Serialize)]
	struct GeoPrompt {
		lat: f64,
		lon: f64,
	}

	let place_name = ctx
		.ai()
		.from_registered("llm:local")
		.prompt_template("identify_place.jinja")
		.render_with(&GeoPrompt {
			lat: coords.latitude,
			lon: coords.longitude,
		})?
		.generate_text()
		.await?;

	Ok(place_name)
}

async fn find_or_create_place(ctx: &JobContext, cluster: &PlaceCluster) -> JobResult<Place> {
	todo!("Implement place matching")
}
