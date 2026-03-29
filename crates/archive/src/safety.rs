//! Safety screening model (stub implementation).
//!
//! This is a temporary stub until the ONNX runtime (ort) is properly integrated.
//! For now, all content is marked as safe.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

/// Model version string stored alongside screening verdicts.
pub const SAFETY_MODEL_VERSION: &str = "stub-v1";

/// Result of screening a single piece of text.
#[derive(Debug, Clone)]
pub struct SafetyVerdict {
	/// Confidence score (0–100) that the text is a prompt injection.
	pub score: u8,
	/// Binary classification result.
	pub is_malicious: bool,
}

impl SafetyVerdict {
	/// Map to a verdict string based on configurable thresholds.
	pub fn verdict_string(&self, quarantine_threshold: u8, flag_threshold: u8) -> &'static str {
		if self.score >= quarantine_threshold {
			"quarantined"
		} else if self.score >= flag_threshold {
			"flagged"
		} else {
			"safe"
		}
	}
}

/// Default quarantine threshold (score 0–100).
pub const DEFAULT_QUARANTINE_THRESHOLD: u8 = 70;

/// Default flag threshold (score 0–100).
pub const DEFAULT_FLAG_THRESHOLD: u8 = 40;

/// Trust tier for a data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustTier {
	/// User-created content (Obsidian notes, local files, personal calendar).
	Authored,
	/// Shared / multi-author spaces (Slack, Discord, GitHub).
	Collaborative,
	/// Third-party content (email inbox, RSS, web bookmarks, browser history).
	External,
}

impl TrustTier {
	/// Parse from a string, defaulting to `External` for unknown values.
	pub fn from_str_or_default(s: &str) -> Self {
		match s {
			"authored" => Self::Authored,
			"collaborative" => Self::Collaborative,
			"external" => Self::External,
			_ => {
				tracing::warn!(value = s, "unknown trust_tier, defaulting to 'external'");
				Self::External
			}
		}
	}

	/// Canonical string representation.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Authored => "authored",
			Self::Collaborative => "collaborative",
			Self::External => "external",
		}
	}
}

impl std::fmt::Display for TrustTier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

impl Default for TrustTier {
	fn default() -> Self {
		Self::External
	}
}

/// Safety screening mode for a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyMode {
	/// Lower thresholds — more aggressive quarantine.
	Strict,
	/// Default thresholds.
	Balanced,
	/// Screen but don't quarantine — everything is flagged or safe.
	Permissive,
}

impl std::fmt::Display for SafetyMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Strict => f.write_str("strict"),
			Self::Balanced => f.write_str("balanced"),
			Self::Permissive => f.write_str("permissive"),
		}
	}
}

impl SafetyMode {
	/// Parse from a string, defaulting to `Balanced`.
	pub fn from_str_or_default(s: &str) -> Self {
		match s {
			"strict" => Self::Strict,
			"balanced" => Self::Balanced,
			"permissive" => Self::Permissive,
			_ => Self::Balanced,
		}
	}
}

/// Per-source safety policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyPolicy {
	pub mode: SafetyMode,
	pub quarantine_threshold: u8,
	pub flag_threshold: u8,
	pub skip_screening: bool,
}

impl SafetyPolicy {
	/// Default policy derived from a trust tier.
	pub fn default_for_tier(tier: TrustTier) -> Self {
		match tier {
			TrustTier::Authored => Self {
				mode: SafetyMode::Balanced,
				quarantine_threshold: DEFAULT_QUARANTINE_THRESHOLD,
				flag_threshold: DEFAULT_FLAG_THRESHOLD,
				skip_screening: true,
			},
			TrustTier::Collaborative => Self {
				mode: SafetyMode::Balanced,
				quarantine_threshold: DEFAULT_QUARANTINE_THRESHOLD,
				flag_threshold: DEFAULT_FLAG_THRESHOLD,
				skip_screening: false,
			},
			TrustTier::External => Self {
				mode: SafetyMode::Strict,
				quarantine_threshold: 50,
				flag_threshold: 25,
				skip_screening: false,
			},
		}
	}
}

impl Default for SafetyPolicy {
	fn default() -> Self {
		Self::default_for_tier(TrustTier::External)
	}
}

/// Stub safety screening model (returns all safe).
pub struct SafetyModel;

impl SafetyModel {
	/// Create a new stub safety model.
	pub fn new(_cache_dir: &Path) -> Result<Self> {
		Ok(Self)
	}

	/// Screen a single piece of text (returns safe).
	pub async fn screen(&self, _text: &str) -> Result<SafetyVerdict> {
		Ok(SafetyVerdict {
			score: 0,
			is_malicious: false,
		})
	}

	/// Screen a batch of texts (returns all safe).
	pub async fn screen_batch(&self, texts: Vec<String>) -> Result<Vec<SafetyVerdict>> {
		Ok(texts
			.into_iter()
			.map(|_| SafetyVerdict {
				score: 0,
				is_malicious: false,
			})
			.collect())
	}
}
