//! Output types for index verification

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Result of index integrity verification
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexVerifyOutput {
	/// Overall integrity status
	pub is_valid: bool,

	/// Integrity report with detailed findings
	pub report: IntegrityReport,

	/// Path that was verified
	pub path: PathBuf,

	/// Time taken to verify (seconds)
	pub duration_secs: f64,
}

/// Detailed integrity report
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrityReport {
	/// Total files found on filesystem
	pub filesystem_file_count: usize,

	/// Total files in database index
	pub database_file_count: usize,

	/// Total directories found on filesystem
	pub filesystem_dir_count: usize,

	/// Total directories in database index
	pub database_dir_count: usize,

	/// Files missing from index (on filesystem but not in DB)
	pub missing_from_index: Vec<IntegrityDifference>,

	/// Stale entries in index (in DB but not on filesystem)
	pub stale_in_index: Vec<IntegrityDifference>,

	/// Entries with incorrect metadata
	pub metadata_mismatches: Vec<IntegrityDifference>,

	/// Entries with incorrect parent relationships
	pub hierarchy_errors: Vec<IntegrityDifference>,

	/// Summary statistics
	pub summary: String,
}

impl IntegrityReport {
	pub fn new() -> Self {
		Self {
			filesystem_file_count: 0,
			database_file_count: 0,
			filesystem_dir_count: 0,
			database_dir_count: 0,
			missing_from_index: Vec::new(),
			stale_in_index: Vec::new(),
			metadata_mismatches: Vec::new(),
			hierarchy_errors: Vec::new(),
			summary: String::new(),
		}
	}

	pub fn is_valid(&self) -> bool {
		self.missing_from_index.is_empty()
			&& self.stale_in_index.is_empty()
			&& self.metadata_mismatches.is_empty()
			&& self.hierarchy_errors.is_empty()
	}

	pub fn total_issues(&self) -> usize {
		self.missing_from_index.len()
			+ self.stale_in_index.len()
			+ self.metadata_mismatches.len()
			+ self.hierarchy_errors.len()
	}

	pub fn generate_summary(&mut self) {
		if self.is_valid() {
			self.summary = format!(
				"✅ Index is valid! {} files and {} directories match filesystem perfectly.",
				self.filesystem_file_count, self.filesystem_dir_count
			);
		} else {
			let mut parts = Vec::new();

			if !self.missing_from_index.is_empty() {
				parts.push(format!(
					"{} missing from index",
					self.missing_from_index.len()
				));
			}

			if !self.stale_in_index.is_empty() {
				parts.push(format!("{} stale entries", self.stale_in_index.len()));
			}

			if !self.metadata_mismatches.is_empty() {
				parts.push(format!(
					"{} metadata mismatches",
					self.metadata_mismatches.len()
				));
			}

			if !self.hierarchy_errors.is_empty() {
				parts.push(format!("{} hierarchy errors", self.hierarchy_errors.len()));
			}

			self.summary = format!(
				"❌ Index has diverged: {}. Total issues: {}",
				parts.join(", "),
				self.total_issues()
			);
		}
	}
}

impl Default for IntegrityReport {
	fn default() -> Self {
		Self::new()
	}
}

/// Represents a single integrity difference
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrityDifference {
	/// Path relative to verification root
	pub path: PathBuf,

	/// Type of issue
	pub issue_type: IssueType,

	/// Expected value (from filesystem or correct state)
	pub expected: Option<String>,

	/// Actual value (from database)
	pub actual: Option<String>,

	/// Human-readable description
	pub description: String,

	/// Debug: database entry ID for investigation
	#[serde(skip_serializing_if = "Option::is_none")]
	pub db_entry_id: Option<i32>,

	/// Debug: database entry name
	#[serde(skip_serializing_if = "Option::is_none")]
	pub db_entry_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type")]
pub enum IssueType {
	MissingFromIndex,
	StaleInIndex,
	SizeMismatch,
	ModifiedTimeMismatch,
	InodeMismatch,
	ExtensionMismatch,
	ParentMismatch,
	KindMismatch,
}

impl IntegrityDifference {
	pub fn missing_from_index(path: PathBuf) -> Self {
		Self {
			description: format!(
				"File exists on filesystem but not in index: {}",
				path.display()
			),
			path,
			issue_type: IssueType::MissingFromIndex,
			expected: Some("Indexed".to_string()),
			actual: Some("Not indexed".to_string()),
			db_entry_id: None,
			db_entry_name: None,
		}
	}

	pub fn stale_in_index(path: PathBuf) -> Self {
		Self {
			description: format!(
				"Entry exists in index but not on filesystem: {}",
				path.display()
			),
			path,
			issue_type: IssueType::StaleInIndex,
			expected: Some("Not indexed".to_string()),
			actual: Some("Indexed".to_string()),
			db_entry_id: None,
			db_entry_name: None,
		}
	}

	pub fn size_mismatch(path: PathBuf, expected: u64, actual: u64) -> Self {
		Self {
			description: format!("Size mismatch for {}", path.display()),
			path,
			issue_type: IssueType::SizeMismatch,
			expected: Some(format!("{} bytes", expected)),
			actual: Some(format!("{} bytes", actual)),
			db_entry_id: None,
			db_entry_name: None,
		}
	}

	pub fn size_mismatch_with_debug(
		path: PathBuf,
		expected: u64,
		actual: u64,
		db_id: i32,
		db_name: String,
	) -> Self {
		Self {
			description: format!(
				"Size mismatch for {} (db_id: {}, db_name: {})",
				path.display(),
				db_id,
				db_name
			),
			path,
			issue_type: IssueType::SizeMismatch,
			expected: Some(format!("{} bytes", expected)),
			actual: Some(format!("{} bytes", actual)),
			db_entry_id: Some(db_id),
			db_entry_name: Some(db_name),
		}
	}

	pub fn modified_time_mismatch(path: PathBuf, expected: String, actual: String) -> Self {
		Self {
			description: format!("Modified time mismatch for {}", path.display()),
			path,
			issue_type: IssueType::ModifiedTimeMismatch,
			expected: Some(expected),
			actual: Some(actual),
			db_entry_id: None,
			db_entry_name: None,
		}
	}

	pub fn parent_mismatch(path: PathBuf, expected_parent: String, actual_parent: String) -> Self {
		Self {
			description: format!("Parent mismatch for {}", path.display()),
			path,
			issue_type: IssueType::ParentMismatch,
			expected: Some(expected_parent),
			actual: Some(actual_parent),
			db_entry_id: None,
			db_entry_name: None,
		}
	}
}
