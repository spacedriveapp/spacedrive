use std::sync::Arc;
#[cfg(not(feature = "ffmpeg"))]
use std::{ffi::OsString, time::Duration};

use image::DynamicImage;
use sd_bake::{Decline, Producer, Tile, WorkItem};

use crate::service::external_tools::ExternalTools;

#[cfg(not(feature = "ffmpeg"))]
const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(not(feature = "ffmpeg"))]
const THUMBNAIL_OUTPUT_LIMIT: usize = 32 * 1024 * 1024;

/// Video-frame producer. Builds with the `ffmpeg` feature decode with the
/// linked libraries, so a machine without an `ffmpeg` executable still gets
/// video thumbnails. Builds without it, which ship no codecs, use the
/// executable installed on the host.
pub(super) struct VideoProducer {
	#[cfg_attr(feature = "ffmpeg", allow(dead_code))]
	tools: Arc<ExternalTools>,
}

impl VideoProducer {
	pub fn new(tools: Arc<ExternalTools>) -> Self {
		Self { tools }
	}
}

/// Scale before selecting. `thumbnail` holds a window of frames to pick a
/// representative one from, and at source resolution that window is the
/// whole cost of the run: 4264x2408 ProRes peaks at 1,290 MB, against 233 MB
/// for the same frame chosen from scaled input.
fn thumbnail_filter(tile_size: u32) -> String {
	format!("scale={tile_size}:{tile_size}:force_original_aspect_ratio=decrease,thumbnail=30")
}

/// Whether video frames can be decoded at all in this build on this host.
pub(super) fn video_decoding_available(tools: &ExternalTools) -> bool {
	cfg!(feature = "ffmpeg") || tools.ffmpeg_path().is_some()
}

impl Producer for VideoProducer {
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
		if item.is_dir || !is_video(&item.path) {
			return Err(Decline::Unsupported);
		}
		#[cfg(feature = "ffmpeg")]
		{
			let picture = sd_ffmpeg::footage::read_rgba(&item.path, &thumbnail_filter(tile_size))
				.map_err(|error| {
					Decline::Failed(format!("could not decode a video frame: {error}"))
				})?
				.ok_or_else(|| Decline::Failed("the video has no frames".to_string()))?;
			let image = image::RgbaImage::from_raw(picture.width, picture.height, picture.rgba)
				.ok_or_else(|| {
					Decline::Failed("the decoded frame has the wrong size".to_string())
				})?;
			Ok(dynamic_image_tile(DynamicImage::ImageRgba8(image)))
		}
		#[cfg(not(feature = "ffmpeg"))]
		{
			if self.tools.ffmpeg_path().is_none() {
				return Err(Decline::Unavailable);
			}
			let args = vec![
				"-hide_banner".into(),
				"-loglevel".into(),
				"error".into(),
				"-nostdin".into(),
				// One frame is wanted, and the caller already runs a batch of
				// these at once, so parallelism belongs across files rather
				// than inside a single decode.
				"-threads".into(),
				"1".into(),
				"-i".into(),
				item.path.as_os_str().to_owned(),
				"-map".into(),
				"0:v:0".into(),
				"-an".into(),
				"-sn".into(),
				"-dn".into(),
				"-frames:v".into(),
				"1".into(),
				"-vf".into(),
				OsString::from(thumbnail_filter(tile_size)),
				"-f".into(),
				"image2pipe".into(),
				"-c:v".into(),
				"png".into(),
				"pipe:1".into(),
			];
			let output = self
				.tools
				.run_ffmpeg(&args, THUMBNAIL_TIMEOUT, THUMBNAIL_OUTPUT_LIMIT)
				.map_err(|error| Decline::Failed(error.to_string()))?;
			if !output.status.success() {
				return Err(Decline::Failed(process_failure(
					"FFmpeg could not decode a video frame",
					&output.stderr,
				)));
			}
			let decoded = image::load_from_memory(&output.stdout)
				.map_err(|error| Decline::Failed(format!("decode FFmpeg frame: {error}")))?;
			Ok(dynamic_image_tile(decoded))
		}
	}
}

fn dynamic_image_tile(image: DynamicImage) -> Tile {
	let rgba = image.to_rgba8();
	let (width, height) = rgba.dimensions();
	let mut bgra = rgba.into_raw();
	for pixel in bgra.chunks_exact_mut(4) {
		pixel.swap(0, 2);
	}
	Tile::new(width, height, bgra).with_source(width, height)
}

pub(crate) fn is_video(path: &std::path::Path) -> bool {
	let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
		return false;
	};
	matches!(
		extension.to_ascii_lowercase().as_str(),
		"3gp"
			| "avi" | "flv"
			| "m2ts" | "m4v"
			| "mkv" | "mov"
			| "mp4" | "mpeg"
			| "mpg" | "mts"
			| "mxf" | "ogv"
			| "ts" | "vob"
			| "webm" | "wmv"
	)
}

#[cfg(not(feature = "ffmpeg"))]
pub(super) fn process_failure(context: &str, stderr: &str) -> String {
	let reason = stderr.lines().map(str::trim).find(|line| !line.is_empty());
	match reason {
		Some(reason) => format!("{context}: {reason}"),
		None => context.to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ffmpeg_stage_declines_non_video_extensions() {
		assert!(is_video(std::path::Path::new("clip.MOV")));
		assert!(is_video(std::path::Path::new("clip.mkv")));
		assert!(!is_video(std::path::Path::new("report.pdf")));
		assert!(!is_video(std::path::Path::new("README")));
	}
}

#[cfg(all(test, feature = "ffmpeg"))]
mod linked_tests {
	use super::*;

	/// A video tile from the linked libraries, fit inside the tile size at
	/// the video's own proportions, with no `ffmpeg` executable needed.
	#[test]
	fn video_thumbnail_decodes_with_the_linked_libraries() {
		let dir = tempfile::tempdir().unwrap();
		let video = dir.path().join("clip.mp4");
		sd_ffmpeg::footage::render(
			"testsrc2=s=640x360:r=25:d=2",
			"libx264",
			&[("preset".to_string(), "ultrafast".to_string())],
			"mp4",
			&[],
			&video,
		)
		.unwrap();
		let producer = VideoProducer::new(Arc::new(ExternalTools::new()));
		let tile = producer.produce(&WorkItem::file(video), 256).unwrap();
		assert_eq!((tile.width(), tile.height()), (256, 144));
		assert!(video_decoding_available(&ExternalTools::new()));
	}
}
