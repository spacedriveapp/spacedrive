use crate::old_job::JobRunErrors;

use prisma_client_rust::QueryError;
use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

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
	ffmpeg_media_program, ffmpeg_media_stream, ffmpeg_media_video_props, location, object,
	PrismaClient,
};
use sd_utils::db::ffmpeg_data_field_to_db;

use std::{
	collections::{HashMap, HashSet},
	path::Path,
};

use futures_concurrency::future::{Join, TryJoin};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum FFmpegDataError {
	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	MediaData(#[from] sd_media_metadata::Error),
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct OldFFmpegDataExtractorMetadata {
	pub extracted: u32,
	pub skipped: u32,
}

pub(super) static FILTERED_AUDIO_AND_VIDEO_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_AUDIO_EXTENSIONS
		.iter()
		.copied()
		.filter(can_extract_ffmpeg_data_for_audio)
		.map(Extension::Audio)
		.chain(
			ALL_VIDEO_EXTENSIONS
				.iter()
				.copied()
				.filter(can_extract_ffmpeg_data_for_video)
				.map(Extension::Video),
		)
		.collect()
});

pub const fn can_extract_ffmpeg_data_for_audio(audio_extension: &AudioExtension) -> bool {
	use AudioExtension::*;
	// TODO: Remove from here any extension which ffmpeg can't extract metadata from
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

pub const fn can_extract_ffmpeg_data_for_video(video_extension: &VideoExtension) -> bool {
	use VideoExtension::*;
	// TODO: Remove from here any extension which ffmpeg can't extract metadata from
	matches!(
		video_extension,
		Avi | Avifs
			| Qt | Mov | Swf
			| Mjpeg | Ts | Mts
			| Mpeg | Mxf | M2v
			| Mpg | Mpe | M2ts
			| Flv | Wm | _3gp
			| M4v | Wmv | Asf
			| Mp4 | Webm | Mkv
			| Vob | Ogv | Wtv
			| Hevc | F4v
	)
}

pub async fn extract_ffmpeg_data(
	path: impl AsRef<Path> + Send,
) -> Result<FFmpegMetadata, FFmpegDataError> {
	FFmpegMetadata::from_path(path).await.map_err(Into::into)
}

pub async fn process(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(OldFFmpegDataExtractorMetadata, JobRunErrors), FFmpegDataError> {
	let mut run_metadata = OldFFmpegDataExtractorMetadata::default();
	if files_paths.is_empty() {
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let location_path = location_path.as_ref();

	let objects_already_with_ffmpeg_data = db
		.ffmpeg_data()
		.find_many(vec![ffmpeg_data::object_id::in_vec(
			files_paths
				.iter()
				.filter_map(|file_path| file_path.object_id)
				.collect(),
		)])
		.select(ffmpeg_data::select!({ object_id }))
		.exec()
		.await?;

	if files_paths.len() == objects_already_with_ffmpeg_data.len() {
		// All files already have media data, skipping
		run_metadata.skipped = files_paths.len() as u32;
		return Ok((run_metadata, JobRunErrors::default()));
	}

	let objects_already_with_ffmpeg_data = objects_already_with_ffmpeg_data
		.into_iter()
		.map(|ffmpeg_data| ffmpeg_data.object_id)
		.collect::<HashSet<_>>();

	run_metadata.skipped = objects_already_with_ffmpeg_data.len() as u32;

	let mut errors = vec![];

	let ffmpeg_datas = files_paths
		.iter()
		.enumerate()
		.filter_map(|(idx, file_path)| {
			file_path.object_id.and_then(|object_id| {
				(!objects_already_with_ffmpeg_data.contains(&object_id))
					.then_some((idx, file_path, object_id))
			})
		})
		.filter_map(|(idx, file_path, object_id)| {
			IsolatedFilePathData::try_from((location_id, file_path))
				.map_err(|e| error!("{e:#?}"))
				.ok()
				.map(|iso_file_path| (idx, location_path.join(iso_file_path), object_id))
		})
		.map(|(idx, path, object_id)| async move {
			let res = extract_ffmpeg_data(&path).await;
			ctx_update_fn(idx + 1);
			(res, path, object_id)
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|(res, path, object_id)| {
			res.map(|ffmpeg_data| (ffmpeg_data, object_id))
				.map_err(|e| errors.push((e, path)))
				.ok()
		})
		.collect::<Vec<_>>();

	let created = save_ffmpeg_data(ffmpeg_datas, db).await?;

	run_metadata.extracted = created as u32;
	run_metadata.skipped += errors.len() as u32;

	Ok((
		run_metadata,
		errors
			.into_iter()
			.map(|(e, path)| format!("Couldn't process file: \"{}\"; Error: {e}", path.display()))
			.collect::<Vec<_>>()
			.into(),
	))
}

pub async fn save_ffmpeg_data(
	ffmpeg_datas: impl IntoIterator<Item = (FFmpegMetadata, object::id::Type)>,
	db: &PrismaClient,
) -> Result<u32, QueryError> {
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
		.map(|created| created.len() as u32)
}

async fn create_ffmpeg_data(
	formats: Vec<String>,
	bit_rate: (u32, u32),
	duration: Option<(u32, u32)>,
	start_time: Option<(u32, u32)>,
	metadata: Metadata,
	object_id: i32,
	db: &PrismaClient,
) -> Result<ffmpeg_data::id::Type, QueryError> {
	db.ffmpeg_data()
		.create(
			formats.join(","),
			ffmpeg_data_field_to_db((bit_rate.0 as i64) << 32 | bit_rate.1 as i64),
			object::id::equals(object_id),
			vec![
				ffmpeg_data::duration::set(
					duration.map(|(a, b)| ffmpeg_data_field_to_db((a as i64) << 32 | b as i64)),
				),
				ffmpeg_data::start_time::set(
					start_time.map(|(a, b)| ffmpeg_data_field_to_db((a as i64) << 32 | b as i64)),
				),
				ffmpeg_data::metadata::set(
					serde_json::to_vec(&metadata)
						.map_err(|err| {
							error!("Error reading FFmpegData metadata: {err:#?}");
							err
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
							(start_high as i64) << 32 | start_low as i64,
						),
						end: ffmpeg_data_field_to_db((end_high as i64) << 32 | end_low as i64),
						time_base_den,
						time_base_num,
						ffmpeg_data_id,
						_params: vec![ffmpeg_media_chapter::metadata::set(
							serde_json::to_vec(&metadata)
								.map_err(|err| {
									error!("Error reading FFmpegMediaChapter metadata: {err:#?}");
									err
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
	let (creates, streams_by_program_id) =
		programs
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
								ffmpeg_media_program::name::set(name.clone()),
								ffmpeg_media_program::metadata::set(
									serde_json::to_vec(&metadata)
										.map_err(|err| {
											error!("Error reading FFmpegMediaProgram metadata: {err:#?}");
											err
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
										.map_err(|err| {
											error!("Error reading FFmpegMediaStream metadata: {err:#?}");
											err
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

	let created_ids = db
		._batch(creates.into_iter().map(
			|ffmpeg_media_codec::CreateUnchecked {
			     bit_rate,
			     stream_id,
			     program_id,
			     ffmpeg_data_id,
			     _params,
			 }| {
				db.ffmpeg_media_codec()
					.create_unchecked(bit_rate, stream_id, program_id, ffmpeg_data_id, _params)
					.select(ffmpeg_media_codec::select!({ id }))
			},
		))
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
