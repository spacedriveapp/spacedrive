use sd_core_job_system::job::JobError;
use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_TOTAL_SIZE_PER_STEP: u64 = 1024 * 1024 * 800; // 800MB
const MAX_FILES_PER_STEP: usize = 20;

pub struct BatchedCopy {
	pub sources: Vec<PathBuf>,
	pub targets: Vec<PathBuf>,
	pub total_size: u64,
}

/// Gather information about the list of files and decide what is the best
/// approach to organize them into batches.
pub async fn batch_copy_files(
	files: Vec<(PathBuf, PathBuf)>,
) -> Result<Vec<BatchedCopy>, JobError> {
	let mut batches = Vec::new();
	let mut current_batch = BatchedCopy {
		sources: Vec::new(),
		targets: Vec::new(),
		total_size: 0,
	};

	for (source, target) in files {
		let file_size = fs::metadata(&source)
			.await
			.map_err(|e| JobError::IO(e.into()))?
			.len();

		// If adding this file would exceed our batch limits, create a new batch
		if current_batch.sources.len() >= MAX_FILES_PER_STEP
			|| current_batch.total_size + file_size > MAX_TOTAL_SIZE_PER_STEP
		{
			if !current_batch.sources.is_empty() {
				batches.push(current_batch);
				current_batch = BatchedCopy {
					sources: Vec::new(),
					targets: Vec::new(),
					total_size: 0,
				};
			}
		}

		current_batch.sources.push(source);
		current_batch.targets.push(target);
		current_batch.total_size += file_size;
	}

	// Push any remaining files
	if !current_batch.sources.is_empty() {
		batches.push(current_batch);
	}

	Ok(batches)
}

/// Recursively collect all files and directories that need to be copied
pub async fn collect_copy_entries(
	source: impl AsRef<Path>,
	target: impl AsRef<Path>,
) -> Result<(Vec<(PathBuf, PathBuf)>, Vec<(PathBuf, PathBuf)>), JobError> {
	let source = source.as_ref();
	let target = target.as_ref();

	let mut files = Vec::new();
	let mut dirs = Vec::new();

	let mut entries = fs::read_dir(source)
		.await
		.map_err(|e| JobError::IO(e.into()))?;

	while let Some(entry) = entries
		.next_entry()
		.await
		.map_err(|e| JobError::IO(e.into()))?
	{
		let source_path = entry.path();
		let relative_path = source_path.strip_prefix(source).unwrap();
		let target_path = target.join(relative_path);

		let file_type = entry
			.file_type()
			.await
			.map_err(|e| JobError::IO(e.into()))?;

		if file_type.is_dir() {
			dirs.push((source_path.clone(), target_path.clone()));
			let (sub_files, sub_dirs) = collect_copy_entries(&source_path, &target_path).await?;
			files.extend(sub_files);
			dirs.extend(sub_dirs);
		} else {
			files.push((source_path, target_path));
		}
	}

	Ok((files, dirs))
}
