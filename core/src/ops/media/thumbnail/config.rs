//! Thumbnail variant configuration
//!
//! Defines the standard thumbnail sizes and their variant names for the sidecar system.

use crate::ops::sidecar::types::{SidecarFormat, SidecarVariant};

/// Standard thumbnail configuration
#[derive(Debug, Clone)]
pub struct ThumbnailVariantConfig {
	pub size: u32,
	pub variant: SidecarVariant,
	pub quality: u8,
}

impl ThumbnailVariantConfig {
	pub fn new(size: u32, variant: impl Into<String>, quality: u8) -> Self {
		Self {
			size,
			variant: SidecarVariant::new(variant),
			quality,
		}
	}

	/// Get the format for thumbnails (always WebP)
	pub fn format(&self) -> SidecarFormat {
		SidecarFormat::Webp
	}
}

/// Standard thumbnail variants used across Spacedrive
pub struct ThumbnailVariants;

impl ThumbnailVariants {
	/// Grid view thumbnail - 1x resolution (256px)
	pub fn grid_1x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(256, "grid@1x", 85)
	}

	/// Grid view thumbnail - 2x resolution (512px) for retina displays
	pub fn grid_2x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(512, "grid@2x", 85)
	}

	/// Detail view thumbnail - 1x resolution (1024px)
	pub fn detail_1x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(1024, "detail@1x", 90)
	}

	/// Detail view thumbnail - 2x resolution (2048px) for retina displays
	pub fn detail_2x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(2048, "detail@2x", 90)
	}

	/// Small icon thumbnail - 1x resolution (128px)
	pub fn icon_1x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(128, "icon@1x", 80)
	}

	/// Small icon thumbnail - 2x resolution (256px) for retina displays
	pub fn icon_2x() -> ThumbnailVariantConfig {
		ThumbnailVariantConfig::new(256, "icon@2x", 80)
	}

	/// Get all standard variants
	pub fn all() -> Vec<ThumbnailVariantConfig> {
		vec![
			Self::icon_1x(),
			Self::icon_2x(),
			Self::grid_1x(),
			Self::grid_2x(),
			Self::detail_1x(),
			Self::detail_2x(),
		]
	}

	/// Get default variants (commonly used subset)
	pub fn defaults() -> Vec<ThumbnailVariantConfig> {
		vec![Self::grid_1x(), Self::grid_2x(), Self::detail_1x()]
	}

	/// Map a size to the closest standard variant
	pub fn from_size(size: u32) -> Option<ThumbnailVariantConfig> {
		match size {
			128 => Some(Self::icon_1x()),
			256 => Some(Self::grid_1x()),
			512 => Some(Self::grid_2x()),
			1024 => Some(Self::detail_1x()),
			2048 => Some(Self::detail_2x()),
			_ => None,
		}
	}

	/// Get variant name for a given size (fallback for custom sizes)
	pub fn variant_name_for_size(size: u32) -> String {
		if let Some(config) = Self::from_size(size) {
			config.variant.as_str().to_string()
		} else {
			format!("custom@{}px", size)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_standard_variants() {
		let grid_1x = ThumbnailVariants::grid_1x();
		assert_eq!(grid_1x.size, 256);
		assert_eq!(grid_1x.variant.as_str(), "grid@1x");
		assert_eq!(grid_1x.quality, 85);

		let detail_2x = ThumbnailVariants::detail_2x();
		assert_eq!(detail_2x.size, 2048);
		assert_eq!(detail_2x.variant.as_str(), "detail@2x");
	}

	#[test]
	fn test_from_size() {
		assert!(ThumbnailVariants::from_size(256).is_some());
		assert!(ThumbnailVariants::from_size(512).is_some());
		assert!(ThumbnailVariants::from_size(999).is_none());
	}

	#[test]
	fn test_defaults() {
		let defaults = ThumbnailVariants::defaults();
		assert_eq!(defaults.len(), 3);
		assert_eq!(defaults[0].size, 256);
		assert_eq!(defaults[1].size, 512);
		assert_eq!(defaults[2].size, 1024);
	}

	#[test]
	fn test_all_variants() {
		let all = ThumbnailVariants::all();
		assert_eq!(all.len(), 6);
	}

	#[test]
	fn test_custom_variant_name() {
		assert_eq!(ThumbnailVariants::variant_name_for_size(256), "grid@1x");
		assert_eq!(
			ThumbnailVariants::variant_name_for_size(999),
			"custom@999px"
		);
	}
}
