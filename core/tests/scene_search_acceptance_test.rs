//! Acceptance for scene descriptions in content search
//! (`docs/core/content-search.mdx`): a `scene` sidecar the Videos extension
//! writes is found through `search.content` with the shot's range, a
//! rewrite replaces what it said, and deleting it drops the hits.
//!
//! The sidecar is written to disk the way the extension host writes it, then
//! handed to the same function the host calls after each write.

#![cfg(feature = "video-shots")]

mod helpers;

use std::{path::Path, time::Duration};

use helpers::*;
use sd_core::{
	domain::{
		addressing::SdPath,
		sidecar::{ContentSidecar, SidecarFormat, SidecarKind, SidecarVariant},
	},
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::{AddOverrides, Library},
	ops::{
		indexing::text_extract::TextKind,
		media::shots::{
			content_uuid_of_video, AnalyzedMedia, FrameRate, Shot, ShotList, SHOTS_VERSION,
		},
		search::content::{
			descriptions::{index_sidecar, VIDEOS_EXTENSION},
			ContentSearchInput, ContentSearchOutput, ContentSearchQuery,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
};
use uuid::Uuid;

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

async fn search(harness: &IndexingHarness, scope: &Path, query: &str) -> ContentSearchOutput {
	ContentSearchQuery::from_input(ContentSearchInput {
		scope: SdPath::local(scope),
		query: query.to_string(),
		kinds: vec![],
		max_results: None,
		cursor: None,
	})
	.unwrap()
	.execute(harness.core.context.clone(), session(&harness.library))
	.await
	.unwrap()
}

fn scene_sidecar(content_uuid: Uuid) -> ContentSidecar {
	ContentSidecar {
		content_uuid,
		kind: SidecarKind::extension(VIDEOS_EXTENSION, "scene").unwrap(),
		variant: SidecarVariant::new("default"),
		format: SidecarFormat::Json,
	}
}

fn shot_list(content_uuid: Uuid) -> ShotList {
	let rate = FrameRate { num: 24, den: 1 };
	let shot = |index: u32, a: u64, b: u64| Shot {
		id: Shot::id_for(content_uuid, a, b),
		index,
		in_point: rate.point(a, None),
		out_point: rate.point(b, None),
		duration_frames: b - a,
		duration_seconds: rate.seconds(b - a),
		keyframe: rate.point((a + b) / 2, None),
		cut_score: 0.0,
	};
	ShotList {
		version: SHOTS_VERSION,
		content_uuid,
		analyzed: AnalyzedMedia::Original,
		rate,
		start_timecode: None,
		out_exclusive: true,
		duration: rate.point(240, None),
		threshold: 0.3,
		min_shot_seconds: 1.0,
		shots: vec![shot(0, 0, 96), shot(1, 96, 240)],
	}
}

async fn write_json(path: &Path, value: &serde_json::Value) {
	tokio::fs::create_dir_all(path.parent().unwrap())
		.await
		.unwrap();
	tokio::fs::write(path, serde_json::to_vec(value).unwrap())
		.await
		.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scene_sidecar_is_searchable_with_its_shot_range() {
	let harness = IndexingHarnessBuilder::new("scene_search_acceptance")
		.disable_watcher()
		.build()
		.await
		.unwrap();
	let dir = harness.create_test_dir("footage").await.unwrap();
	let root = dir.path().to_path_buf();
	let video = dir
		.write_file("day1/harbour.mp4", "not really a video, but bytes to hash")
		.await
		.unwrap();

	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.clone(),
		name: None,
		overrides: AddOverrides::default(),
	})
	.map_err(anyhow::Error::msg)
	.unwrap()
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.unwrap();
	let store = harness
		.core
		.context
		.volume_index()
		.store_for(&output.root)
		.await
		.expect("the tracked source has a store");
	let text_index = store.text_index().await.unwrap();

	// The media sidecars are keyed by the candidate uuid of the bytes.
	let content_uuid = content_uuid_of_video(&video).await.unwrap();
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	while !text_index.has_content(content_uuid).await.unwrap() {
		assert!(
			tokio::time::Instant::now() < deadline,
			"the video was never identified"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}

	let library_path = harness.library.path().to_path_buf();
	let list = shot_list(content_uuid);
	write_json(
		&ContentSidecar::shots(content_uuid).path_in(&library_path),
		&serde_json::to_value(&list).unwrap(),
	)
	.await;
	let scene_path = scene_sidecar(content_uuid).path_in(&library_path);
	let scene = |second: &str| {
		serde_json::json!({
			"version": 1,
			"shots": [
				{ "shot_id": list.shots[0].id.to_string(), "description": "A tugboat leaves the dock at dawn", "places": [] },
				{ "shot_id": list.shots[1].id.to_string(), "description": second, "places": [] },
			]
		})
	};
	write_json(&scene_path, &scene("A red lighthouse on a rocky point")).await;
	let indexed = index_sidecar(&harness.library, VIDEOS_EXTENSION, "scene", content_uuid)
		.await
		.unwrap();
	assert_eq!(indexed, 1);

	let out = search(&harness, &root, "lighthouse").await;
	assert_eq!(out.hits.len(), 1, "{out:?}");
	let hit = &out.hits[0];
	assert_eq!(hit.path, "day1/harbour.mp4");
	assert_eq!(hit.kind, TextKind::Scene);
	let time = hit.time.expect("a scene hit carries the shot's range");
	assert_eq!((time.start_seconds, time.end_seconds), (4.0, 10.0));
	assert!(hit
		.snippet
		.iter()
		.any(|part| part.matched && part.text == "lighthouse"));
	let first = search(&harness, &root, "tugboat").await;
	let time = first.hits[0].time.unwrap();
	assert_eq!((time.start_seconds, time.end_seconds), (0.0, 4.0));

	// A rewritten sidecar replaces what the last one said.
	write_json(&scene_path, &scene("A ferry crossing the bay")).await;
	index_sidecar(&harness.library, VIDEOS_EXTENSION, "scene", content_uuid)
		.await
		.unwrap();
	assert!(search(&harness, &root, "lighthouse").await.hits.is_empty());
	assert_eq!(search(&harness, &root, "ferry").await.hits.len(), 1);

	// A deleted sidecar drops every hit it gave.
	tokio::fs::remove_file(&scene_path).await.unwrap();
	let indexed = index_sidecar(&harness.library, VIDEOS_EXTENSION, "scene", content_uuid)
		.await
		.unwrap();
	assert_eq!(indexed, 0);
	assert!(search(&harness, &root, "ferry").await.hits.is_empty());
	assert!(search(&harness, &root, "tugboat").await.hits.is_empty());

	harness.shutdown().await.unwrap();
}
