//! File validation and integrity checking job

use crate::{
	domain::addressing::{SdPath, SdPathBatch},
	domain::content_identity::ContentHashGenerator,
	infra::jobs::prelude::*,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	path::PathBuf,
	time::{Duration, Instant},
};
use tokio::fs;
use uuid::Uuid;

/// File validation modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationMode {
	/// Check file accessibility and basic metadata
	Basic,
	/// Verify file integrity using CAS ID
	Integrity,
	/// Check for corruption patterns
	Corruption,
	/// Full validation including content verification
	Complete,
}

/// Validation severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
	Info,
	Warning,
	Error,
	Critical,
}

/// File validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
	pub path: SdPath,
	pub issue_type: String,
	pub severity: ValidationSeverity,
	pub description: String,
	pub suggested_action: Option<String>,
}

/// File validation job
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationJob {
	pub targets: SdPathBatch,
	pub mode: ValidationMode,
	pub verify_against_index: bool,
	pub check_permissions: bool,

	// Internal state for resumption
	#[serde(skip)]
	validated_files: Vec<PathBuf>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

/// Validation progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationProgress {
	pub current_file: String,
	pub files_validated: usize,
	pub total_files: usize,
	pub issues_found: usize,
	pub bytes_validated: u64,
	pub current_operation: String,
}

impl JobProgress for ValidationProgress {}

impl Job for ValidationJob {
	const NAME: &'static str = "file_validation";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Validate file integrity and accessibility");
}

impl crate::infra::jobs::traits::DynJob for ValidationJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	// ValidationJob doesn't track specific entry resources, so use default None
}

#[async_trait::async_trait]
impl JobHandler for ValidationJob {
	type Output = ValidationOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		ctx.log(format!(
			"Starting file validation with mode: {:?}",
			self.mode
		));

		let mut issues = Vec::new();
		let mut validated_count = 0;
		let mut total_bytes_validated = 0u64;

		// Collect all files to validate
		let files_to_validate = self.collect_files(&ctx).await?;
		let total_files = files_to_validate.len();

		ctx.log(format!("Found {} files to validate", total_files));

		for (index, file_info) in files_to_validate.iter().enumerate() {
			ctx.check_interrupt().await?;

			ctx.progress(Progress::structured(ValidationProgress {
				current_file: file_info.path.display(),
				files_validated: validated_count,
				total_files,
				issues_found: issues.len(),
				bytes_validated: total_bytes_validated,
				current_operation: self.get_operation_name(),
			}));

			match self.validate_file(file_info, &ctx).await {
				Ok(file_issues) => {
					issues.extend(file_issues);
					validated_count += 1;
					total_bytes_validated += file_info.size;
				}
				Err(e) => {
					issues.push(ValidationIssue {
						path: file_info.path.clone(),
						issue_type: "ValidationError".to_string(),
						severity: ValidationSeverity::Error,
						description: format!("Failed to validate file: {}", e),
						suggested_action: Some(
							"Check file permissions and accessibility".to_string(),
						),
					});
					ctx.add_non_critical_error(format!(
						"Failed to validate {}: {}",
						file_info.path.display(),
						e
					));
				}
			}

			// Checkpoint every 50 files
			if index % 50 == 0 {
				ctx.checkpoint().await?;
			}
		}

		let error_count = issues
			.iter()
			.filter(|i| {
				matches!(
					i.severity,
					ValidationSeverity::Error | ValidationSeverity::Critical
				)
			})
			.count();
		let warning_count = issues
			.iter()
			.filter(|i| matches!(i.severity, ValidationSeverity::Warning))
			.count();

		ctx.log(format!(
			"Validation completed: {} files validated, {} errors, {} warnings",
			validated_count, error_count, warning_count
		));

		Ok(ValidationOutput {
			validated_count,
			total_files,
			issues,
			total_bytes_validated,
			duration: self.started_at.elapsed(),
			validation_mode: self.mode.clone(),
		})
	}
}

