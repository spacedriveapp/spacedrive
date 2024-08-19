use crate::media_processor::{self, media_data_extractor};

use sd_core_prisma_helpers::object_with_media_data;

use sd_file_ext::extensions::{
	AudioExtension, Extension, VideoExtension, ALL_AUDIO_EXTENSIONS, ALL_VIDEO_EXTENSIONS,
};
use sd_media_metadata::{
	ffmpeg::{
		audio_props::AudioProps,
		chapter::Chapter,
		codec::{Codec, Props},
		metadata::Metadata,
		program::Program,
		stream::Stream,
		video_props::VideoProps,
	},
	FFmpegMetadata,
};
use sd_prisma::prisma::{
	ffmpeg_data, ffmpeg_media_audio_props, ffmpeg_media_chapter, ffmpeg_media_codec,
	ffmpeg_media_program, ffmpeg_media_stream, ffmpeg_media_video_props, object, PrismaClient,
};
use sd_utils::{
	db::{ffmpeg_data_field_from_db, ffmpeg_data_field_to_db},
	i64_to_frontend,
};

use std::{collections::HashMap, path::Path};

use futures_concurrency::future::TryJoin;
use once_cell::sync::Lazy;
use prisma_client_rust::QueryError;
use tracing::error;

use super::from_slice_option_to_option;

pub static AVAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_AUDIO_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_extract_for_audio(ext))
		.map(Extension::Audio)
		.chain(
			ALL_VIDEO_EXTENSIONS
				.iter()
				.copied()
				.filter(|&ext| can_extract_for_video(ext))
				.map(Extension::Video),
		)
		.collect()
});

#[must_use]
pub const fn can_extract_for_audio(audio_extension: AudioExtension) -> bool {
	use AudioExtension::{
		Aac, Adts, Aif, Aiff, Amr, Aptx, Ast, Caf, Flac, Loas, M4a, Mid, Mp2, Mp3, Oga, Ogg, Opus,
		Tta, Voc, Wav, Wma, Wv,
	};

	matches!(
		audio_extension,
		Mp3 | Mp2
			| M4a | Wav | Aiff
			| Aif | Flac | Ogg
			| Oga | Opus | Wma
			| Amr | Aac | Wv
			| Voc | Tta | Loas
			| Caf | Aptx | Adts
			| Ast | Mid
	)
}

#[must_use]
pub const fn can_extract_for_video(video_extension: VideoExtension) -> bool {
	use VideoExtension::{
		Asf, Avi, Avifs, F4v, Flv, Hevc, M2ts, M2v, M4v, Mjpeg, Mkv, Mov, Mp4, Mpe, Mpeg, Mpg, Mxf,
		Ogv, Qt, Swf, Vob, Webm, Wm, Wmv, Wtv, _3gp,
	};

	matches!(
		video_extension,
		Avi | Avifs
			| Qt | Mov | Swf
			| Mjpeg | Mpeg
			| Mxf | M2v | Mpg
			| Mpe | M2ts | Flv
			| Wm | _3gp | M4v
			| Wmv | Asf | Mp4
			| Webm | Mkv | Vob
			| Ogv | Wtv | Hevc
			| F4v // | Ts | Mts  TODO: Uncomment when we start using magic instead of extension
	)
}

pub async fn extract(
	path: impl AsRef<Path> + Send,
) -> Result<FFmpegMetadata, media_processor::NonCriticalMediaProcessorError> {
	let path = path.as_ref();

	FFmpegMetadata::from_path(&path).await.map_err(|e| {
		media_data_extractor::NonCriticalMediaDataExtractorError::FailedToExtractImageMediaData(
			path.to_path_buf(),
			e.to_string(),
		)
		.into()
	})
}

