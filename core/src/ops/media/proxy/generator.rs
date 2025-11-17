//! Proxy generation engine using FFmpeg command wrapper

use super::{
	error::{ProxyError, ProxyResult},
	hardware::{detect_hardware_accel, HardwareAccel},
	ProxyVariantConfig,
};
use serde::{Deserialize, Serialize};
use std::{
	ffi::OsString,
	path::Path,
	process::Stdio,
	time::{Duration, Instant},
};
use tokio::{
	io::{AsyncBufReadExt, BufReader},
	process::Command,
};
use tracing::{debug, info, warn};

/// Information about a generated proxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInfo {
	pub size_bytes: u64,
	pub encoding_time_secs: u64,
	pub average_speed_multiplier: f32,
}

/// Progress information during proxy generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProgress {
	pub frame: u32,
	pub fps: f32,
	pub speed_multiplier: f32,
	pub time_secs: f32,
	pub percent: f32,
}

/// Proxy generator using FFmpeg
pub struct ProxyGenerator {
	config: ProxyVariantConfig,
	preset: String,
	use_hardware_accel: bool,
	hardware_accel: Option<HardwareAccel>,
}

impl ProxyGenerator {
	/// Create a new proxy generator
	pub fn new(config: ProxyVariantConfig, preset: String, use_hardware_accel: bool) -> Self {
		let hardware_accel = if use_hardware_accel {
			detect_hardware_accel()
		} else {
			None
		};

		if let Some(ref hw) = hardware_accel {
			info!("Using hardware acceleration: {:?}", hw);
		} else {
			info!("Using software encoding (libx264)");
		}

		Self {
			config,
			preset,
			use_hardware_accel,
			hardware_accel,
		}
	}

	/// Generate a proxy video
	pub async fn generate(
		&self,
		input: impl AsRef<Path>,
		output: impl AsRef<Path>,
	) -> ProxyResult<ProxyInfo> {
		let input = input.as_ref();
		let output = output.as_ref();

		debug!(
			"Generating {} proxy: {} -> {}",
			self.config.resolution.as_str(),
			input.display(),
			output.display()
		);

		// Ensure output directory exists
		if let Some(parent) = output.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Build FFmpeg command
		let args = self.build_ffmpeg_args(input, output)?;

		// Spawn FFmpeg process
		let mut cmd = Command::new("ffmpeg");
		cmd.args(args)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::piped());

		debug!("Executing FFmpeg command: {:?}", cmd);

		let mut child = cmd.spawn().map_err(|e| ProxyError::FFmpegNotFound)?;

		let start_time = Instant::now();
		let mut last_speed = 0.0f32;

		// Monitor progress from stderr
		if let Some(stderr) = child.stderr.take() {
			let reader = BufReader::new(stderr);
			let mut lines = reader.lines();

			while let Ok(Some(line)) = lines.next_line().await {
				if let Some(speed) = self.parse_speed_from_line(&line) {
					last_speed = speed;
					debug!("Encoding speed: {:.1}× realtime", speed);
				}
			}
		}

		// Wait for completion
		let status = child
			.wait()
			.await
			.map_err(|e| ProxyError::Other(format!("Failed to wait for ffmpeg: {}", e)))?;

		if !status.success() {
			let code = status.code().unwrap_or(-1);
			return Err(ProxyError::FFmpegProcessFailed(code));
		}

		let encoding_time = start_time.elapsed();

		// Get output file size
		let metadata = tokio::fs::metadata(output).await?;
		let size_bytes = metadata.len();

		info!(
			"✓ Generated proxy: {} MB in {:.1}s ({:.1}× realtime)",
			size_bytes / (1024 * 1024),
			encoding_time.as_secs_f32(),
			last_speed
		);

