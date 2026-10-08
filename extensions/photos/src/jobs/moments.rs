use spacedrive_sdk::job;

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::types::JobResult;
use uuid::Uuid;

use crate::models::*;
use crate::utils::*;

/// The custom field that ties a photo to its moment.
pub const MOMENT_FIELD: &str = "photos.moment_id";

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct CreateMomentsState {
	/// Photos to group; empty means every image the grant reaches.
	pub photo_ids: Vec<Uuid>,
	/// A pause longer than this between consecutive photos starts a new
	/// moment.
	pub gap_hours: f64,
	/// Consecutive photos farther apart than this start a new moment, when
	/// both carry a location.
	pub split_km: f64,
	pub moments: usize,
	pub grouped: usize,
	pub undated: usize,
	pub already_grouped: usize,
}

impl Default for CreateMomentsState {
	fn default() -> Self {
		Self {
			photo_ids: Vec::new(),
			gap_hours: 6.0,
			split_km: 50.0,
			moments: 0,
			grouped: 0,
			undated: 0,
			already_grouped: 0,
		}
	}
}

/// Group photos into moments by capture time and place, from EXIF alone.
///
/// A moment is a run of photos with no pause longer than `gap_hours`
/// between neighbours and no jump longer than `split_km`. Each moment is a
/// `Moment` model; each photo in it is tagged `Moments/<title>` and gets
/// its moment id as a custom field, which is also what keeps a re-run from
/// grouping a photo twice. A photo with no capture time belongs to no
/// moment. Titles come from the dates, not from a language model.
#[job(name = "create_moments")]
pub async fn create_moments(ctx: &JobContext, state: &mut CreateMomentsState) -> JobResult<()> {
	ctx.progress(Progress::indeterminate("Reading capture times..."));

	let photos = if state.photo_ids.is_empty() {
		ctx.vdfs()
			.query_records()
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

	let mut captures = Vec::new();
	for photo in &photos {
		ctx.check_interrupt().await?;
		if photo.custom_field::<Uuid>(MOMENT_FIELD).await?.is_some() {
			state.already_grouped += 1;
			continue;
		}
		let Some(taken) = photo.exif().await?.and_then(|exif| {
			exif.date_taken.map(|taken| PhotoCapture {
				photo_id: photo.id(),
				taken,
				location: exif
					.latitude
					.zip(exif.longitude)
					.map(|(latitude, longitude)| GpsCoordinates {
						latitude,
						longitude,
						altitude: None,
					}),
			})
		}) else {
			state.undated += 1;
			continue;
		};
		captures.push(taken);
	}

	let groups = cluster_into_moments(&captures, state.gap_hours, state.split_km);
	let total = groups.len();
	for (index, group) in groups.into_iter().enumerate() {
		ctx.check_interrupt().await?;
		let title = moment_title(group.start_date, group.end_date);
		let moment = Moment {
			id: Uuid::new_v4(),
			title: title.clone(),
			start_date: group.start_date,
			end_date: group.end_date,
			location: group.place_id,
			photo_ids: group.photo_ids.clone(),
			photo_count: group.photo_ids.len(),
		};
		ctx.vdfs().create_model(moment.clone()).await?;

		for photo_id in &group.photo_ids {
			ctx.vdfs()
				.add_tag(*photo_id, &format!("Moments/{title}"))
				.await?;
			ctx.vdfs()
				.update_custom_field(*photo_id, MOMENT_FIELD, moment.id)
				.await?;
			state.grouped += 1;
			ctx.increment_items(1);
		}
		state.moments += 1;
		ctx.checkpoint(state)?;
		ctx.progress(Progress::simple(
			(index + 1) as f32 / total as f32,
			format!("Created moment {title} ({} photos)", group.photo_ids.len()),
		));
	}

	ctx.log(&format!(
		"Created {} moments over {} photos ({} undated, {} already in a moment)",
		state.moments, state.grouped, state.undated, state.already_grouped
	));
	ctx.progress(Progress::complete("Moments created"));
	Ok(())
}

/// A title from the span a moment covers: `March 12, 2024`, `March 12 to
/// 14, 2024`, `March 30 to April 2, 2024`, `December 31, 2023 to January 1,
/// 2024`.
pub fn moment_title(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
	let (sd, ed) = (start.date_naive(), end.date_naive());
	if sd == ed {
		return start.format("%B %-d, %Y").to_string();
	}
	if sd.year() != ed.year() {
		return format!(
			"{} to {}",
			start.format("%B %-d, %Y"),
			end.format("%B %-d, %Y")
		);
	}
	if sd.month() != ed.month() {
		return format!("{} to {}", start.format("%B %-d"), end.format("%B %-d, %Y"));
	}
	format!("{} to {}", start.format("%B %-d"), end.format("%-d, %Y"))
}
