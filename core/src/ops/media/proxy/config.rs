//! Proxy variant configuration and presets

use crate::ops::sidecar::types::{SidecarFormat, SidecarVariant};
use serde::{Deserialize, Serialize};

/// Proxy resolution preset
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyResolution {
	/// 180p @ 15fps - Ultra-minimal for scrubbing
	Scrubbing,
	/// 240p - Ultra-low for instant preview
	UltraLow,
	/// 480p - Quick preview
	Quick,
	/// 720p - Editing workflow
	Editing,
}

impl ProxyResolution {
	pub fn height(&self) -> u32 {
		match self {
			Self::Scrubbing => 180,
			Self::UltraLow => 240,
			Self::Quick => 480,
			Self::Editing => 720,
		}
	}

	pub fn framerate(&self) -> Option<u32> {
		match self {
			Self::Scrubbing => Some(15), // Reduced framerate for scrubbing
			_ => None,                   // Use original framerate
		}
	}

	pub fn crf(&self) -> u32 {
		match self {
			Self::Scrubbing => 33,
			Self::UltraLow => 30,
			Self::Quick => 26,
			Self::Editing => 23,
		}
	}

	pub fn audio_bitrate(&self) -> u32 {
		match self {
			Self::Scrubbing => 32, // kbps
			Self::UltraLow => 48,
			Self::Quick => 96,
			Self::Editing => 128,
		}
	}

	pub fn audio_sample_rate(&self) -> u32 {
		match self {
			Self::Scrubbing | Self::UltraLow => 22050, // Reduced for smaller size
			Self::Quick | Self::Editing => 44100,      // Standard
		}
	}

	pub fn variant_name(&self) -> &'static str {
		match self {
			Self::Scrubbing => "proxy_scrub",
			Self::UltraLow => "proxy_ultra",
			Self::Quick => "proxy_quick",
			Self::Editing => "proxy_edit",
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Scrubbing => "scrubbing",
			Self::UltraLow => "ultra_low",
			Self::Quick => "quick",
			Self::Editing => "editing",
		}
	}
}

/// Configuration for a single proxy variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyVariantConfig {
	pub resolution: ProxyResolution,
	pub variant: SidecarVariant,
}

impl ProxyVariantConfig {
	pub fn new(resolution: ProxyResolution) -> Self {
		Self {
			resolution,
			variant: SidecarVariant::new(resolution.variant_name()),
		}
	}

	pub fn format(&self) -> SidecarFormat {
		SidecarFormat::Mp4
	}
}

/// Standard proxy variants
pub struct ProxyVariants;

impl ProxyVariants {
	/// Scrubbing proxy - 180p @ 15fps (fast generation, tiny size)
	pub fn scrubbing() -> ProxyVariantConfig {
		ProxyVariantConfig::new(ProxyResolution::Scrubbing)
	}

	/// Ultra-low proxy - 240p (instant preview)
	pub fn ultra_low() -> ProxyVariantConfig {
		ProxyVariantConfig::new(ProxyResolution::UltraLow)
	}

	/// Quick proxy - 480p (desktop preview)
	pub fn quick() -> ProxyVariantConfig {
		ProxyVariantConfig::new(ProxyResolution::Quick)
	}

	/// Editing proxy - 720p (professional workflow)
	pub fn editing() -> ProxyVariantConfig {
		ProxyVariantConfig::new(ProxyResolution::Editing)
	}

	/// Default variants for auto-generation
	pub fn defaults() -> Vec<ProxyVariantConfig> {
		vec![Self::scrubbing()] // Only scrubbing proxy by default
	}

	/// All standard variants
	pub fn all() -> Vec<ProxyVariantConfig> {
		vec![
			Self::scrubbing(),
			Self::ultra_low(),
			Self::quick(),
			Self::editing(),
		]
	}
}

/// Configuration for proxy generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyJobConfig {
	/// Proxy variants to generate
	pub variants: Vec<ProxyVariantConfig>,

	/// Whether to regenerate existing proxies
	pub regenerate: bool,

	/// Use hardware acceleration if available
	pub use_hardware_accel: bool,

	/// FFmpeg preset (ultrafast, veryfast, fast, medium, slow)
	pub preset: String,
}

impl Default for ProxyJobConfig {
	fn default() -> Self {
		Self {
			variants: ProxyVariants::defaults(),
			regenerate: false,
			use_hardware_accel: true,
			preset: "veryfast".to_string(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_proxy_resolutions() {
		let scrub = ProxyResolution::Scrubbing;
		assert_eq!(scrub.height(), 180);
		assert_eq!(scrub.framerate(), Some(15));
		assert_eq!(scrub.crf(), 33);
		assert_eq!(scrub.audio_bitrate(), 32);

		let quick = ProxyResolution::Quick;
		assert_eq!(quick.height(), 480);
		assert_eq!(quick.framerate(), None); // Use original
		assert_eq!(quick.crf(), 26);
	}

	#[test]
	fn test_defaults() {
		let defaults = ProxyVariants::defaults();
		assert_eq!(defaults.len(), 1);
		assert_eq!(defaults[0].resolution, ProxyResolution::Scrubbing);
	}
}
