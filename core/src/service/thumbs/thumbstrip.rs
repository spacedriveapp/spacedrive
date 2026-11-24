#[cfg(not(feature = "ffmpeg"))]
use std::ffi::OsString;
use std::{fs, path::Path, time::Duration};

use tempfile::Builder;

#[cfg(not(feature = "ffmpeg"))]
use super::ffmpeg::process_failure;
use crate::service::external_tools::ExternalTools;

pub const COLUMNS: u8 = 5;
pub const ROWS: u8 = 5;
const FRAME_WIDTH: u16 = 160;
const FRAME_HEIGHT: u16 = 90;
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(not(feature = "ffmpeg"))]
const PROCESS_OUTPUT_LIMIT: usize = 1024 * 1024;

/// Whether this build can make thumbstrips on this host: with the linked
/// libraries, or with the `ffmpeg` and `ffprobe` executables in builds that
/// ship no codecs.
pub(super) fn available(tools: &ExternalTools) -> bool {
	cfg!(feature = "ffmpeg") || (tools.ffmpeg_path().is_some() && tools.ffprobe_path().is_some())
}

/// Render a timeline-wide 5×5 PNG sprite and atomically publish it.
pub(super) fn generate(tools: &ExternalTools, input: &Path, output: &Path) -> Result<(), String> {
	if output.is_file() {
		return Ok(());
	}
	let duration = duration_seconds(tools, input)?;
	let fps = f64::from(COLUMNS) * f64::from(ROWS) / duration;
	let filter = format!(
		"fps={fps:.9},scale={FRAME_WIDTH}:{FRAME_HEIGHT}:force_original_aspect_ratio=increase,crop={FRAME_WIDTH}:{FRAME_HEIGHT},tile={COLUMNS}x{ROWS}:padding=0:margin=0"
	);

	let parent = output
		.parent()
		.ok_or_else(|| format!("thumbstrip path has no parent: {}", output.display()))?;
	fs::create_dir_all(parent)
		.map_err(|error| format!("create thumbstrip directory {}: {error}", parent.display()))?;
	let temporary = Builder::new()
		.prefix(".thumbstrip-")
		.suffix(".png")
		.tempfile_in(parent)
		.map_err(|error| format!("create thumbstrip temporary file: {error}"))?
		.into_temp_path();

	render(tools, input, &filter, &temporary)?;
	let metadata =
		fs::metadata(&temporary).map_err(|error| format!("read generated thumbstrip: {error}"))?;
	if metadata.len() == 0 {
		return Err("FFmpeg generated an empty thumbstrip".to_string());
	}

	if output.exists() {
		return Ok(());
	}
	fs::rename(&temporary, output)
		.map_err(|error| format!("publish thumbstrip {}: {error}", output.display()))?;
	Ok(())
}

/// The tiled frame as an RGB PNG, as the executable writes it.
#[cfg(feature = "ffmpeg")]
fn render(_tools: &ExternalTools, input: &Path, filter: &str, output: &Path) -> Result<(), String> {
	let picture = sd_ffmpeg::footage::read_rgba(input, filter, PROCESS_TIMEOUT)
		.map_err(|error| format!("FFmpeg could not generate the thumbstrip: {error}"))?
		.ok_or("FFmpeg generated no thumbstrip frame")?;
	let image = image::RgbaImage::from_raw(picture.width, picture.height, picture.rgba)
		.ok_or("the thumbstrip frame has the wrong size")?;
	image::DynamicImage::ImageRgba8(image)
		.to_rgb8()
		.save_with_format(output, image::ImageFormat::Png)
		.map_err(|error| format!("write thumbstrip: {error}"))
}

#[cfg(not(feature = "ffmpeg"))]
fn render(tools: &ExternalTools, input: &Path, filter: &str, output: &Path) -> Result<(), String> {
	let args = vec![
		"-hide_banner".into(),
		"-loglevel".into(),
		"error".into(),
		"-nostdin".into(),
		"-y".into(),
		"-i".into(),
		input.as_os_str().to_owned(),
		"-map".into(),
		"0:v:0".into(),
		"-an".into(),
		"-sn".into(),
		"-dn".into(),
		"-vf".into(),
		OsString::from(filter),
		"-frames:v".into(),
		"1".into(),
		"-c:v".into(),
		"png".into(),
		"-f".into(),
		"image2".into(),
		"-update".into(),
		"1".into(),
		output.as_os_str().to_owned(),
	];
	let result = tools
		.run_ffmpeg(&args, PROCESS_TIMEOUT, PROCESS_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !result.status.success() {
		return Err(process_failure(
			"FFmpeg could not generate the thumbstrip",
			&result.stderr,
		));
	}
	Ok(())
}

#[cfg(feature = "ffmpeg")]
fn duration_seconds(_tools: &ExternalTools, input: &Path) -> Result<f64, String> {
	let report = sd_ffmpeg::footage::probe(input, PROBE_TIMEOUT)
		.map_err(|error| format!("could not read video duration: {error}"))?;
	parse_duration(report["format"]["duration"].as_str().unwrap_or_default())
}

#[cfg(not(feature = "ffmpeg"))]
fn duration_seconds(tools: &ExternalTools, input: &Path) -> Result<f64, String> {
	let args = vec![
		"-v".into(),
		"error".into(),
		"-show_entries".into(),
		"format=duration".into(),
		"-of".into(),
		"default=noprint_wrappers=1:nokey=1".into(),
		input.as_os_str().to_owned(),
	];
	let result = tools
		.run_ffprobe(&args, PROBE_TIMEOUT, PROCESS_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !result.status.success() {
		return Err(process_failure(
			"FFprobe could not read video duration",
			&result.stderr,
		));
	}
	parse_duration(&String::from_utf8_lossy(&result.stdout))
}

fn parse_duration(value: &str) -> Result<f64, String> {
	let duration = value
		.trim()
		.parse::<f64>()
		.map_err(|error| format!("parse video duration {value:?}: {error}"))?;
	if !duration.is_finite() || duration <= 0.0 {
		return Err(format!("invalid video duration {duration}"));
	}
	Ok(duration)
}

#[cfg(all(test, feature = "ffmpeg"))]
mod tests {
	use super::*;

	/// A thumbstrip from the linked libraries, with no `ffmpeg` executable
	/// on the machine.
	#[test]
	fn thumbstrip_tiles_the_whole_video() {
		let dir = tempfile::tempdir().unwrap();
		let video = dir.path().join("clip.mov");
		sd_ffmpeg::footage::render(
			"testsrc2=s=320x180:r=25:d=5",
			"libx264",
			&[("preset".to_string(), "ultrafast".to_string())],
			"mov",
			&[],
			&video,
		)
		.unwrap();
		let output = dir.path().join("strip.png");
		generate(&ExternalTools::new(), &video, &output).unwrap();
		let strip = image::open(&output).unwrap();
		assert_eq!(
			(strip.width(), strip.height()),
			(
				u32::from(FRAME_WIDTH) * u32::from(COLUMNS),
				u32::from(FRAME_HEIGHT) * u32::from(ROWS)
			)
		);
		assert!(available(&ExternalTools::new()));
	}
}
