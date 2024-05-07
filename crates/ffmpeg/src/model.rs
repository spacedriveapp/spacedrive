use std::collections::HashMap;

use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct FFmpegMediaData {
	pub formats: Vec<String>,
	pub duration: Option<i64>,
	pub start_time: Option<i64>,
	pub bit_rate: i64,
	pub chapters: Vec<FFmpegChapter>,
	pub programs: Vec<FFmpegProgram>,
	pub metadata: FFmpegMetadata,
}

#[derive(Debug)]
pub struct FFmpegChapter {
	pub id: i64,
	pub start: i64,
	pub end: i64,
	pub time_base_den: i32,
	pub time_base_num: i32,
	pub metadata: FFmpegMetadata,
}

#[derive(Default, Debug)]
pub struct FFmpegMetadata {
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

#[derive(Debug)]
pub struct FFmpegProgram {
	pub id: i32,
	pub name: Option<String>,
	pub streams: Vec<FFmpegStream>,
	pub metadata: FFmpegMetadata,
}

#[derive(Debug)]
pub struct FFmpegStream {
	pub id: i32,
	pub name: Option<String>,
	pub codec: Option<FFmpegCodec>,
	pub aspect_ratio_num: i32,
	pub aspect_ratio_den: i32,
	pub frames_per_second_num: i32,
	pub frames_per_second_den: i32,
	pub time_base_real_den: i32,
	pub time_base_real_num: i32,
	pub dispositions: Vec<String>,
	pub metadata: FFmpegMetadata,
}

#[derive(Debug)]
pub struct FFmpegCodec {
	pub kind: Option<String>,
	pub sub_kind: Option<String>,
	pub tag: Option<String>,
	pub name: Option<String>,
	pub profile: Option<String>,
	pub bit_rate: i32,
	pub props: Option<FFmpegProps>,
}

#[derive(Debug)]
pub enum FFmpegProps {
	Video(FFmpegVideoProps),
	Audio(FFmpegAudioProps),
	Subtitle(FFmpegSubtitleProps),
}

#[derive(Debug)]
pub struct FFmpegVideoProps {
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

#[derive(Debug)]
pub struct FFmpegAudioProps {
	pub delay: i32,
	pub padding: i32,
	pub sample_rate: Option<i32>,
	pub sample_format: Option<String>,
	pub bit_per_sample: Option<i32>,
	pub channel_layout: Option<String>,
}

#[derive(Debug)]
pub struct FFmpegSubtitleProps {
	pub width: i32,
	pub height: i32,
}
