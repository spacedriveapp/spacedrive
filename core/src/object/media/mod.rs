use sd_core_prisma_helpers::object_with_media_data;
use sd_media_metadata::{
	ffmpeg::{
		audio_props::AudioProps,
		chapter::Chapter,
		codec::{Codec, Props},
		program::Program,
		stream::Stream,
		video_props::VideoProps,
	},
	ExifMetadata, FFmpegMetadata,
};
use sd_prisma::prisma::{
	exif_data::*, ffmpeg_media_audio_props, ffmpeg_media_chapter, ffmpeg_media_video_props,
};

pub mod exif_metadata_extractor;
pub mod ffmpeg_metadata_extractor;
pub mod old_media_processor;
pub mod old_thumbnail;

pub use old_media_processor::OldMediaProcessorJobInit;
use sd_utils::db::ffmpeg_data_field_from_db;

pub fn exif_data_image_to_query(mdi: ExifMetadata, object_id: object_id::Type) -> CreateUnchecked {
	CreateUnchecked {
		object_id,
		_params: vec![
			camera_data::set(serde_json::to_vec(&mdi.camera_data).ok()),
			media_date::set(serde_json::to_vec(&mdi.date_taken).ok()),
			resolution::set(serde_json::to_vec(&mdi.resolution).ok()),
			media_location::set(serde_json::to_vec(&mdi.location).ok()),
			artist::set(mdi.artist),
			description::set(mdi.description),
			copyright::set(mdi.copyright),
			exif_version::set(mdi.exif_version),
			epoch_time::set(mdi.date_taken.map(|x| x.unix_timestamp())),
		],
	}
}

pub fn exif_data_image_to_query_params(
	mdi: ExifMetadata,
) -> (Vec<(&'static str, rmpv::Value)>, Vec<SetParam>) {
	use sd_sync::option_sync_db_entry;
	use sd_utils::chain_optional_iter;

	chain_optional_iter(
		[],
		[
			option_sync_db_entry!(serde_json::to_vec(&mdi.camera_data).ok(), camera_data),
			option_sync_db_entry!(serde_json::to_vec(&mdi.date_taken).ok(), media_date),
			option_sync_db_entry!(serde_json::to_vec(&mdi.location).ok(), media_location),
			option_sync_db_entry!(mdi.artist, artist),
			option_sync_db_entry!(mdi.description, description),
			option_sync_db_entry!(mdi.copyright, copyright),
			option_sync_db_entry!(mdi.exif_version, exif_version),
		],
	)
	.into_iter()
	.unzip()
}

pub fn exif_media_data_from_prisma_data(data: sd_prisma::prisma::exif_data::Data) -> ExifMetadata {
	ExifMetadata {
		camera_data: from_slice_option_to_option(data.camera_data).unwrap_or_default(),
		date_taken: from_slice_option_to_option(data.media_date).unwrap_or_default(),
		resolution: from_slice_option_to_option(data.resolution).unwrap_or_default(),
		location: from_slice_option_to_option(data.media_location),
		artist: data.artist,
		description: data.description,
		copyright: data.copyright,
		exif_version: data.exif_version,
	}
}

pub fn ffmpeg_data_from_prisma_data(
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
		duration: duration.map(|duration| {
			let duration = ffmpeg_data_field_from_db(&duration);
			((duration >> 32) as i32, duration as i32)
		}),
		start_time: start_time.map(|start_time| {
			let start_time = ffmpeg_data_field_from_db(&start_time);
			((start_time >> 32) as i32, start_time as i32)
		}),
		bit_rate,
		chapters: chapters
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
					start: {
						let start = ffmpeg_data_field_from_db(&start);
						((start >> 32) as i32, start as i32)
					},
					end: {
						let end = ffmpeg_data_field_from_db(&end);
						((end >> 32) as i32, end as i32)
					},
					time_base_den,
					time_base_num,
					metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
				},
			)
			.collect(),
		programs: programs
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
					streams: streams
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
								codec: codec.map(
									|object_with_media_data::ffmpeg_data::programs::streams::codec::Data{
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
									}
								),
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
						.collect(),
					metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
				},
			)
			.collect(),
		metadata: from_slice_option_to_option(metadata).unwrap_or_default(),
	}
}

#[must_use]
fn from_slice_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_slice(&x).ok())
		.unwrap_or_default()
}
