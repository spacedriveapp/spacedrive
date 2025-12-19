//! Gaussian Splat generation system
//!
//! Generates 3D Gaussian splats from images using Apple's SHARP model.
//! Generates .ply sidecar files for photorealistic view synthesis.

pub mod action;
pub mod job;
pub mod processor;

pub use action::{GenerateSplatAction, GenerateSplatInput, GenerateSplatOutput};
pub use job::{GaussianSplatJob, GaussianSplatJobConfig};
pub use processor::GaussianSplatProcessor;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Generate a 3D Gaussian splat from an image using SHARP
///
/// This calls the `sharp` CLI tool as a subprocess.
/// The sharp tool must be installed (e.g., via `pip install -r requirements.txt` in ml-sharp repo)
///
/// # Arguments
/// * `source_path` - Path to the input image
/// * `output_dir` - Directory where the .ply file will be generated
/// * `model_path` - Optional path to the SHARP model checkpoint
///
/// # Returns
/// Path to the generated .ply file
pub async fn generate_splat_from_image(
	source_path: &Path,
	output_dir: &Path,
	model_path: Option<&Path>,
) -> Result<PathBuf> {
	use tokio::process::Command;

	// Ensure output directory exists
	tokio::fs::create_dir_all(output_dir).await?;

	// Build command
	let mut cmd = Command::new("sharp");
	cmd.arg("predict")
		.arg("-i")
		.arg(source_path)
		.arg("-o")
		.arg(output_dir);

	// Add model path if provided
	if let Some(model) = model_path {
		cmd.arg("-c").arg(model);
	}

	// Execute
	let output = cmd
		.output()
		.await
		.context("Failed to execute 'sharp' command. Is it installed?")?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		anyhow::bail!("SHARP failed: {}", stderr);
	}

	// The output file will be named based on input filename with .ply extension
	let ply_filename = source_path
		.file_stem()
		.context("Invalid source filename")?
		.to_str()
		.context("Non-UTF8 filename")?;

	let ply_path = output_dir.join(format!("{}.ply", ply_filename));

	if !ply_path.exists() {
		anyhow::bail!(
			"SHARP did not generate expected output file: {:?}",
			ply_path
		);
	}

	Ok(ply_path)
}

/// Check if SHARP CLI is available in PATH
pub async fn check_sharp_available() -> Result<bool> {
	let output = tokio::process::Command::new("sharp")
		.arg("--help")
		.output()
		.await;

	Ok(output.is_ok())
}

/// Check if an image type is supported for splat generation
pub fn is_splat_supported(mime_type: &str) -> bool {
	// SHARP supports common image formats
	matches!(
		mime_type,
		"image/jpeg" | "image/png" | "image/webp" | "image/bmp" | "image/tiff"
	)
}
