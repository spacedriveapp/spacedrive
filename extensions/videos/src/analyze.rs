//! The per-shot pass: caption each keyframe, label its place and find its
//! faces.

use serde::{Deserialize, Serialize};
use spacedrive_sdk::job;
use spacedrive_sdk::prelude::*;

use crate::infer;
use crate::models::{FacesDoc, Footage, SceneDoc, ShotFaces, ShotScene, ShotSummary};
use crate::people;
use crate::shots::{self, Shot, ShotList, ShotListRead};

/// The file extensions the manifest's `read_records` glob grants.
pub const VIDEO_EXTENSIONS: &[&str] = &[
	"mp4", "mov", "m4v", "mkv", "avi", "webm", "mts", "m2ts", "mxf",
];

/// A place label below this confidence is noise more often than not.
const PLACE_CONFIDENCE: f32 = 0.2;
const PLACES_PER_SHOT: usize = 3;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnalyzeState {
	/// Records to analyze; empty means every video the grant covers.
	pub records: Vec<Uuid>,
	pub listed: bool,
	pub current: usize,
	pub analyzed: usize,
	pub waiting_for_shots: usize,
	pub skipped: usize,
	pub shots: usize,
	/// Categories that answered `not_available` during this run.
	pub unavailable: Vec<String>,
	/// Leave the roster alone, for a caller that clusters later.
	pub skip_clustering: bool,
}

/// Analyze every shot of every video, then rebuild the people roster.
///
/// Safe to run again over a growing folder: a shot already captioned,
/// labelled or searched for faces is not sent to a model again, and a
/// category that had no provider last time is retried. A missing provider
/// is a warning, never a failure, so the job completes and the shot list
/// is browsable with whatever the core and the models could provide.
#[job(name = "analyze")]
pub async fn analyze_footage(ctx: &JobContext, state: &mut AnalyzeState) -> Result<()> {
	if !state.listed {
		if state.records.is_empty() {
			state.records = ctx
				.vdfs()
				.query_records()
				.with_extensions(VIDEO_EXTENSIONS.iter().copied())
				.collect()
				.await?
				.into_iter()
				.map(|record| record.id())
				.collect();
		}
		state.listed = true;
		ctx.checkpoint(state)?;
	}

	let total = state.records.len();
	ctx.log(&format!("Analyzing {total} videos"));
	let mut shots_unsupported = false;
	while state.current < total {
		let uuid = state.records[state.current];
		match ctx.vdfs().get_record(uuid).await {
			Ok(record) => analyze_record(ctx, state, &record, &mut shots_unsupported).await?,
			Err(Error::NotFound) => state.skipped += 1,
			Err(e) => return Err(e),
		}
		state.current += 1;
		ctx.increment_items(1);
		ctx.progress(Progress::simple(
			state.current as f32 / total.max(1) as f32,
			format!("Analyzed {}/{total} videos", state.current),
		));
		ctx.check_interrupt().await?;
		ctx.checkpoint(state)?;
	}

	if !state.skip_clustering {
		people::rebuild(ctx).await?;
	}

	let unavailable = if state.unavailable.is_empty() {
		String::from("none")
	} else {
		state.unavailable.join(", ")
	};
	ctx.log(&format!(
		"Videos: {} analyzed ({} shots), {} waiting for shots, {} skipped; not_available: {unavailable}",
		state.analyzed, state.shots, state.waiting_for_shots, state.skipped
	));
	ctx.progress(Progress::complete("Video analysis complete"));
	Ok(())
}

/// Note a category that has no provider: one warning per run, and the
/// file's summary names it so the client can say why a shot has no
/// description.
fn unavailable(ctx: &JobContext, state: &mut AnalyzeState, category: &str, reason: &str) {
	if !state.unavailable.iter().any(|c| c == category) {
		ctx.add_warning(&format!("{category} not_available: {reason}"));
		state.unavailable.push(category.to_string());
	}
}

fn is_unavailable(state: &AnalyzeState, category: &str) -> bool {
	state.unavailable.iter().any(|c| c == category)
}

