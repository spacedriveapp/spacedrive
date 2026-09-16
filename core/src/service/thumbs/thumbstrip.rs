use std::{ffi::OsString, fs, path::Path, time::Duration};

use tempfile::Builder;

use crate::service::external_tools::ExternalTools;

use super::ffmpeg::process_failure;

pub const COLUMNS: u8 = 5;
pub const ROWS: u8 = 5;
const FRAME_WIDTH: u16 = 160;
const FRAME_HEIGHT: u16 = 90;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(120);
const PROCESS_OUTPUT_LIMIT: usize = 1024 * 1024;

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
		temporary.as_os_str().to_owned(),
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
		.run_ffprobe(&args, Duration::from_secs(15), PROCESS_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !result.status.success() {
		return Err(process_failure(
			"FFprobe could not read video duration",
			&result.stderr,
		));
	}
	let value = String::from_utf8_lossy(&result.stdout);
	let duration = value
		.trim()
		.parse::<f64>()
		.map_err(|error| format!("parse FFprobe duration {value:?}: {error}"))?;
	if !duration.is_finite() || duration <= 0.0 {
		return Err(format!("FFprobe returned invalid duration {duration}"));
	}
	Ok(duration)
}
