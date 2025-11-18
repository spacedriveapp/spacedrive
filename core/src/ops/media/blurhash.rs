//! Blurhash generation utilities for images and videos
//!
//! Blurhash is a compact representation of an image that can be decoded into a
//! low-resolution placeholder. Perfect for showing while full images/thumbnails load.

use image::{DynamicImage, GenericImageView};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlurhashError {
	#[error("Image is too small for blurhash generation")]
	ImageTooSmall,

	#[error("Blurhash encoding failed: {0}")]
	EncodingFailed(String),

	#[error("Invalid image dimensions")]
	InvalidDimensions,
}

/// Generate a blurhash from a DynamicImage
///
/// The blurhash will be generated using 4x3 components for good quality
/// while keeping the hash compact (~20-30 chars).
///
/// # Arguments
///
/// * `image` - The image to generate a blurhash from
///
/// # Returns
///
/// A blurhash string that can be decoded into a placeholder image
pub fn generate_blurhash(image: &DynamicImage) -> Result<String, BlurhashError> {
	let (width, height) = image.dimensions();

	if width == 0 || height == 0 {
		return Err(BlurhashError::InvalidDimensions);
	}

	// Blurhash works best with reasonable dimensions
	// If image is too large, resize to max 256px for blurhash calculation
	let working_image = if width > 256 || height > 256 {
		let scale = 256.0 / width.max(height) as f64;
		let new_width = (width as f64 * scale) as u32;
		let new_height = (height as f64 * scale) as u32;

		image.resize_exact(
			new_width.max(1),
			new_height.max(1),
			image::imageops::FilterType::Lanczos3,
		)
	} else {
		image.clone()
	};

	let (w, h) = working_image.dimensions();

	// Convert to RGB8 for blurhash encoding
	let rgb_image = working_image.to_rgb8();
	let pixels = rgb_image.as_raw();

	// Generate blurhash with 4x3 components (good balance of quality and size)
	// Results in ~20-30 character hash
	let hash = blurhash::encode(4, 3, w, h, pixels)
		.map_err(|e| BlurhashError::EncodingFailed(e.to_string()))?;

	Ok(hash)
}

/// Generate a blurhash from a video frame
///
/// This is a convenience wrapper around `generate_blurhash` for video frames.
pub fn generate_blurhash_from_video_frame(frame: &DynamicImage) -> Result<String, BlurhashError> {
	generate_blurhash(frame)
}

#[cfg(test)]
mod tests {
	use super::*;
	use image::RgbImage;

	#[test]
	fn test_generate_blurhash() {
		// Create a simple gradient image for testing
		let width = 100;
		let height = 100;
		let mut img = RgbImage::new(width, height);

		for y in 0..height {
			for x in 0..width {
				let r = (x as f32 / width as f32 * 255.0) as u8;
				let g = (y as f32 / height as f32 * 255.0) as u8;
				let b = 128;
				img.put_pixel(x, y, image::Rgb([r, g, b]));
			}
		}

		let dynamic_img = DynamicImage::ImageRgb8(img);
		let hash = generate_blurhash(&dynamic_img).unwrap();

		// Blurhash should be a non-empty string
		assert!(!hash.is_empty());
		// Should be around 20-30 characters for 4x3 components
		assert!(hash.len() > 10 && hash.len() < 50);
	}

	#[test]
	fn test_zero_dimensions() {
		let img = DynamicImage::new_rgb8(0, 0);
		let result = generate_blurhash(&img);
		assert!(result.is_err());
	}

	#[test]
	fn test_large_image_resize() {
		// Create a large image to test automatic resizing
		let img = DynamicImage::new_rgb8(2000, 2000);
		let hash = generate_blurhash(&img).unwrap();

		// Should still generate a hash even with large dimensions
		assert!(!hash.is_empty());
	}
}

