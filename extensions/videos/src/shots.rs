//! The core's media sidecars as this extension reads them.

use serde::Deserialize;
use spacedrive_sdk::prelude::*;

/// The core's shot list for one video (`shots/default.json`, version 1).
/// Times are frames with their seconds and timecode beside them; this
/// extension reads the seconds.
#[derive(Debug, Clone, Deserialize)]
pub struct ShotList {
	pub duration: Option<Moment>,
	pub shots: Vec<Shot>,
}

/// One shot. The id is stable across reruns that find the same cut, which
/// is what lets descriptions, faces and names stay attached to it. The
/// keyframe image is the `keyframe` sidecar whose variant is the shot id.
#[derive(Debug, Clone, Deserialize)]
pub struct Shot {
	pub id: String,
	#[serde(default)]
	pub index: u32,
	#[serde(rename = "in")]
	pub start: Moment,
	/// Exclusive: the next shot's `in`.
	#[serde(rename = "out")]
	pub end: Moment,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Moment {
	pub seconds: f64,
}

impl Moment {
	pub fn ms(&self) -> u64 {
		(self.seconds * 1000.0).round().max(0.0) as u64
	}
}

/// What reading a shot list can come back with, short of an error.
pub enum ShotListRead {
	Found(ShotList),
	/// The core has not split this video yet.
	Missing,
	/// This core writes no shot lists at all.
	Unsupported(String),
}

pub async fn read_shot_list(ctx: &JobContext, media_uuid: Uuid) -> Result<ShotListRead> {
	match ctx
		.vdfs()
		.read_media_sidecar(media_uuid, "shots", "default", "json")
		.await
	{
		Ok(bytes) => serde_json::from_slice(&bytes)
			.map(ShotListRead::Found)
			.map_err(|e| Error::Deserialization(format!("shot list: {e}"))),
		Err(Error::NotFound) => Ok(ShotListRead::Missing),
		Err(Error::NotAvailable(reason)) => Ok(ShotListRead::Unsupported(reason)),
		Err(e) => Err(e),
	}
}

pub async fn read_keyframe(ctx: &JobContext, media_uuid: Uuid, shot: &Shot) -> Result<Vec<u8>> {
	ctx.vdfs()
		.read_media_sidecar(media_uuid, "keyframe", &shot.id, "jpg")
		.await
}
