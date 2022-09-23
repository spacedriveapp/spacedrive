use crate::{
	film_strip::film_strip_filter,
	movie_decoder::{MovieDecoder, ThumbnailSize},
	video_frame::VideoFrame,
};

use std::path::Path;

mod error;
mod film_strip;
mod movie_decoder;
mod thumbnailer;
mod utils;
mod video_frame;

pub use error::ThumbnailerError;
pub use thumbnailer::{Thumbnailer, ThumbnailerBuilder};

/// Helper function to generate a thumbnail file from a video file with reasonable defaults
pub async fn to_thumbnail(
	video_file_path: impl AsRef<Path>,
	output_thumbnail_path: impl AsRef<Path>,
	size: u32,
	quality: f32,
) -> Result<(), ThumbnailerError> {
	ThumbnailerBuilder::new()
		.size(size)
		.quality(quality)?
		.build()
		.process(video_file_path, output_thumbnail_path)
		.await
}

/// Helper function to generate a thumbnail bytes from a video file with reasonable defaults
pub async fn to_webp_bytes(
	video_file_path: impl AsRef<Path>,
	size: u32,
	quality: f32,
) -> Result<Vec<u8>, ThumbnailerError> {
	ThumbnailerBuilder::new()
		.size(size)
		.quality(quality)?
		.build()
		.process_to_webp_bytes(video_file_path)
		.await
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;
	use tokio::fs;

	#[tokio::test]
	#[ignore]
	async fn test_all_files() {
		let video_file_path = [
			Path::new("./samples/video_01.mp4"),
			Path::new("./samples/video_02.mov"),
			Path::new("./samples/video_03.mov"),
			Path::new("./samples/video_04.mov"),
			Path::new("./samples/video_05.mov"),
			Path::new("./samples/video_06.mov"),
			Path::new("./samples/video_07.mp4"),
			Path::new("./samples/video_08.mov"),
			Path::new("./samples/video_09.MP4"),
		];

		let expected_webp_files = [
			Path::new("./samples/video_01.webp"),
			Path::new("./samples/video_02.webp"),
			Path::new("./samples/video_03.webp"),
			Path::new("./samples/video_04.webp"),
			Path::new("./samples/video_05.webp"),
			Path::new("./samples/video_06.webp"),
			Path::new("./samples/video_07.webp"),
			Path::new("./samples/video_08.webp"),
			Path::new("./samples/video_09.webp"),
		];

		let root = tempdir().unwrap();
		let actual_webp_files = [
			root.path().join("video_01.webp"),
			root.path().join("video_02.webp"),
			root.path().join("video_03.webp"),
			root.path().join("video_04.webp"),
			root.path().join("video_05.webp"),
			root.path().join("video_06.webp"),
			root.path().join("video_07.webp"),
			root.path().join("video_08.webp"),
			root.path().join("video_09.webp"),
		];

		for (input, output) in video_file_path.iter().zip(actual_webp_files.iter()) {
			if let Err(e) = to_thumbnail(input, output, 128, 100.0).await {
				eprintln!("Error: {e}; Input: {}", input.display());
				panic!("{}", e);
			}
		}

		for (expected, actual) in expected_webp_files.iter().zip(actual_webp_files.iter()) {
			let expected_bytes = fs::read(expected).await.unwrap();
			let actual_bytes = fs::read(actual).await.unwrap();
			assert_eq!(expected_bytes, actual_bytes);
		}
	}
}
