use chrono::TimeDelta;
use std::collections::HashMap;

#[derive(Default)]
pub struct MediaMetadata {
	pub album: Option<String>,
	pub album_artist: Option<String>,
	pub artist: Option<String>,
	pub comment: Option<String>,
	pub composer: Option<String>,
	pub copyright: Option<String>,
	pub creation_time: Option<chrono::DateTime<chrono::Utc>>,
	pub date: Option<chrono::DateTime<chrono::Utc>>,
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
	pub variant_bitrate: Option<u32>,
	pub custom: HashMap<String, String>,
}

pub struct MediaChapter {
	pub id: u32,
	pub start: f64,
	pub end: f64,
	pub metadata: MediaMetadata,
}

pub struct MediaVideoProps {
	pub pixel_format: Option<String>,
	pub color_range: Option<String>,
	pub bits_per_channel: Option<i32>,
	pub color_space: Option<String>,
	pub color_primaries: Option<String>,
	pub color_transfer: Option<String>,
	pub field_order: Option<String>,
	pub chroma_location: Option<String>,
	pub width: i32,
	pub height: i32,
	pub aspect_ratio_num: Option<i32>,
	pub aspect_ratio_den: Option<i32>,
	pub properties: Vec<String>,
}

pub struct MediaAudioProps {
	pub delay: Option<i32>,
	pub padding: Option<i32>,
	pub sample_rate: Option<i32>,
	pub sample_format: Option<String>,
	pub bit_per_sample: Option<i32>,
	pub channel_layout: Option<String>,
}

pub struct MediaSubtitleProps {
	pub width: Option<i32>,
	pub height: Option<i32>,
}

pub enum Props {
	MediaVideoProps,
	MediaAudioProps,
	MediaSubtitleProps,
}

pub struct MediaCodec {
	pub kind: Option<String>,
	pub subkind: Option<String>,
	pub name: Option<String>,
	pub profile: Option<String>,
	pub tag: Option<String>,
	pub bit_rate: Option<i64>,
	pub props: Option<Props>,
}

pub struct MediaStream {
	pub id: u32,
	pub name: Option<String>,
	pub codec: Option<MediaCodec>,
	pub aspect_ratio_num: i32,
	pub aspect_ratio_den: i32,
	pub frames_per_second_num: i32,
	pub frames_per_second_den: i32,
	pub time_base_real_den: i32,
	pub time_base_real_num: i32,
	pub dispositions: Vec<String>,
	pub metadata: MediaMetadata,
}

pub struct MediaProgram {
	pub id: u32,
	pub name: Option<String>,
	pub streams: Vec<MediaStream>,
	pub metadata: MediaMetadata,
}

pub struct MediaInfo {
	pub formats: Vec<String>,
	pub duration: Option<TimeDelta>,
	pub start_time: Option<TimeDelta>,
	pub bitrate: Option<i64>,
	pub chapters: Vec<MediaChapter>,
	pub programs: Vec<MediaProgram>,
	pub metadata: Option<MediaMetadata>,
}
