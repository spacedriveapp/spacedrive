use spacedrive_sdk::{job, task};

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;
use uuid::Uuid;

use crate::models::*;
use crate::utils::*;

/// The custom field that ties a photo to its place.
pub const PLACE_FIELD: &str = "photos.place_id";

/// What a place is called until something names it.
pub const UNNAMED_PLACE: &str = "Unknown Location";

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IdentifyPlacesState {
	/// A source-relative directory to look under; "" is every source root.
	pub location: String,
	/// Photos to place; empty means every image under `location`.
	pub photo_ids: Vec<Uuid>,
	/// Photos within this distance of a place's centre belong to it.
	pub radius_meters: f64,
	pub placed: usize,
	pub unlocated: usize,
	pub places_created: usize,
	pub already_placed: usize,
}

/// Group photos by where they were taken, from EXIF GPS alone, and tag
/// each with its place.
///
/// Clustering, place matching, the `Place` model, the custom field and the
/// `Places/<name>` tag need no inference. A photo that already carries a
/// place is left alone, so a re-run places only new photos and a place's
/// count stays the number of photos assigned to it. Naming a new place does
/// need inference: the
/// reverse geocode asks a language model, and when the host has none the
/// place keeps the name `Unknown Location` and the job warns once, so a
/// later run with a provider can name it.
#[job(name = "identify_places")]
pub async fn identify_places_in_location(
	ctx: &JobContext,
	state: &mut IdentifyPlacesState,
) -> JobResult<()> {
	ctx.progress(Progress::indeterminate("Finding photos with GPS..."));
	if state.radius_meters <= 0.0 {
		state.radius_meters = 500.0;
	}

	let photos = if state.photo_ids.is_empty() {
		ctx.vdfs()
			.query_records()
			.in_location(state.location.clone())
			.of_type::<Image>()
			.collect()
			.await?
	} else {
		let mut photos = Vec::with_capacity(state.photo_ids.len());
		for id in &state.photo_ids {
			photos.push(ctx.vdfs().get_record(*id).await?);
		}
		photos
	};

	let mut located = Vec::new();
	for photo in photos {
		ctx.check_interrupt().await?;
		if photo.custom_field::<Uuid>(PLACE_FIELD).await?.is_some() {
			state.already_placed += 1;
			continue;
		}
		let coords = photo.exif().await?.and_then(|exif| {
			exif.latitude
				.zip(exif.longitude)
				.map(|(latitude, longitude)| GpsCoordinates {
					latitude,
					longitude,
					altitude: None,
				})
		});
		match coords {
			Some(coords) => located.push((photo, coords)),
			None => state.unlocated += 1,
		}
	}

	let mut geocoder_missing = false;
	for cluster in cluster_by_location(&located, state.radius_meters) {
		ctx.check_interrupt().await?;
		let (mut place, created) = find_or_create_place(ctx, &cluster, state.radius_meters).await?;
		if created {
			state.places_created += 1;
		}

		if place.name == UNNAMED_PLACE && !geocoder_missing {
			match ctx.run(reverse_geocode, cluster.center.clone()).await {
				Ok(name) => {
					let id = place.id;
					ctx.vdfs()
						.update_model(id, |mut p: Place| {
							p.name = name.clone();
							Ok(p)
						})
						.await?;
					place.name = name;
				}
				Err(Error::NotAvailable(reason)) => {
					ctx.add_warning(&format!("place naming skipped: {reason}"));
					geocoder_missing = true;
				}
				Err(e) => return Err(e),
			}
		}

		for photo in &cluster.photos {
			ctx.vdfs()
				.update_custom_field(photo.id(), PLACE_FIELD, place.id)
				.await?;
			ctx.vdfs()
				.add_tag(photo.id(), &format!("Places/{}", place.name))
				.await?;
			state.placed += 1;
			ctx.increment_items(1);
		}
		ctx.checkpoint(state)?;
	}

	ctx.log(&format!(
		"Placed {} photos ({} without a location, {} new places, {} already placed)",
		state.placed, state.unlocated, state.places_created, state.already_placed
	));
	ctx.progress(Progress::complete("Places identified"));
	Ok(())
}

/// Ask a language model what is at these coordinates.
#[task]
async fn reverse_geocode(ctx: TaskContext, coords: GpsCoordinates) -> TaskResult<String> {
	let name = ctx
		.ai()
		.from_registered("llm:local")
		.generate_text(&format!(
			"Name the place at latitude {} and longitude {} in a few words.",
			coords.latitude, coords.longitude
		))
		.await?;
	Ok(name.trim().to_string())
}

/// The existing place whose centre is within `radius_meters` of the
/// cluster's, or a new unnamed one. Returns whether it was created.
async fn find_or_create_place(
	ctx: &JobContext,
	cluster: &PlaceCluster,
	radius_meters: f64,
) -> JobResult<(Place, bool)> {
	let places = ctx.vdfs().query_models::<Place>().collect().await?;
	let nearest = places.into_iter().min_by(|a, b| {
		let da = distance_meters(&place_center(a), &cluster.center);
		let db = distance_meters(&place_center(b), &cluster.center);
		da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
	});
	if let Some(place) = nearest {
		if distance_meters(&place_center(&place), &cluster.center) <= radius_meters {
			let id = place.id;
			let count = cluster.photos.len();
			ctx.vdfs()
				.update_model(id, |mut p: Place| {
					p.photo_count += count;
					Ok(p)
				})
				.await?;
			return Ok((place, false));
		}
	}
	let place = Place {
		id: Uuid::new_v4(),
		name: UNNAMED_PLACE.to_string(),
		latitude: cluster.center.latitude,
		longitude: cluster.center.longitude,
		radius_meters: radius_meters as f32,
		photo_count: cluster.photos.len(),
		thumbnail_photo_id: cluster.photos.first().map(|p| p.id()),
	};
	ctx.vdfs().create_model(place.clone()).await?;
	Ok((place, true))
}

fn place_center(place: &Place) -> GpsCoordinates {
	GpsCoordinates {
		latitude: place.latitude,
		longitude: place.longitude,
		altitude: None,
	}
}
