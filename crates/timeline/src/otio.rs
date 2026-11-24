//! OpenTimelineIO JSON (`.otio`), which Resolve and any OTIO tool read.
//!
//! Clips keep their source rate: a clip's `source_range` and its media's
//! `available_range` are counted from the file's start timecode at the
//! file's rate, the convention OTIO's own editorial adapters use.

use serde_json::{json, Value};

use crate::{file_url, timeline::Edit, Rate, Timeline, TimelineError};

fn time(value: i64, rate: Rate) -> Value {
	json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate.fps(), "value": value as f64 })
}

fn range(start: i64, duration: i64, rate: Rate) -> Value {
	json!({
		"OTIO_SCHEMA": "TimeRange.1",
		"start_time": time(start, rate),
		"duration": time(duration, rate),
	})
}

fn clip(edit: &Edit<'_>) -> Value {
	let media = edit.media;
	let name = edit.clip.label.as_deref().unwrap_or(&media.name);
	json!({
		"OTIO_SCHEMA": "Clip.2",
		"name": name,
		"source_range": range(
			media.start_timecode + edit.clip.source_in,
			edit.clip.source_out - edit.clip.source_in,
			media.rate,
		),
		"media_references": {
			"DEFAULT_MEDIA": {
				"OTIO_SCHEMA": "ExternalReference.1",
				"name": media.name,
				"target_url": file_url(&media.path),
				"available_range": range(media.start_timecode, media.duration, media.rate),
				"available_image_bounds": null,
				"metadata": {},
			}
		},
		"active_media_reference_key": "DEFAULT_MEDIA",
		"effects": [],
		"markers": [],
		"enabled": true,
		"metadata": {},
	})
}

fn gap(duration: i64, rate: Rate) -> Value {
	json!({
		"OTIO_SCHEMA": "Gap.1",
		"name": "",
		"source_range": range(0, duration, rate),
		"effects": [],
		"markers": [],
		"enabled": true,
		"metadata": {},
	})
}

fn track(name: &str, kind: &str, children: Vec<Value>) -> Value {
	json!({
		"OTIO_SCHEMA": "Track.1",
		"name": name,
		"kind": kind,
		"source_range": null,
		"children": children,
		"effects": [],
		"markers": [],
		"enabled": true,
		"metadata": {},
	})
}

pub fn write(timeline: &Timeline) -> Result<String, TimelineError> {
	let edits = timeline.edits()?;
	let video: Vec<Value> = edits.iter().map(clip).collect();
	let mut tracks = vec![track("V1", "Video", video)];
	if edits.iter().any(|edit| edit.media.audio.is_some()) {
		// A clip without sound leaves a gap as long as its picture, at the
		// same source rate, so the audio stays in sync with the video track.
		let audio = edits
			.iter()
			.map(|edit| match edit.media.audio {
				Some(_) => clip(edit),
				None => gap(edit.clip.source_out - edit.clip.source_in, edit.media.rate),
			})
			.collect();
		tracks.push(track("A1", "Audio", audio));
	}
	let document = json!({
		"OTIO_SCHEMA": "Timeline.1",
		"name": timeline.name,
		"global_start_time": time(0, timeline.rate),
		"tracks": {
			"OTIO_SCHEMA": "Stack.1",
			"name": "tracks",
			"source_range": null,
			"children": tracks,
			"effects": [],
			"markers": [],
			"enabled": true,
			"metadata": {},
		},
		"metadata": {},
	});
	Ok(serde_json::to_string_pretty(&document).expect("a JSON value always serializes") + "\n")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::timeline::tests::sample;

	#[test]
	fn clips_keep_their_source_rate_and_timecode() {
		let document: Value = serde_json::from_str(&write(&sample()).unwrap()).unwrap();
		let tracks = &document["tracks"]["children"];
		let first = &tracks[0]["children"][0];
		assert_eq!(first["source_range"]["start_time"]["value"], 1_212_750.0);
		assert_eq!(first["source_range"]["duration"]["value"], 72.0);
		assert_eq!(
			first["source_range"]["start_time"]["rate"],
			24000.0 / 1001.0
		);
		assert_eq!(
			first["media_references"]["DEFAULT_MEDIA"]["target_url"],
			"file:///Volumes/Footage/Japan%20Trip/A001_C004.mov"
		);
		let second = &tracks[0]["children"][1];
		assert_eq!(second["name"], "platform wide");
		assert_eq!(second["source_range"]["start_time"]["rate"], 25.0);
		// The second clip has no sound, so the audio track holds a gap as
		// long as its picture, at its own rate.
		let gap = &tracks[1]["children"][1];
		assert_eq!(gap["OTIO_SCHEMA"], "Gap.1");
		assert_eq!(gap["source_range"]["duration"]["value"], 137.0);
		assert_eq!(gap["source_range"]["duration"]["rate"], 25.0);
	}
}