/// File information for validation
#[derive(Debug, Clone)]
struct FileValidationInfo {
	path: SdPath,
	size: u64,
	modified: Option<std::time::SystemTime>,
	permissions: Option<std::fs::Permissions>,
}

impl ValidationJob {
	/// Create a new validation job
	pub fn new(targets: SdPathBatch, mode: ValidationMode) -> Self {
		Self {
			targets,
			mode,
			verify_against_index: true,
			check_permissions: true,
			validated_files: Vec::new(),
			started_at: Instant::now(),
		}
	}

	/// Set whether to verify against the file index
	pub fn with_index_verification(mut self, verify: bool) -> Self {
		self.verify_against_index = verify;
		self
	}

	/// Set whether to check file permissions
	pub fn with_permission_check(mut self, check: bool) -> Self {
		self.check_permissions = check;
		self
	}

	/// Collect all files to validate
	async fn collect_files(&self, ctx: &JobContext<'_>) -> JobResult<Vec<FileValidationInfo>> {
		let mut files = Vec::new();

		for target in &self.targets.paths {
			ctx.check_interrupt().await?;

			if let Some(local_path) = target.as_local_path() {
				self.collect_files_recursive(local_path, target, &mut files, ctx)
					.await?;
			}
		}

		Ok(files)
	}

	/// Collect files from a directory using iterative approach
	async fn collect_files_recursive(
		&self,
		path: &std::path::Path,
		sd_path: &SdPath,
		files: &mut Vec<FileValidationInfo>,
		ctx: &JobContext<'_>,
	) -> JobResult<()> {
		let mut stack = vec![(path.to_path_buf(), sd_path.clone())];

		while let Some((current_path, current_sd_path)) = stack.pop() {
			ctx.check_interrupt().await?;

			let metadata = fs::metadata(&current_path).await?;

			if metadata.is_file() {
				files.push(FileValidationInfo {
					path: current_sd_path,
					size: metadata.len(),
					modified: metadata.modified().ok(),
					permissions: std::fs::metadata(&current_path)
						.ok()
						.map(|m| m.permissions()),
				});
			} else if metadata.is_dir() {
				let mut dir = fs::read_dir(&current_path).await?;

				while let Some(entry) = dir.next_entry().await? {
					let entry_path = entry.path();
					let entry_sd_path = current_sd_path.join(entry.file_name());
					stack.push((entry_path, entry_sd_path));
				}
			}
		}

		Ok(())
	}

	/// Validate a single file
	async fn validate_file(
		&self,
		file_info: &FileValidationInfo,
		ctx: &JobContext<'_>,
	) -> JobResult<Vec<ValidationIssue>> {
		let mut issues = Vec::new();

		if let Some(local_path) = file_info.path.as_local_path() {
			// Basic validation
			issues.extend(self.validate_basic(file_info, local_path).await?);

			// Mode-specific validation
			match self.mode {
				ValidationMode::Basic => {
					// Basic validation already done
				}
				ValidationMode::Integrity => {
					issues.extend(self.validate_integrity(file_info, local_path, ctx).await?);
				}
				ValidationMode::Corruption => {
					issues.extend(self.validate_corruption(file_info, local_path).await?);
				}
				ValidationMode::Complete => {
					issues.extend(self.validate_integrity(file_info, local_path, ctx).await?);
					issues.extend(self.validate_corruption(file_info, local_path).await?);
					issues.extend(self.validate_comprehensive(file_info, local_path).await?);
				}
			}
		}

		Ok(issues)
	}

