use serde::{Deserialize, Serialize};
use specta::Type;

use super::{audio_props::AudioProps, subtitle_props::SubtitleProps, video_props::VideoProps};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct Codec {
	pub kind: Option<String>,
	pub sub_kind: Option<String>,
	pub tag: Option<String>,
	pub name: Option<String>,
	pub profile: Option<String>,
	pub bit_rate: i32,
	pub props: Option<Props>,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub enum Props {
	Video(VideoProps),
	Audio(AudioProps),
	Subtitle(SubtitleProps),
}
