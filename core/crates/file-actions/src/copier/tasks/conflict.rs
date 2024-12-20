use sd_core_job_system::job::JobError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Finds an available name for a file by appending a number if necessary
pub async fn find_available_name(path: impl AsRef<Path>) -> Result<PathBuf, JobError> {
	let path = path.as_ref();

	if !path.exists() {
		return Ok(path.to_owned());
	}

	let file_stem = path
		.file_stem()
		.and_then(|s| s.to_str())
		.ok_or_else(|| JobError::InvalidPath)?;

	let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

	let parent = path.parent().ok_or_else(|| JobError::InvalidPath)?;

	let mut counter = 1;
	loop {
		let new_name = if extension.is_empty() {
			format!("{} ({})", file_stem, counter)
		} else {
			format!("{} ({}).{}", file_stem, counter, extension)
		};

		let new_path = parent.join(new_name);
		if !new_path.exists() {
			return Ok(new_path);
		}
		counter += 1;
	}
}

/// Resolves name conflicts for a batch of files
pub async fn resolve_name_conflicts(
	files: Vec<(PathBuf, PathBuf)>,
) -> Result<Vec<(PathBuf, PathBuf)>, JobError> {
	let mut seen_paths = HashSet::new();
	let mut resolved = Vec::with_capacity(files.len());

	for (source, target) in files {
		let mut final_target = target.clone();

		// If we've seen this path before or if it exists on disk
		if seen_paths.contains(&target) || target.exists() {
			final_target = find_available_name(&target).await?;
		}

		seen_paths.insert(final_target.clone());
		resolved.push((source, final_target));
	}

	Ok(resolved)
}