async fn analyze_record(
	ctx: &JobContext,
	state: &mut AnalyzeState,
	record: &Record,
	shots_unsupported: &mut bool,
) -> Result<()> {
	let Some(media_uuid) = record.media_uuid else {
		ctx.log(&format!("{} has not been hashed yet", record.name()));
		state.skipped += 1;
		return Ok(());
	};
	let previous: Option<Footage> = match ctx.vdfs().get_model_by_content(media_uuid).await {
		Ok(footage) => Some(footage),
		Err(Error::NotFound) => None,
		Err(e) => return Err(e),
	};

	let list = match shots::read_shot_list(ctx, media_uuid).await? {
		ShotListRead::Found(list) => list,
		ShotListRead::Missing => {
			state.waiting_for_shots += 1;
			return save_footage(ctx, record, media_uuid, None, Vec::new(), Vec::new()).await;
		}
		ShotListRead::Unsupported(reason) => {
			if !*shots_unsupported {
				ctx.add_warning(&format!("shots not_available: {reason}"));
				*shots_unsupported = true;
			}
			state.waiting_for_shots += 1;
			return save_footage(ctx, record, media_uuid, None, Vec::new(), Vec::new()).await;
		}
	};

	let mut scene: SceneDoc = read_own(ctx, media_uuid, "scene")
		.await?
		.unwrap_or_default();
	let mut faces: FacesDoc = read_own(ctx, media_uuid, "faces")
		.await?
		.unwrap_or_default();
	scene.version = 1;
	faces.version = 1;
	let (mut scene_changed, mut faces_changed) = (false, false);
	let mut missing_here = Vec::new();

	for shot in &list.shots {
		let needs_caption = scene
			.shots
			.iter()
			.find(|s| s.shot_id == shot.id)
			.is_none_or(|s| s.description.is_none())
			&& !is_unavailable(state, "image_caption");
		let needs_places = scene
			.shots
			.iter()
			.find(|s| s.shot_id == shot.id)
			.is_none_or(|s| s.places.is_empty())
			&& !is_unavailable(state, "scene_classification");
		let needs_faces = !faces.shots.iter().any(|s| s.shot_id == shot.id)
			&& !is_unavailable(state, "face_detection");
		if !(needs_caption || needs_places || needs_faces) {
			continue;
		}
		let image = match shots::read_keyframe(ctx, media_uuid, shot).await {
			Ok(image) => image,
			Err(Error::NotFound) => {
				ctx.log(&format!(
					"{}: shot {} has no keyframe yet",
					record.name(),
					shot.id
				));
				continue;
			}
			Err(e) => return Err(e),
		};

		let entry_index = match scene.shots.iter().position(|s| s.shot_id == shot.id) {
			Some(i) => i,
			None => {
				scene.shots.push(ShotScene {
					shot_id: shot.id.clone(),
					..Default::default()
				});
				scene.shots.len() - 1
			}
		};
		if needs_caption {
			match infer::caption(&image) {
				Ok(text) => {
					scene.shots[entry_index].description = Some(text);
					scene_changed = true;
				}
				Err(Error::NotAvailable(reason)) => {
					unavailable(ctx, state, "image_caption", &reason)
				}
				Err(e) => ctx.add_warning(&format!("caption {}: {e}", shot.id)),
			}
		}
		if needs_places {
			match infer::classify(&image) {
				Ok(tags) => {
					scene.shots[entry_index].places = tags;
					scene_changed = true;
				}
				Err(Error::NotAvailable(reason)) => {
					unavailable(ctx, state, "scene_classification", &reason)
				}
				Err(e) => ctx.add_warning(&format!("classify {}: {e}", shot.id)),
			}
		}
		if needs_faces {
			match infer::detect_faces(&image) {
				Ok(found) => {
					faces.shots.push(ShotFaces {
						shot_id: shot.id.clone(),
						faces: found,
					});
					faces_changed = true;
				}
				Err(Error::NotAvailable(reason)) => {
					unavailable(ctx, state, "face_detection", &reason)
				}
				Err(e) => ctx.add_warning(&format!("detect faces {}: {e}", shot.id)),
			}
		}
	}
	scene
		.shots
		.retain(|s| s.description.is_some() || !s.places.is_empty());

	if scene_changed {
		ctx.save_sidecar(media_uuid, "scene", &scene).await?;
	}
	if faces_changed {
		ctx.save_sidecar(media_uuid, "faces", &faces).await?;
	}
	for category in ["image_caption", "scene_classification", "face_detection"] {
		if is_unavailable(state, category) {
			missing_here.push(category.to_string());
		}
	}

	let summaries = summarize(&list, &scene, &faces, previous.as_ref());
	let mut places: Vec<&str> = summaries
		.iter()
		.flat_map(|s| s.places.iter().map(String::as_str))
		.collect();
	places.sort_unstable();
	places.dedup();
	for place in places {
		ctx.vdfs()
			.add_tag(
				record.id(),
				&format!("Videos/Places/{}", tag_segment(place)),
			)
			.await?;
	}

	state.analyzed += 1;
	state.shots += list.shots.len();
	save_footage(
		ctx,
		record,
		media_uuid,
		Some(&list),
		summaries,
		missing_here,
	)
	.await
}

