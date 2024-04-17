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
	pub disc: Option<i32>,
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
	pub track: Option<i32>,
	pub variant_bitrate: Option<i32>,
	pub custom: HashMap<String, String>,
}

pub struct MediaChapter {
	pub id: i32,
	pub start: Option<i64>,
	pub end: Option<i64>,
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
	pub coded_width: Option<i32>,
	pub coded_height: Option<i32>,
	pub aspect_ratio_num: Option<i32>,
	pub aspect_ratio_den: Option<i32>,
	pub properties: Option<String>,
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

enum Props {
	MediaVideoProps,
	MediaAudioProps,
	MediaSubtitleProps,
}

pub struct MediaCodec {
	pub kind: Option<String>,
	pub tag: Option<String>,
	pub name: Option<String>,
	pub profile: Option<String>,
	pub bit_rate: Option<i64>,
	pub props: Option<Props>,
}

pub struct MediaStream {
	pub id: i32,
	pub name: Option<String>,
	pub codec: Option<MediaCodec>,
	pub aspect_ratio_num: Option<i32>,
	pub aspect_ratio_den: Option<i32>,
	pub frames_per_second_num: Option<i32>,
	pub frames_per_second_den: Option<i32>,
	pub time_base_real_den: Option<i32>,
	pub time_base_real_num: Option<i32>,
	pub dispositions: Option<String>,
	pub metadata: MediaMetadata,
}

pub struct MediaProgram {
	pub id: i32,
	pub name: Option<String>,
	pub streams: Vec<MediaStream>,
	pub metadata: MediaMetadata,
}

pub struct MediaInfo {
	pub formats: Option<Vec<String>>,
	pub duration: Option<i64>,
	pub start_time: Option<i64>,
	pub bitrate: Option<i64>,
	pub chapters: Vec<MediaChapter>,
	pub programs: Vec<MediaProgram>,
	pub metadata: Option<MediaMetadata>,
}
