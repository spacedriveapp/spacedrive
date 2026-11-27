//! The container header reader against real clips: an MP4 whose `moov`
//! box follows its media data, a fast-start MOV, WebM and MKV. Each was cut
//! by FFmpeg from its test pattern, and the expected values are what
//! `ffprobe` reports for it.

use std::path::{Path, PathBuf};

use sd_media_metadata::video::{read_header, VideoHeader};

fn fixture(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures")
		.join(name)
}

fn header(name: &str) -> VideoHeader {
	read_header(&fixture(name))
		.expect("readable")
		.expect("a video header")
}

fn assert_header(name: &str, duration: f64, width: u32, height: u32, codec: &str) {
	let header = header(name);
	let seconds = header.duration_seconds.expect("a duration");
	assert!(
		(seconds - duration).abs() < 0.05,
		"{name}: {seconds} s, expected {duration} s"
	);
	assert_eq!(header.width, Some(width), "{name} width");
	assert_eq!(header.height, Some(height), "{name} height");
	assert_eq!(header.codec.as_deref(), Some(codec), "{name} codec");
}

#[test]
fn an_mp4_with_its_movie_box_after_the_media_is_read() {
	assert_header("moov-at-end.mp4", 2.0, 320, 240, "h264");
}

#[test]
fn a_fast_start_mov_is_read() {
	assert_header("faststart.mov", 1.5, 640, 360, "mpeg4");
}

#[test]
fn webm_is_read() {
	assert_header("clip.webm", 1.0, 256, 144, "vp9");
}

#[test]
fn mkv_is_read() {
	assert_header("clip.mkv", 3.0, 192, 108, "hevc");
}

#[test]
fn a_file_that_is_not_a_video_has_no_header() {
	let path = std::env::temp_dir().join(format!("sd-video-header-{}.txt", std::process::id()));
	std::fs::write(&path, b"plain text, not a container").unwrap();
	assert_eq!(read_header(&path).unwrap(), None);
	std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_box_larger_than_the_file_ends_the_read() {
	let path = std::env::temp_dir().join(format!("sd-video-overflow-{}.mp4", std::process::id()));
	let mut bytes = Vec::new();
	bytes.extend_from_slice(&24u32.to_be_bytes());
	bytes.extend_from_slice(b"ftypisom");
	bytes.extend_from_slice(&[0; 12]);
	bytes.extend_from_slice(&1u32.to_be_bytes());
	bytes.extend_from_slice(b"free");
	bytes.extend_from_slice(&(u64::MAX - 23).to_be_bytes());
	bytes.extend_from_slice(&[0; 32]);
	std::fs::write(&path, &bytes).unwrap();
	assert_eq!(read_header(&path).unwrap(), None);
	std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_truncated_video_answers_without_panicking() {
	let bytes = std::fs::read(fixture("faststart.mov")).unwrap();
	let path = std::env::temp_dir().join(format!("sd-video-truncated-{}.mov", std::process::id()));
	for cut in [8, 64, 200, bytes.len() / 2] {
		std::fs::write(&path, &bytes[..cut]).unwrap();
		let _ = read_header(&path);
	}
	std::fs::remove_file(&path).unwrap();
}
