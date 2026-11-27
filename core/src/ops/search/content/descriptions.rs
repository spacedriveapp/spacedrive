//! # Descriptions in content search
//!
//! Extensions describe what content shows in JSON sidecars: the Videos
//! extension writes a `scene` sidecar with a description of each shot's
//! keyframe, and the Photos extension writes a `caption` sidecar per photo.
//! This module copies their text into the content text index of every
//! source that holds the bytes, so `search.content` finds a file by what it
//! shows. A shot keeps its in and out points from the shot list, so a scene
//! hit carries the range the way a transcript hit carries the time words
//! were said.
//!
//! The host calls [`index_sidecar`] after it writes or deletes an extension
//! sidecar. Each kind is indexed only in a build with the feature that makes
//! its descriptions: `scene` with `video-shots`, `caption` with
//! `ai-cloud`, which every build that captions has.

use std::io;

use serde::Deserialize;
use uuid::Uuid;

use crate::{
	domain::sidecar::{ContentSidecar, SidecarFormat, SidecarKind, SidecarVariant},
	library::Library,
	service::sidecars::{SidecarFinder, SidecarRegistry},
};

/// The extension that writes `scene` sidecars.
pub const VIDEOS_EXTENSION: &str = "com.spacedrive.videos";

/// The extension that writes `caption` sidecars.
pub const PHOTOS_EXTENSION: &str = "com.spacedrive.photos";

/// The index kind an extension sidecar's text is filed under, or `None`
/// when this build does not index that sidecar.
pub fn indexed_kind(extension_id: &str, kind: &str) -> Option<&'static str> {
	match (extension_id, kind) {
		#[cfg(feature = "video-shots")]
		(VIDEOS_EXTENSION, "scene") => Some(sd_store::SCENE_KIND),
		#[cfg(feature = "ai-cloud")]
		(PHOTOS_EXTENSION, "caption") => Some(sd_store::CAPTION_KIND),
		_ => None,
	}
}

/// Bring the index in line with an extension sidecar as it is on disk now.
///
/// A sidecar that exists replaces what the index held for it; one that is
/// gone, or says nothing, drops it. Sources that hold no file with the
/// content get nothing, so a description lands only where search can reach
/// it. Returns how many sources now hold the description.
pub async fn index_sidecar(
	library: &Library,
	extension_id: &str,
	kind: &str,
	content_uuid: Uuid,
) -> anyhow::Result<usize> {
	let Some(index_kind) = indexed_kind(extension_id, kind) else {
		return Ok(0);
	};
	let sidecar = ContentSidecar {
		content_uuid,
		kind: SidecarKind::extension(extension_id, kind).map_err(anyhow::Error::msg)?,
		variant: SidecarVariant::new("default"),
		format: SidecarFormat::Json,
	};
	let finder = SidecarRegistry::of(library).finder();
	let read = match finder.find(&sidecar) {
		Some(path) => tokio::fs::read(path).await,
		None => Err(io::ErrorKind::NotFound.into()),
	};
	let spans = match read {
		Ok(bytes) => spans_of(&finder, index_kind, content_uuid, &bytes).await?,
		Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
		Err(error) => return Err(error.into()),
	};
	let doc = sd_store::DescriptionDoc {
		content_uuid,
		kind: index_kind.to_string(),
		spans,
	};

	let volume_index = library.core_context().volume_index();
	let mut written = 0;
	for source in volume_index.sources_of(library.id()) {
		let Some(store) = volume_index.store_for(&source.root).await else {
			continue;
		};
		let index = store.text_index().await?;
		if doc.spans.is_empty() || !index.has_content(content_uuid).await? {
			index.forget_descriptions(content_uuid, index_kind).await?;
			continue;
		}
		index.write_descriptions(&doc).await?;
		crate::ops::indexing::semantic_index::index_vectors_after_sidecars(library, &source.root);
		written += 1;
	}
	Ok(written)
}

async fn spans_of(
	finder: &SidecarFinder,
	index_kind: &str,
	content_uuid: Uuid,
	bytes: &[u8],
) -> anyhow::Result<Vec<sd_store::DescriptionSpan>> {
	match index_kind {
		#[cfg(feature = "video-shots")]
		sd_store::SCENE_KIND => scene_spans(finder, content_uuid, bytes).await,
		#[cfg(feature = "ai-cloud")]
		sd_store::CAPTION_KIND => {
			let caption: Caption = serde_json::from_slice(bytes)?;
			Ok(vec![sd_store::DescriptionSpan {
				start_ms: None,
				end_ms: None,
				text: caption.text,
			}])
		}
		_ => {
			let _ = (finder, content_uuid, bytes);
			Ok(Vec::new())
		}
	}
}

/// The fields of the Videos extension's `scene` sidecar this reads.
#[cfg(feature = "video-shots")]
#[derive(Deserialize)]
struct SceneDoc {
	#[serde(default)]
	shots: Vec<ShotScene>,
}

#[cfg(feature = "video-shots")]
#[derive(Deserialize)]
struct ShotScene {
	shot_id: String,
	description: Option<String>,
}

/// The fields of the Photos extension's `caption` sidecar this reads.
#[cfg(feature = "ai-cloud")]
#[derive(Deserialize)]
struct Caption {
	text: String,
}

/// One span per described shot, timed from the shot list. A shot the list
/// no longer names, or a video whose list is missing, is still found, just
/// without a range.
#[cfg(feature = "video-shots")]
async fn scene_spans(
	finder: &SidecarFinder,
	content_uuid: Uuid,
	bytes: &[u8],
) -> anyhow::Result<Vec<sd_store::DescriptionSpan>> {
	use std::collections::HashMap;

	let scene: SceneDoc = serde_json::from_slice(bytes)?;
	let ranges: HashMap<String, (i64, i64)> =
		crate::ops::media::shots::read_shots(finder, content_uuid)
			.await
			.unwrap_or_else(|error| {
				tracing::warn!(%content_uuid, %error, "scene descriptions indexed without shot times");
				None
			})
			.map(|list| {
				list.shots
					.iter()
					.map(|shot| {
						(
							shot.id.to_string(),
							(
								(shot.in_point.seconds * 1000.0).round() as i64,
								(shot.out_point.seconds * 1000.0).round() as i64,
							),
						)
					})
					.collect()
			})
			.unwrap_or_default();
	Ok(scene
		.shots
		.into_iter()
		.filter_map(|shot| {
			let range = ranges.get(&shot.shot_id);
			Some(sd_store::DescriptionSpan {
				start_ms: range.map(|(start, _)| *start),
				end_ms: range.map(|(_, end)| *end),
				text: shot.description?,
			})
		})
		.collect())
}