		Ok(ProxyInfo {
			size_bytes,
			encoding_time_secs: encoding_time.as_secs(),
			average_speed_multiplier: last_speed,
		})
	}

	/// Build FFmpeg command arguments safely
	fn build_ffmpeg_args(&self, input: &Path, output: &Path) -> ProxyResult<Vec<OsString>> {
		let mut args = Vec::new();

		// Overwrite output file without prompting
		args.push(OsString::from("-y"));

		// Input file
		args.push(OsString::from("-i"));
		args.push(input.as_os_str().to_owned());

		// Build video filter
		let mut vf_parts = Vec::new();

		// Scale to target resolution
		vf_parts.push(format!("scale=-2:{}", self.config.resolution.height()));

		// Add framerate reduction if needed
		if let Some(fps) = self.config.resolution.framerate() {
			vf_parts.push(format!("fps={}", fps));
		}

		let vf = vf_parts.join(",");
		args.push(OsString::from("-vf"));
		args.push(OsString::from(vf));

		// Video codec settings
		if let Some(hw) = &self.hardware_accel {
			// Hardware acceleration
			args.push(OsString::from("-c:v"));
			args.push(OsString::from(hw.encoder_name()));

			// Add hardware-specific preset if supported
			if let Some(preset) = hw.preset() {
				args.push(OsString::from("-preset"));
				args.push(OsString::from(preset));
			}

			// Add extra hardware-specific args
			for arg in hw.extra_args() {
				args.push(OsString::from(arg));
			}

			// Use bitrate mode for hardware encoders (not CRF)
			let target_bitrate = self.calculate_target_bitrate();
			args.push(OsString::from("-b:v"));
			args.push(OsString::from(format!("{}k", target_bitrate)));
		} else {
			// Software encoding (libx264)
			args.push(OsString::from("-c:v"));
			args.push(OsString::from("libx264"));

			args.push(OsString::from("-preset"));
			args.push(OsString::from(&self.preset));

			// CRF mode for software
			args.push(OsString::from("-crf"));
			args.push(OsString::from(self.config.resolution.crf().to_string()));
		}

		// Audio codec settings
		args.push(OsString::from("-c:a"));
		args.push(OsString::from("aac"));
		args.push(OsString::from("-b:a"));
		args.push(OsString::from(format!(
			"{}k",
			self.config.resolution.audio_bitrate()
		)));
		args.push(OsString::from("-ar"));
		args.push(OsString::from(
			self.config.resolution.audio_sample_rate().to_string(),
		));

		// Optimize for streaming (moov atom at start)
		args.push(OsString::from("-movflags"));
		args.push(OsString::from("+faststart"));

		// Output file
		args.push(output.as_os_str().to_owned());

		Ok(args)
	}

	/// Calculate target bitrate for hardware encoders
	fn calculate_target_bitrate(&self) -> u32 {
		// Approximate bitrate based on resolution
		match self.config.resolution.height() {
			180 => 300, // kbps
			240 => 500,
			480 => 1000,
			720 => 2000,
			1080 => 4000,
			_ => 2000,
		}
	}

	/// Parse encoding speed from FFmpeg output line
	fn parse_speed_from_line(&self, line: &str) -> Option<f32> {
		// FFmpeg progress format: "... speed=51.2x ..."
		if let Some(speed_idx) = line.find("speed=") {
			let speed_str = &line[speed_idx + 6..];
			if let Some(x_idx) = speed_str.find('x') {
				let speed_num = &speed_str[..x_idx];
				return speed_num.trim().parse::<f32>().ok();
			}
		}
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_speed_parsing() {
		let gen = ProxyGenerator::new(
			ProxyVariantConfig::new(super::super::ProxyResolution::Quick),
			"veryfast".to_string(),
			false,
		);

		let line = "frame= 1234 fps=42 q=28.0 size= 1024kB time=00:00:10.00 bitrate= 839.7kbits/s speed=51.2x";
		assert_eq!(gen.parse_speed_from_line(line), Some(51.2));

		let line2 = "frame=  100 fps=99 speed= 155x";
		assert_eq!(gen.parse_speed_from_line(line2), Some(155.0));
	}

	#[test]
	fn test_bitrate_calculation() {
		let gen = ProxyGenerator::new(
			ProxyVariantConfig::new(super::super::ProxyResolution::Quick),
			"veryfast".to_string(),
			false,
		);

		assert_eq!(gen.calculate_target_bitrate(), 1000); // 480p
	}
}
