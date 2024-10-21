use crate::{frame_decoder::ThumbnailSize, Error, FrameDecoder};

use std::{io, ops::Deref, path::Path};

use image::{imageops, DynamicImage, RgbImage};
use sd_utils::error::FileIOError;
use tokio::{fs, io::AsyncWriteExt, task::spawn_blocking};
use tracing::error;
use webp::Encoder;

/// `Thumbnailer` struct holds data from a `ThumbnailerBuilder`, exposing methods
/// to generate thumbnails from video files.
#[derive(Debug, Clone)]
pub struct Thumbnailer {
	builder: ThumbnailerBuilder,
}

impl Thumbnailer {
	/// Processes an video input file and write to file system a thumbnail with webp format
	pub(crate) async fn process(
		&self,
		video_file_path: impl AsRef<Path> + Send,
		output_thumbnail_path: impl AsRef<Path> + Send,
	) -> Result<(), Error> {
		let output_thumbnail_path = output_thumbnail_path.as_ref();
		let path = output_thumbnail_path.parent().ok_or_else(|| {
			FileIOError::from((
				output_thumbnail_path,
				io::Error::new(
					io::ErrorKind::InvalidInput,
					"Cannot determine parent directory",
				),
			))
		})?;

		fs::create_dir_all(path)
			.await
			.map_err(|e| FileIOError::from((path, e)))?;

		let webp = self.process_to_webp_bytes(video_file_path).await?;
		let mut file = fs::File::create(output_thumbnail_path)
			.await
			.map_err(|e: io::Error| FileIOError::from((output_thumbnail_path, e)))?;

		file.write_all(&webp)
			.await
			.map_err(|e| FileIOError::from((output_thumbnail_path, e)))?;

		file.sync_all()
			.await
			.map_err(|e| FileIOError::from((output_thumbnail_path, e)).into())
	}

	/// Processes an video input file and returns a webp encoded thumbnail as bytes
	async fn process_to_webp_bytes(
		&self,
		video_file_path: impl AsRef<Path> + Send,
	) -> Result<Vec<u8>, Error> {
		let prefer_embedded_metadata = self.builder.prefer_embedded_metadata;
		let seek_percentage = self.builder.seek_percentage;
		let size = self.builder.size;
		let maintain_aspect_ratio = self.builder.maintain_aspect_ratio;
		let quality = self.builder.quality;

		spawn_blocking({
			let video_file_path = video_file_path.as_ref().to_path_buf();
			move || -> Result<Vec<u8>, Error> {
				let mut decoder = FrameDecoder::new(
					&video_file_path,
					// TODO: allow_seek should be false for remote files
					true,
					prefer_embedded_metadata,
				)?;

				// We actually have to decode a frame to get some metadata before we can start decoding for real
				decoder.decode_video_frame()?;

				if !decoder.use_embedded() {
					let result = decoder
						.get_duration_secs()
						.ok_or(Error::NoVideoDuration)
						.and_then(|duration| {
							decoder.seek(
								#[allow(clippy::cast_possible_truncation)]
								{
									// This conversion is ok because we don't worry much about precision here
									(duration * f64::from(seek_percentage)).round() as i64
								},
							)
						});

					if let Err(err) = result {
						error!(
							"Failed to seek {}: {err:#?}",
							video_file_path.to_string_lossy()
						);
						// Seeking failed, try first frame again
						// Re-instantiating decoder to avoid possible segfault
						// https://github.com/dirkvdb/ffmpegthumbnailer/commit/da292ccb51a526ebc833f851a388ca308d747289
						decoder =
							FrameDecoder::new(&video_file_path, false, prefer_embedded_metadata)?;
						decoder.decode_video_frame()?;
					}
				}

				let video_frame =
					decoder.get_scaled_video_frame(Some(size), maintain_aspect_ratio)?;

				let mut image = DynamicImage::ImageRgb8(
					RgbImage::from_raw(video_frame.width, video_frame.height, video_frame.data)
						.ok_or(Error::CorruptVideo(video_file_path.into_boxed_path()))?,
				);

				let image = if video_frame.rotation < -135.0 {
					imageops::rotate180_in_place(&mut image);
					image
				} else if video_frame.rotation > 45.0 && video_frame.rotation < 135.0 {
					image.rotate270()
				} else if video_frame.rotation < -45.0 && video_frame.rotation > -135.0 {
					image.rotate90()
				} else {
					image
				};

				// Type WebPMemory is !Send, which makes the Future in this function !Send,
				// this make us `deref` to have a `&[u8]` and then `to_owned` to make a Vec<u8>
				// which implies on a unwanted clone...
				Ok(Encoder::from_image(&image)
					.expect("Should not fail as the underlining DynamicImage is an RgbImage")
					.encode(quality)
					.deref()
					.to_vec())
			}
		})
		.await?
	}
}

/// `ThumbnailerBuilder` struct holds data to build a `Thumbnailer` struct, exposing many methods
/// to configure how a thumbnail must be generated.
#[derive(Debug, Clone)]
#[must_use]
pub struct ThumbnailerBuilder {
	maintain_aspect_ratio: bool,
	size: ThumbnailSize,
	seek_percentage: f32,
	quality: f32,
	prefer_embedded_metadata: bool,
}

impl Default for ThumbnailerBuilder {
	fn default() -> Self {
		Self {
			maintain_aspect_ratio: true,
			size: ThumbnailSize::Scale(1024),
			seek_percentage: 0.1,
			quality: 80.0,
			prefer_embedded_metadata: true,
		}
	}
}

impl ThumbnailerBuilder {
	/// Creates a new `ThumbnailerBuilder` with default values:
	/// - `maintain_aspect_ratio`: true
	/// - `size`: 128 pixels
	/// - `seek_percentage`: 10%
	/// - `quality`: 80
	/// - `prefer_embedded_metadata`: true
	pub fn new() -> Self {
		Self::default()
	}

	/// To respect or not the aspect ratio from the video file in the generated thumbnail
	pub const fn maintain_aspect_ratio(mut self, maintain_aspect_ratio: bool) -> Self {
		self.maintain_aspect_ratio = maintain_aspect_ratio;
		self
	}

	/// To set a thumbnail size, respecting or not its aspect ratio, according to `maintain_aspect_ratio` value
	pub const fn size(mut self, size: ThumbnailSize) -> Self {
		self.size = size;
		self
	}

	/// Seek percentage must be a value between 0.0 and 1.0
	pub fn seek_percentage(mut self, seek_percentage: f32) -> Result<Self, Error> {
		if !(0.0..=1.0).contains(&seek_percentage) {
			return Err(Error::InvalidSeekPercentage(seek_percentage));
		}
		self.seek_percentage = seek_percentage;
		Ok(self)
	}

	/// Quality must be a value between 0.0 and 100.0
	pub fn quality(mut self, quality: f32) -> Result<Self, Error> {
		if !(0.0..=100.0).contains(&quality) {
			return Err(Error::InvalidQuality(quality));
		}
		self.quality = quality;
		Ok(self)
	}

	/// To use embedded metadata in the video file, if available, instead of getting a frame as a
	/// thumbnail
	pub const fn prefer_embedded_metadata(mut self, prefer_embedded_metadata: bool) -> Self {
		self.prefer_embedded_metadata = prefer_embedded_metadata;
		self
	}

	/// Builds a `Thumbnailer` struct
	#[must_use]
	pub const fn build(self) -> Thumbnailer {
		Thumbnailer { builder: self }
	}
}
