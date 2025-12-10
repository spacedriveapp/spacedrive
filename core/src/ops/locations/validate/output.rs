//! Output types for location path validation

use serde::{Deserialize, Serialize};
use specta::Type;

/// Risk level for adding a path as a location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
	/// Safe - nested path in user directories
	Low,
	/// Caution - shallow path on primary volume (e.g., /Users/jamie)
	Medium,
	/// Warning - system directory or root-level path (e.g., /, /System)
	High,
}

/// A validation warning message
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidationWarning {
	pub message: String,
	pub suggestion: Option<String>,
}

/// Suggestion to use volume indexing instead
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeIndexingSuggestion {
	pub volume_fingerprint: String,
	pub volume_name: String,
	pub message: String,
}

/// Output from location path validation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateLocationPathOutput {
	/// Whether this path is recommended for use as a location
	pub is_recommended: bool,
	/// Risk level assessment
	pub risk_level: RiskLevel,
	/// List of warnings (empty if no issues)
	pub warnings: Vec<ValidationWarning>,
	/// Alternative suggestion to use volume indexing
	pub suggested_alternative: Option<VolumeIndexingSuggestion>,
	/// Path depth from root (number of components)
	pub path_depth: u32,
	/// Whether path is on the primary system volume
	pub is_on_primary_volume: bool,
}
