use crate::Result;

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

pub mod audio_props;
pub mod chapter;
pub mod codec;
pub mod metadata;
pub mod program;
pub mod stream;
pub mod subtitle_props;
pub mod video_props;

use chapter::Chapter;
use metadata::Metadata;
use program::Program;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct FFmpegMetadata {
	pub formats: Vec<String>,
	pub duration: Option<(i32, u32)>,
	pub start_time: Option<(i32, u32)>,
	pub bit_rate: (i32, u32),
	pub chapters: Vec<Chapter>,
	pub programs: Vec<Program>,
	pub metadata: Metadata,
}

impl FFmpegMetadata {
	pub async fn from_path(path: impl AsRef<Path> + Send) -> Result<Self> {
		#[cfg(not(feature = "ffmpeg"))]
		{
			let _ = path;
			Err(crate::Error::NoFFmpeg)
		}

		#[cfg(feature = "ffmpeg")]
		{
			sd_ffmpeg::probe(path)
				.await
				.map(Into::into)
				.map_err(Into::into)
		}
	}
}

#[cfg(feature = "ffmpeg")]
mod extract_data {

	use sd_ffmpeg::model::{
		FFmpegAudioProps, FFmpegChapter, FFmpegCodec, FFmpegMediaData, FFmpegMetadata,
		FFmpegProgram, FFmpegProps, FFmpegStream, FFmpegSubtitleProps, FFmpegVideoProps,
	};
	use sd_utils::i64_to_frontend;

	impl From<FFmpegMediaData> for super::FFmpegMetadata {
		fn from(
			FFmpegMediaData {
				formats,
				duration,
				start_time,
				bit_rate,
				chapters,
				programs,
				metadata,
			}: FFmpegMediaData,
		) -> Self {
			Self {
				formats,
				duration: duration.map(i64_to_frontend),
				start_time: start_time.map(i64_to_frontend),
				bit_rate: i64_to_frontend(bit_rate),
				chapters: chapters.into_iter().map(Into::into).collect(),
				programs: programs.into_iter().map(Into::into).collect(),
				metadata: metadata.into(),
			}
		}
	}

	impl From<FFmpegChapter> for super::Chapter {
		fn from(
			FFmpegChapter {
				id,
				start,
				end,
				time_base_den,
				time_base_num,
				metadata,
			}: FFmpegChapter,
		) -> Self {
			Self {
				id: {
					#[allow(clippy::cast_possible_truncation)]
					{
						// NOTICE: chapter.id is a i64, but I think it will be extremely rare to have a chapter id that doesn't fit in a i32
						id as i32
					}
				},
				// TODO: FIX these 2 when rspc/specta supports bigint
				start: i64_to_frontend(start),
				end: i64_to_frontend(end),
				time_base_num,
				time_base_den,
				metadata: metadata.into(),
			}
		}
	}

	impl From<FFmpegProgram> for super::Program {
		fn from(
			FFmpegProgram {
				id,
				name,
				streams,
				metadata,
			}: FFmpegProgram,
		) -> Self {
			Self {
				id,
				name,
				streams: streams.into_iter().map(Into::into).collect(),
				metadata: metadata.into(),
			}
		}
	}

	impl From<FFmpegStream> for super::stream::Stream {
		fn from(
			FFmpegStream {
				id,
				name,
				codec,
				aspect_ratio_num,
				aspect_ratio_den,
				frames_per_second_num,
				frames_per_second_den,
				time_base_real_den,
				time_base_real_num,
				dispositions,
				metadata,
			}: FFmpegStream,
		) -> Self {
			Self {
				id,
				name,
				codec: codec.map(Into::into),
				aspect_ratio_num,
				aspect_ratio_den,
				frames_per_second_num,
				frames_per_second_den,
				time_base_real_den,
				time_base_real_num,
				dispositions,
				metadata: metadata.into(),
			}
		}
	}

	impl From<FFmpegCodec> for super::codec::Codec {
		fn from(
			FFmpegCodec {
				kind,
				sub_kind,
				tag,
				name,
				profile,
				bit_rate,
				props,
			}: FFmpegCodec,
		) -> Self {
			Self {
				kind,
				sub_kind,
				tag,
				name,
				profile,
				bit_rate,
				props: props.map(Into::into),
			}
		}
	}

	impl From<FFmpegProps> for super::codec::Props {
		fn from(props: FFmpegProps) -> Self {
			match props {
				FFmpegProps::Video(video_props) => Self::Video(video_props.into()),
				FFmpegProps::Audio(audio_props) => Self::Audio(audio_props.into()),
				FFmpegProps::Subtitle(subtitle_props) => Self::Subtitle(subtitle_props.into()),
			}
		}
	}

	impl From<FFmpegAudioProps> for super::audio_props::AudioProps {
		fn from(
			FFmpegAudioProps {
				delay,
				padding,
				sample_rate,
				sample_format,
				bit_per_sample,
				channel_layout,
			}: FFmpegAudioProps,
		) -> Self {
			Self {
				delay,
				padding,
				sample_rate,
				sample_format,
				bit_per_sample,
				channel_layout,
			}
		}
	}

	impl From<FFmpegSubtitleProps> for super::subtitle_props::SubtitleProps {
		fn from(FFmpegSubtitleProps { width, height }: FFmpegSubtitleProps) -> Self {
			Self { width, height }
		}
	}

	impl From<FFmpegVideoProps> for super::video_props::VideoProps {
		fn from(
			FFmpegVideoProps {
				pixel_format,
				color_range,
				bits_per_channel,
				color_space,
				color_primaries,
				color_transfer,
				field_order,
				chroma_location,
				width,
				height,
				aspect_ratio_num,
				aspect_ratio_den,
				properties,
			}: FFmpegVideoProps,
		) -> Self {
			Self {
				pixel_format,
				color_range,
				bits_per_channel,
				color_space,
				color_primaries,
				color_transfer,
				field_order,
				chroma_location,
				width,
				height,
				aspect_ratio_num,
				aspect_ratio_den,
				properties,
			}
		}
	}

	impl From<FFmpegMetadata> for super::Metadata {
		fn from(
			FFmpegMetadata {
				album,
				album_artist,
				artist,
				comment,
				composer,
				copyright,
				creation_time,
				date,
				disc,
				encoder,
				encoded_by,
				filename,
				genre,
				language,
				performer,
				publisher,
				service_name,
				service_provider,
				title,
				track,
				variant_bit_rate,
				custom,
			}: FFmpegMetadata,
		) -> Self {
			Self {
				album,
				album_artist,
				artist,
				comment,
				composer,
				copyright,
				creation_time,
				date,
				disc,
				encoder,
				encoded_by,
				filename,
				genre,
				language,
				performer,
				publisher,
				service_name,
				service_provider,
				title,
				track,
				variant_bit_rate,
				custom,
			}
		}
	}
}
