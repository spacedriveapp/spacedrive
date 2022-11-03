use std::{
	ffi::OsStr,
	path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
#[cfg(feature = "ffmpeg")]
use ffmpeg_next::{codec::context::Context, format, media::Type};
use tracing::{error, info};

#[derive(Default, Debug)]
pub struct MediaItem {
	pub created_at: Option<String>,
	pub brand: Option<String>,
	pub model: Option<String>,
	pub duration_seconds: f64,
	pub best_video_stream_index: usize,
	pub best_audio_stream_index: usize,
	pub best_subtitle_stream_index: usize,
	pub steams: Vec<Stream>,
}

#[derive(Debug)]
pub struct Stream {
	pub codec: String,
	pub frames: f64,
	pub duration_seconds: f64,
	pub kind: Option<StreamKind>,
}

#[derive(Debug)]
#[allow(dead_code)] // TODO: Remove this when we start using ffmpeg
pub enum StreamKind {
	Video(VideoStream),
	Audio(AudioStream),
}

#[derive(Debug)]
pub struct VideoStream {
	pub width: u32,
	pub height: u32,
	pub aspect_ratio: String,
	#[cfg(feature = "ffmpeg")]
	pub format: format::Pixel,
	pub bitrate: usize,
}

#[derive(Debug)]
pub struct AudioStream {
	pub channels: u16,
	#[cfg(feature = "ffmpeg")]
	pub format: format::Sample,
	pub bitrate: usize,
	pub rate: u32,
}

#[cfg(feature = "ffmpeg")]
fn extract(iter: &mut ffmpeg_next::dictionary::Iter, key: &str) -> Option<String> {
	iter.find(|k| k.0.contains(key)).map(|k| k.1.to_string())
}

#[cfg(feature = "ffmpeg")]
pub fn get_video_metadata(path: &PathBuf) -> Result<(), ffmpeg_next::Error> {
	ffmpeg_next::init().unwrap();

	let mut name = path
		.file_name()
		.and_then(OsStr::to_str)
		.map(ToString::to_string)
		.unwrap_or(String::new());

	// strip to exact potential date length and attempt to parse
	name = name.chars().take(19).collect();
	// specifically OBS uses this format for time, other checks could be added
	let potential_date = NaiveDateTime::parse_from_str(&name, "%Y-%m-%d %H-%M-%S");

	match format::input(&path) {
		Ok(context) => {
			let mut media_item = MediaItem::default();
			let metadata = context.metadata();
			let mut iter = metadata.iter();

			// creation_time is usually the creation date of the file
			media_item.created_at = extract(&mut iter, "creation_time");
			// apple photos use "com.apple.quicktime.creationdate", which we care more about than the creation_time
			media_item.created_at = extract(&mut iter, "creationdate");
			// fallback to potential time if exists
			if media_item.created_at.is_none() {
				media_item.created_at = potential_date.map(|d| d.to_string()).ok();
			}
			// origin metadata
			media_item.brand = extract(&mut iter, "major_brand");
			media_item.brand = extract(&mut iter, "make");
			media_item.model = extract(&mut iter, "model");

			if let Some(stream) = context.streams().best(Type::Video) {
				media_item.best_video_stream_index = stream.index();
			}
			if let Some(stream) = context.streams().best(Type::Audio) {
				media_item.best_audio_stream_index = stream.index();
			}
			if let Some(stream) = context.streams().best(Type::Subtitle) {
				media_item.best_subtitle_stream_index = stream.index();
			}
			media_item.duration_seconds =
				context.duration() as f64 / f64::from(ffmpeg_next::ffi::AV_TIME_BASE);

			for stream in context.streams() {
				let codec = Context::from_parameters(stream.parameters())?;

				let mut stream_item = Stream {
					codec: codec.id().name().to_string(),
					frames: stream.frames() as f64,
					duration_seconds: stream.duration() as f64 * f64::from(stream.time_base()),
					kind: None,
				};

				if codec.medium() == ffmpeg_next::media::Type::Video {
					if let Ok(video) = codec.decoder().video() {
						stream_item.kind = Some(StreamKind::Video(VideoStream {
							bitrate: video.bit_rate(),
							format: video.format(),
							width: video.width(),
							height: video.height(),
							aspect_ratio: video.aspect_ratio().to_string(),
						}));
					}
				} else if codec.medium() == ffmpeg_next::media::Type::Audio {
					if let Ok(audio) = codec.decoder().audio() {
						stream_item.kind = Some(StreamKind::Audio(AudioStream {
							channels: audio.channels(),
							bitrate: audio.bit_rate(),
							rate: audio.rate(),
							format: audio.format(),
						}));
					}
				}
				media_item.steams.push(stream_item);
			}
			info!("{:#?}", media_item);
		}

		Err(error) => error!("error: {}", error),
	}
	Ok(())
}