pub async fn save(
	ffmpeg_datas: impl IntoIterator<Item = (FFmpegMetadata, object::id::Type)> + Send,
	db: &PrismaClient,
) -> Result<u64, sd_core_sync::Error> {
	ffmpeg_datas
		.into_iter()
		.map(
			move |(
				FFmpegMetadata {
					formats,
					duration,
					start_time,
					bit_rate,
					chapters,
					programs,
					metadata,
				},
				object_id,
			)| {
				db._transaction()
					.with_timeout(30 * 1000)
					.run(move |db| async move {
						let data_id = create_ffmpeg_data(
							formats, bit_rate, duration, start_time, metadata, object_id, &db,
						)
						.await?;

						create_ffmpeg_chapters(data_id, chapters, &db).await?;

						let streams = create_ffmpeg_programs(data_id, programs, &db).await?;

						let codecs = create_ffmpeg_streams(data_id, streams, &db).await?;

						let (audio_props, video_props) =
							create_ffmpeg_codecs(data_id, codecs, &db).await?;

						(
							create_ffmpeg_audio_props(audio_props, &db),
							create_ffmpeg_video_props(video_props, &db),
						)
							.try_join()
							.await
							.map(|_| ())
					})
			},
		)
		.collect::<Vec<_>>()
		.try_join()
		.await
		.map(|created| created.len() as u64)
		.map_err(Into::into)
}

async fn create_ffmpeg_data(
	formats: Vec<String>,
	(bit_rate_high, bit_rate_low): (i32, u32),
	maybe_duration: Option<(i32, u32)>,
	maybe_start_time: Option<(i32, u32)>,
	metadata: Metadata,
	object_id: i32,
	db: &PrismaClient,
) -> Result<ffmpeg_data::id::Type, QueryError> {
	db.ffmpeg_data()
		.create(
			formats.join(","),
			ffmpeg_data_field_to_db(i64::from(bit_rate_high) << 32 | i64::from(bit_rate_low)),
			object::id::equals(object_id),
			vec![
				ffmpeg_data::duration::set(maybe_duration.map(|(duration_high, duration_low)| {
					ffmpeg_data_field_to_db(
						i64::from(duration_high) << 32 | i64::from(duration_low),
					)
				})),
				ffmpeg_data::start_time::set(maybe_start_time.map(
					|(start_time_high, start_time_low)| {
						ffmpeg_data_field_to_db(
							i64::from(start_time_high) << 32 | i64::from(start_time_low),
						)
					},
				)),
				ffmpeg_data::metadata::set(
					serde_json::to_vec(&metadata)
						.map_err(|e| {
							error!(?e, "Error reading FFmpegData metadata;");
							e
						})
						.ok(),
				),
			],
		)
		.select(ffmpeg_data::select!({ id }))
		.exec()
		.await
		.map(|data| data.id)
}

async fn create_ffmpeg_chapters(
	ffmpeg_data_id: ffmpeg_data::id::Type,
	chapters: Vec<Chapter>,
	db: &PrismaClient,
) -> Result<(), QueryError> {
	db.ffmpeg_media_chapter()
		.create_many(
			chapters
				.into_iter()
				.map(
					|Chapter {
					     id: chapter_id,
					     start: (start_high, start_low),
					     end: (end_high, end_low),
					     time_base_den,
					     time_base_num,
					     metadata,
					 }| ffmpeg_media_chapter::CreateUnchecked {
						chapter_id,
						start: ffmpeg_data_field_to_db(
							i64::from(start_high) << 32 | i64::from(start_low),
						),
						end: ffmpeg_data_field_to_db(
							i64::from(end_high) << 32 | i64::from(end_low),
						),
						time_base_den,
						time_base_num,
						ffmpeg_data_id,
						_params: vec![ffmpeg_media_chapter::metadata::set(
							serde_json::to_vec(&metadata)
								.map_err(|e| {
									error!(?e, "Error reading FFmpegMediaChapter metadata;");
									e
								})
								.ok(),
						)],
					},
				)
				.collect(),
		)
		.exec()
		.await
		.map(|_| ())
}

async fn create_ffmpeg_programs(
	data_id: i32,
	programs: Vec<Program>,
	db: &PrismaClient,
) -> Result<Vec<(ffmpeg_media_program::program_id::Type, Vec<Stream>)>, QueryError> {
	let (creates, streams_by_program_id) = programs
		.into_iter()
		.map(
			|Program {
			     id: program_id,
			     name,
			     metadata,
			     streams,
			 }| {
				(
					ffmpeg_media_program::CreateUnchecked {
						program_id,
						ffmpeg_data_id: data_id,
						_params: vec![
							ffmpeg_media_program::name::set(name),
							ffmpeg_media_program::metadata::set(
								serde_json::to_vec(&metadata)
									.map_err(|e| {
										error!(?e, "Error reading FFmpegMediaProgram metadata;");
										e
									})
									.ok(),
							),
						],
					},
					(program_id, streams),
				)
			},
		)
		.unzip::<_, _, Vec<_>, Vec<_>>();

	db.ffmpeg_media_program()
		.create_many(creates)
		.exec()
		.await
		.map(|_| streams_by_program_id)
}

