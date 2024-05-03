use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Serialize, Deserialize, Type)]
pub struct Metadata {
	pub album: Option<String>,
	pub album_artist: Option<String>,
	pub artist: Option<String>,
	pub comment: Option<String>,
	pub composer: Option<String>,
	pub copyright: Option<String>,
	pub creation_time: Option<DateTime<Utc>>,
	pub date: Option<DateTime<Utc>>,
	pub disc: Option<u32>,
	pub encoder: Option<String>,
	pub encoded_by: Option<String>,
	pub filename: Option<String>,
	pub genre: Option<String>,
	pub language: Option<String>,
	pub performer: Option<String>,
	pub publisher: Option<String>,
	pub service_name: Option<String>,
	pub service_provider: Option<String>,
	pub title: Option<String>,
	pub track: Option<u32>,
	pub variant_bit_rate: Option<u32>,
	pub custom: HashMap<String, String>,
}
