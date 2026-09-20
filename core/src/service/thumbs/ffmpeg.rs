use std::{ffi::OsString, sync::Arc, time::Duration};

use image::DynamicImage;
use sd_bake::{Decline, Producer, Tile, WorkItem};

use crate::service::external_tools::ExternalTools;

const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(30);
const THUMBNAIL_OUTPUT_LIMIT: usize = 32 * 1024 * 1024;

/// Video-frame producer backed by the FFmpeg executable installed on the host.
pub(super) struct HostFfmpegProducer {
	tools: Arc<ExternalTools>,
}

impl HostFfmpegProducer {
	pub fn new(tools: Arc<ExternalTools>) -> Self {
		Self { tools }
	}
}

impl Producer for HostFfmpegProducer {
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
		if item.is_dir || !is_video(&item.path) {
			return Err(Decline::Unsupported);
		}
		if self.tools.ffmpeg_path().is_none() {
			return Err(Decline::Unavailable);
		}

		// Scale before selecting. `thumbnail` holds a window of frames to pick a
		// representative one from, and at source resolution that window is the
		// whole cost of the run: 4264x2408 ProRes peaks at 1,290 MB, against
		// 233 MB for the same frame chosen from scaled input.
		let filter = format!(
			"scale={tile_size}:{tile_size}:force_original_aspect_ratio=decrease,thumbnail=30"
		);
		let args = vec![
			"-hide_banner".into(),
			"-loglevel".into(),
			"error".into(),
			"-nostdin".into(),
			// One frame is wanted, and the caller already runs a batch of these
			// at once, so parallelism belongs across files rather than inside a
			// single decode.
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
			OsString::from(filter),
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

fn dynamic_image_tile(image: DynamicImage) -> Tile {
	let rgba = image.to_rgba8();
	let (width, height) = rgba.dimensions();
	let mut bgra = rgba.into_raw();
	for pixel in bgra.chunks_exact_mut(4) {
		pixel.swap(0, 2);
	}
	Tile::new(width, height, bgra).with_source(width, height)
}

pub(super) fn is_video(path: &std::path::Path) -> bool {
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