async fn create_ffmpeg_streams(
	ffmpeg_data_id: ffmpeg_data::id::Type,
	streams: Vec<(ffmpeg_media_program::program_id::Type, Vec<Stream>)>,
	db: &PrismaClient,
) -> Result<
	Vec<(
		ffmpeg_media_program::program_id::Type,
		ffmpeg_media_stream::stream_id::Type,
		Codec,
	)>,
	QueryError,
> {
	let (creates, maybe_codecs) = streams
		.into_iter()
		.flat_map(|(program_id, streams)| {
			streams.into_iter().map(
				move |Stream {
				          id: stream_id,
				          name,
				          codec: maybe_codec,
				          aspect_ratio_num,
				          aspect_ratio_den,
				          frames_per_second_num,
				          frames_per_second_den,
				          time_base_real_den,
				          time_base_real_num,
				          dispositions,
				          metadata,
				      }| {
					(
						ffmpeg_media_stream::CreateUnchecked {
							stream_id,
							aspect_ratio_num,
							aspect_ratio_den,
							frames_per_second_num,
							frames_per_second_den,
							time_base_real_den,
							time_base_real_num,
							program_id,
							ffmpeg_data_id,
							_params: vec![
								ffmpeg_media_stream::name::set(name),
								ffmpeg_media_stream::dispositions::set(
									(!dispositions.is_empty()).then_some(dispositions.join(",")),
								),
								ffmpeg_media_stream::title::set(metadata.title.clone()),
								ffmpeg_media_stream::encoder::set(metadata.encoder.clone()),
								ffmpeg_media_stream::language::set(metadata.language.clone()),
								ffmpeg_media_stream::metadata::set(
									serde_json::to_vec(&metadata)
										.map_err(|e| {
											error!(?e, "Error reading FFmpegMediaStream metadata;");
											e
										})
										.ok(),
								),
							],
						},
						maybe_codec.map(|codec| (program_id, stream_id, codec)),
					)
				},
			)
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	db.ffmpeg_media_stream()
		.create_many(creates)
		.exec()
		.await
		.map(|_| maybe_codecs.into_iter().flatten().collect())
}

async fn create_ffmpeg_codecs(
	ffmpeg_data_id: ffmpeg_data::id::Type,
	codecs: Vec<(
		ffmpeg_media_program::program_id::Type,
		ffmpeg_media_stream::stream_id::Type,
		Codec,
	)>,
	db: &PrismaClient,
) -> Result<
	(
		Vec<(ffmpeg_media_codec::id::Type, AudioProps)>,
		Vec<(ffmpeg_media_codec::id::Type, VideoProps)>,
	),
	QueryError,
> {
	let expected_creates = codecs.len();

	let (creates, mut audio_props, mut video_props) = codecs.into_iter().enumerate().fold(
		(
			Vec::with_capacity(expected_creates),
			HashMap::with_capacity(expected_creates),
			HashMap::with_capacity(expected_creates),
		),
		|(mut creates, mut audio_props, mut video_props),
		 (
			idx,
			(
				program_id,
				stream_id,
				Codec {
					kind,
					sub_kind,
					tag,
					name,
					profile,
					bit_rate,
					props: maybe_props,
				},
			),
		)| {
			creates.push(ffmpeg_media_codec::CreateUnchecked {
				bit_rate,
				stream_id,
				program_id,
				ffmpeg_data_id,
				_params: vec![
					ffmpeg_media_codec::kind::set(kind),
					ffmpeg_media_codec::sub_kind::set(sub_kind),
					ffmpeg_media_codec::tag::set(tag),
					ffmpeg_media_codec::name::set(name),
					ffmpeg_media_codec::profile::set(profile),
				],
			});

			if let Some(props) = maybe_props {
				match props {
					Props::Audio(props) => {
						audio_props.insert(idx, props);
					}
					Props::Video(props) => {
						video_props.insert(idx, props);
					}
					Props::Subtitle(_) => {
						// We don't care about subtitles props for now :D
					}
				}
			}

			(creates, audio_props, video_props)
		},
	);

	let created_ids = creates
		.into_iter()
		.map(
			|ffmpeg_media_codec::CreateUnchecked {
			     bit_rate,
			     stream_id,
			     program_id,
			     ffmpeg_data_id,
			     _params: params,
			 }| {
				db.ffmpeg_media_codec()
					.create_unchecked(bit_rate, stream_id, program_id, ffmpeg_data_id, params)
					.select(ffmpeg_media_codec::select!({ id }))
					.exec()
			},
		)
		.collect::<Vec<_>>()
		.try_join()
		.await?;

	assert_eq!(
		created_ids.len(),
		expected_creates,
		"Not all codecs were created and our invariant is broken!"
	);

	debug_assert!(
		created_ids
			.windows(2)
			.all(|window| window[0].id < window[1].id),
		"Codecs were created in a different order than we expected, our invariant is broken!"
	);

	Ok(created_ids.into_iter().enumerate().fold(
		(
			Vec::with_capacity(audio_props.len()),
			Vec::with_capacity(video_props.len()),
		),
		|(mut a_props, mut v_props), (idx, codec_data)| {
			if let Some(audio_props) = audio_props.remove(&idx) {
				a_props.push((codec_data.id, audio_props));
			} else if let Some(video_props) = video_props.remove(&idx) {
				v_props.push((codec_data.id, video_props));
			}

			(a_props, v_props)
		},
	))
}

async fn create_ffmpeg_audio_props(
	audio_props: Vec<(ffmpeg_media_codec::id::Type, AudioProps)>,
	db: &PrismaClient,
) -> Result<(), QueryError> {
	db.ffmpeg_media_audio_props()
		.create_many(
			audio_props
				.into_iter()
				.map(
					|(
						codec_id,
						AudioProps {
							delay,
							padding,
							sample_rate,
							sample_format,
							bit_per_sample,
							channel_layout,
						},
					)| ffmpeg_media_audio_props::CreateUnchecked {
						delay,
						padding,
						codec_id,
						_params: vec![
							ffmpeg_media_audio_props::sample_rate::set(sample_rate),
							ffmpeg_media_audio_props::sample_format::set(sample_format),
							ffmpeg_media_audio_props::bit_per_sample::set(bit_per_sample),
							ffmpeg_media_audio_props::channel_layout::set(channel_layout),
						],
					},
				)
				.collect(),
		)
		.exec()
		.await
		.map(|_| ())
}

async fn create_ffmpeg_video_props(
	video_props: Vec<(ffmpeg_media_codec::id::Type, VideoProps)>,
	db: &PrismaClient,
) -> Result<(), QueryError> {
	db.ffmpeg_media_video_props()
		.create_many(
			video_props
				.into_iter()
				.map(
					|(
						codec_id,
						VideoProps {
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
						},
					)| {
						ffmpeg_media_video_props::CreateUnchecked {
							width,
							height,
							codec_id,
							_params: vec![
								ffmpeg_media_video_props::pixel_format::set(pixel_format),
								ffmpeg_media_video_props::color_range::set(color_range),
								ffmpeg_media_video_props::bits_per_channel::set(bits_per_channel),
								ffmpeg_media_video_props::color_space::set(color_space),
								ffmpeg_media_video_props::color_primaries::set(color_primaries),
								ffmpeg_media_video_props::color_transfer::set(color_transfer),
								ffmpeg_media_video_props::field_order::set(field_order),
								ffmpeg_media_video_props::chroma_location::set(chroma_location),
								ffmpeg_media_video_props::aspect_ratio_num::set(aspect_ratio_num),
								ffmpeg_media_video_props::aspect_ratio_den::set(aspect_ratio_den),
								ffmpeg_media_video_props::properties::set(Some(
									properties.join(","),
								)),
							],
						}
					},
				)
				.collect(),
		)
		.exec()
		.await
		.map(|_| ())
}

pub fn from_prisma_data(
	object_with_media_data::ffmpeg_data::Data {
		formats,
		duration,
		start_time,
		bit_rate,
		metadata,
		chapters,
		programs,
		..
	}: object_with_media_data::ffmpeg_data::Data,
) -> FFmpegMetadata {
	FFmpegMetadata {
		formats: formats.split(',').map(String::from).collect::<Vec<_>>(),
		duration: duration.map(|duration| i64_to_frontend(ffmpeg_data_field_from_db(&duration))),
		start_time: start_time
			.map(|start_time| i64_to_frontend(ffmpeg_data_field_from_db(&start_time))),
		bit_rate: i64_to_frontend(ffmpeg_data_field_from_db(&bit_rate)),
		chapters: chapters_from_prisma_data(chapters),
		programs: programs_from_prisma_data(programs),
		metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
	}
}

#[inline]
fn chapters_from_prisma_data(chapters: Vec<ffmpeg_media_chapter::Data>) -> Vec<Chapter> {
	chapters
		.into_iter()
		.map(
			|ffmpeg_media_chapter::Data {
			     chapter_id,
			     start,
			     end,
			     time_base_den,
			     time_base_num,
			     metadata,
			     ..
			 }| Chapter {
				id: chapter_id,
				start: i64_to_frontend(ffmpeg_data_field_from_db(&start)),
				end: i64_to_frontend(ffmpeg_data_field_from_db(&end)),
				time_base_den,
				time_base_num,
				metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
			},
		)
		.collect()
}

#[inline]
fn programs_from_prisma_data(
	programs: Vec<object_with_media_data::ffmpeg_data::programs::Data>,
) -> Vec<Program> {
	programs
		.into_iter()
		.map(
			|object_with_media_data::ffmpeg_data::programs::Data {
			     program_id,
			     name,
			     metadata,
			     streams,
			     ..
			 }| Program {
				id: program_id,
				name,
				streams: streams_from_prisma_data(streams),
				metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
			},
		)
		.collect()
}

fn streams_from_prisma_data(
	streams: Vec<object_with_media_data::ffmpeg_data::programs::streams::Data>,
) -> Vec<Stream> {
	streams
		.into_iter()
		.map(
			|object_with_media_data::ffmpeg_data::programs::streams::Data {
			     stream_id,
			     name,
			     aspect_ratio_num,
			     aspect_ratio_den,
			     frames_per_second_num,
			     frames_per_second_den,
			     time_base_real_den,
			     time_base_real_num,
			     dispositions,
			     metadata,
			     codec,
			     ..
			 }| {
				Stream {
					id: stream_id,
					name,
					codec: codec_from_prisma_data(codec),
					aspect_ratio_num,
					aspect_ratio_den,
					frames_per_second_num,
					frames_per_second_den,
					time_base_real_den,
					time_base_real_num,
					dispositions: dispositions
						.map(|dispositions| {
							dispositions
								.split(',')
								.map(String::from)
								.collect::<Vec<_>>()
						})
						.unwrap_or_default(),
					metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
				}
			},
		)
		.collect()
}

fn codec_from_prisma_data(
	codec: Option<object_with_media_data::ffmpeg_data::programs::streams::codec::Data>,
) -> Option<Codec> {
	codec.map(
		|object_with_media_data::ffmpeg_data::programs::streams::codec::Data {
		     kind,
		     sub_kind,
		     tag,
		     name,
		     profile,
		     bit_rate,
		     audio_props,
		     video_props,
		     ..
		 }| Codec {
			kind,
			sub_kind,
			tag,
			name,
			profile,
			bit_rate,
			props: match (audio_props, video_props) {
				(
					Some(ffmpeg_media_audio_props::Data {
						delay,
						padding,
						sample_rate,
						sample_format,
						bit_per_sample,
						channel_layout,
						..
					}),
					None,
				) => Some(Props::Audio(AudioProps {
					delay,
					padding,
					sample_rate,
					sample_format,
					bit_per_sample,
					channel_layout,
				})),
				(
					None,
					Some(ffmpeg_media_video_props::Data {
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
						..
					}),
				) => Some(Props::Video(VideoProps {
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
					properties: properties
						.map(|dispositions| {
							dispositions
								.split(',')
								.map(String::from)
								.collect::<Vec<_>>()
						})
						.unwrap_or_default(),
				})),
				_ => None,
			},
		},
	)
}