	/// Basic file validation
	async fn validate_basic(
		&self,
		file_info: &FileValidationInfo,
		local_path: &std::path::Path,
	) -> JobResult<Vec<ValidationIssue>> {
		let mut issues = Vec::new();

		// Check if file exists and is accessible
		if !fs::try_exists(local_path).await.unwrap_or(false) {
			issues.push(ValidationIssue {
				path: file_info.path.clone(),
				issue_type: "FileNotFound".to_string(),
				severity: ValidationSeverity::Critical,
				description: "File does not exist or is not accessible".to_string(),
				suggested_action: Some("Check if file was moved or deleted".to_string()),
			});
			return Ok(issues);
		}

		// Check metadata consistency
		if let Ok(current_metadata) = fs::metadata(local_path).await {
			if current_metadata.len() != file_info.size {
				issues.push(ValidationIssue {
					path: file_info.path.clone(),
					issue_type: "SizeMismatch".to_string(),
					severity: ValidationSeverity::Warning,
					description: format!(
						"File size changed: expected {}, found {}",
						file_info.size,
						current_metadata.len()
					),
					suggested_action: Some("File may have been modified".to_string()),
				});
			}

			// Check modification time if available
			if let (Some(expected_modified), Ok(current_modified)) =
				(file_info.modified, current_metadata.modified())
			{
				if expected_modified != current_modified {
					issues.push(ValidationIssue {
						path: file_info.path.clone(),
						issue_type: "ModificationTimeChanged".to_string(),
						severity: ValidationSeverity::Info,
						description: "File modification time has changed".to_string(),
						suggested_action: None,
					});
				}
			}
		}

		// Check permissions if enabled
		if self.check_permissions {
			if let Some(ref expected_permissions) = file_info.permissions {
				if let Ok(current_metadata) = std::fs::metadata(local_path) {
					let current_permissions = current_metadata.permissions();
					if current_permissions.readonly() != expected_permissions.readonly() {
						issues.push(ValidationIssue {
							path: file_info.path.clone(),
							issue_type: "PermissionChanged".to_string(),
							severity: ValidationSeverity::Warning,
							description: "File permissions have changed".to_string(),
							suggested_action: Some(
								"Check if permission change was intentional".to_string(),
							),
						});
					}
				}
			}
		}

		Ok(issues)
	}

	/// Integrity validation using CAS ID
	async fn validate_integrity(
		&self,
		file_info: &FileValidationInfo,
		local_path: &std::path::Path,
		ctx: &JobContext<'_>,
	) -> JobResult<Vec<ValidationIssue>> {
		let mut issues = Vec::new();

		// For integrity validation, we would need to compare against stored CAS ID
		// This is a placeholder implementation
		let file_size = file_info.size as u64;
		match ContentHashGenerator::generate_full_hash(local_path, file_size).await {
			Ok(current_cas_id) => {
				// Here we would compare against the stored CAS ID from the database
				// For now, we just verify that we can generate one
				ctx.log(format!(
					"Generated CAS ID for validation: {}",
					&current_cas_id[..16]
				));
			}
			Err(e) => {
				issues.push(ValidationIssue {
					path: file_info.path.clone(),
					issue_type: "IntegrityCheckFailed".to_string(),
					severity: ValidationSeverity::Error,
					description: format!("Failed to compute content hash: {}", e),
					suggested_action: Some("File may be corrupted or inaccessible".to_string()),
				});
			}
		}

		Ok(issues)
	}

