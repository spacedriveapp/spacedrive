//! Output types for source path validation

use serde::{Deserialize, Serialize};
use specta::Type;

/// Risk level for adding a path as a source
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

/// Suggestion to track the whole drive instead
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WholeVolumeSuggestion {
	pub volume_fingerprint: String,
	pub volume_name: String,
	pub message: String,
}

/// Output from source path validation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateSourcePathOutput {
	/// Whether this path is recommended as a source root
	pub is_recommended: bool,
	/// Risk level assessment
	pub risk_level: RiskLevel,
	/// List of warnings (empty if no issues)
	pub warnings: Vec<ValidationWarning>,
	/// Alternative suggestion to track the whole drive
	pub suggested_alternative: Option<WholeVolumeSuggestion>,
	/// Path depth from root (number of components)
	pub path_depth: u32,
	/// Whether path is on the primary system volume
	pub is_on_primary_volume: bool,
}