/// One of this extension's own sidecars, or `None` when there is none yet.
async fn read_own<T: serde::de::DeserializeOwned>(
	ctx: &JobContext,
	media_uuid: Uuid,
	kind: &str,
) -> Result<Option<T>> {
	if !ctx.sidecar_exists(media_uuid, kind)? {
		return Ok(None);
	}
	ctx.read_sidecar(media_uuid, kind).await.map(Some)
}

fn summarize(
	list: &ShotList,
	scene: &SceneDoc,
	faces: &FacesDoc,
	previous: Option<&Footage>,
) -> Vec<ShotSummary> {
	list.shots
		.iter()
		.map(|shot: &Shot| {
			let described = scene.shots.iter().find(|s| s.shot_id == shot.id);
			let mut places: Vec<String> = described
				.map(|s| {
					s.places
						.iter()
						.filter(|t| t.confidence >= PLACE_CONFIDENCE)
						.take(PLACES_PER_SHOT)
						.map(|t| t.label.clone())
						.collect()
				})
				.unwrap_or_default();
			places.dedup();
			ShotSummary {
				id: shot.id.clone(),
				index: shot.index,
				start_ms: shot.start.ms(),
				end_ms: shot.end.ms(),
				keyframe: shot.id.clone(),
				description: described.and_then(|s| s.description.clone()),
				places,
				faces: faces
					.shots
					.iter()
					.find(|s| s.shot_id == shot.id)
					.map_or(0, |s| s.faces.len() as u32),
				people: previous
					.and_then(|f| f.shots.iter().find(|s| s.id == shot.id))
					.map(|s| s.people.clone())
					.unwrap_or_default(),
			}
		})
		.collect()
}

async fn save_footage(
	ctx: &JobContext,
	record: &Record,
	media_uuid: Uuid,
	list: Option<&ShotList>,
	shots: Vec<ShotSummary>,
	unavailable: Vec<String>,
) -> Result<()> {
	let footage = Footage {
		id: media_uuid,
		record: record.id(),
		source_id: record.source_id,
		name: record.name().to_string(),
		relative_path: record.path().to_string(),
		content_uuid: record.content_uuid(),
		status: if list.is_some() {
			"analyzed"
		} else {
			"waiting_for_shots"
		}
		.to_string(),
		duration_ms: list.and_then(|l| l.duration).map_or(0, |d| d.ms()),
		shots,
		unavailable,
		updated_ms: spacedrive_sdk::clock::now_ms(),
	};
	ctx.vdfs()
		.create_model_for_content(media_uuid, footage)
		.await
}

/// A label as one tag path segment.
pub fn tag_segment(label: &str) -> String {
	label.replace('/', "-").trim().to_string()
}