	/// Check for file corruption patterns
	async fn validate_corruption(
		&self,
		file_info: &FileValidationInfo,
		local_path: &std::path::Path,
	) -> JobResult<Vec<ValidationIssue>> {
		let mut issues = Vec::new();

		// Check for zero-byte files (unless expected)
		if file_info.size == 0 {
			issues.push(ValidationIssue {
				path: file_info.path.clone(),
				issue_type: "EmptyFile".to_string(),
				severity: ValidationSeverity::Warning,
				description: "File is empty (0 bytes)".to_string(),
				suggested_action: Some("Verify if this is expected".to_string()),
			});
		}

		// Check for extremely large files that might indicate corruption
		if file_info.size > 100 * 1024 * 1024 * 1024 {
			// 100GB
			issues.push(ValidationIssue {
				path: file_info.path.clone(),
				issue_type: "UnusuallyLargeFile".to_string(),
				severity: ValidationSeverity::Info,
				description: format!("File is very large: {} bytes", file_info.size),
				suggested_action: Some("Verify file integrity if unexpected".to_string()),
			});
		}

		// Check file extension vs content (basic check)
		if let Some(extension) = local_path.extension().and_then(|e| e.to_str()) {
			match extension.to_lowercase().as_str() {
				"txt" | "md" | "json" | "xml" | "csv" => {
					// Text files - check for binary content in beginning
					if let Ok(mut file) = tokio::fs::File::open(local_path).await {
						use tokio::io::AsyncReadExt;
						let mut buffer = [0; 1024];
						if let Ok(bytes_read) = file.read(&mut buffer).await {
							let content = &buffer[..bytes_read];
							if content
								.iter()
								.any(|&b| b < 32 && b != 9 && b != 10 && b != 13)
							{
								issues.push(ValidationIssue {
									path: file_info.path.clone(),
									issue_type: "SuspiciousContent".to_string(),
									severity: ValidationSeverity::Warning,
									description: "Text file contains binary data".to_string(),
									suggested_action: Some(
										"Verify file type is correct".to_string(),
									),
								});
							}
						}
					}
				}
				_ => {} // Other file types - could add more specific checks
			}
		}

		Ok(issues)
	}

	/// Comprehensive validation
	async fn validate_comprehensive(
		&self,
		file_info: &FileValidationInfo,
		local_path: &std::path::Path,
	) -> JobResult<Vec<ValidationIssue>> {
		let mut issues = Vec::new();

		// Check file name validity
		if let Some(filename) = local_path.file_name().and_then(|n| n.to_str()) {
			// Check for problematic characters
			let problematic_chars = ['<', '>', ':', '"', '|', '?', '*'];
			if problematic_chars.iter().any(|&c| filename.contains(c)) {
				issues.push(ValidationIssue {
					path: file_info.path.clone(),
					issue_type: "ProblematicFilename".to_string(),
					severity: ValidationSeverity::Warning,
					description: "Filename contains characters that may cause issues".to_string(),
					suggested_action: Some("Consider renaming file".to_string()),
				});
			}

			// Check for very long filenames
			if filename.len() > 255 {
				issues.push(ValidationIssue {
					path: file_info.path.clone(),
					issue_type: "LongFilename".to_string(),
					severity: ValidationSeverity::Warning,
					description: "Filename is very long and may not be compatible with all systems"
						.to_string(),
					suggested_action: Some("Consider shortening filename".to_string()),
				});
			}
		}

		// Check path depth
		let path_components = local_path.components().count();
		if path_components > 32 {
			issues.push(ValidationIssue {
				path: file_info.path.clone(),
				issue_type: "DeepPath".to_string(),
				severity: ValidationSeverity::Info,
				description: "File is located very deep in directory structure".to_string(),
				suggested_action: Some("Consider reorganizing directory structure".to_string()),
			});
		}

		Ok(issues)
	}

	/// Get operation name for progress display
	fn get_operation_name(&self) -> String {
		match self.mode {
			ValidationMode::Basic => "Basic validation".to_string(),
			ValidationMode::Integrity => "Integrity checking".to_string(),
			ValidationMode::Corruption => "Corruption detection".to_string(),
			ValidationMode::Complete => "Complete validation".to_string(),
		}
	}
}

/// Job output for file validation
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationOutput {
	pub validated_count: usize,
	pub total_files: usize,
	pub issues: Vec<ValidationIssue>,
	pub total_bytes_validated: u64,
	pub duration: Duration,
	pub validation_mode: ValidationMode,
}

impl From<ValidationOutput> for JobOutput {
	fn from(output: ValidationOutput) -> Self {
		JobOutput::FileValidation {
			validated_count: output.validated_count,
			issues_found: output.issues.len(),
			total_bytes_validated: output.total_bytes_validated,
		}
	}
}
