//! Discovery phase - walks directories and collects entries

use crate::{
	infra::job::generic_progress::ToGenericProgress,
	infra::job::prelude::{JobContext, JobError, Progress},
	ops::indexing::{
		entry::EntryProcessor,
		rules::{build_default_ruler, RuleToggles, RulerDecision},
		state::{DirEntry, EntryKind, IndexError, IndexPhase, IndexerProgress, IndexerState},
	},
};
use std::path::Path;
use std::time::Instant;

struct SimpleMetadata {
	is_dir: bool,
}
impl crate::ops::indexing::rules::MetadataForIndexerRules for SimpleMetadata {
	fn is_dir(&self) -> bool {
		self.is_dir
	}
}

/// Run the discovery phase of indexing
pub async fn run_discovery_phase(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
) -> Result<(), JobError> {
	ctx.log(format!(
		"Discovery phase starting from: {}",
		root_path.display()
	));
	ctx.log(format!(
		"Initial directories to walk: {}",
		state.dirs_to_walk.len()
	));

	let mut skipped_count = 0u64;

	let toggles = rule_toggles;

	while let Some(dir_path) = state.dirs_to_walk.pop_front() {
		ctx.check_interrupt().await?;

		// Skip if already seen (handles symlink loops)
		if !state.seen_paths.insert(dir_path.clone()) {
			continue;
		}

		// Build rules in the context of the current directory for gitignore behavior
		let dir_ruler = build_default_ruler(toggles, root_path, &dir_path).await;

		// Do not skip the directory itself by rules; only apply rules to its entries

		// Update progress
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::Discovery {
				dirs_queued: state.dirs_to_walk.len(),
			},
			current_path: dir_path.to_string_lossy().to_string(),
			total_found: state.stats,
			processing_rate: state.calculate_rate(),
			estimated_remaining: state.estimate_remaining(),
			scope: None,
			persistence: None,
			is_ephemeral: false,
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		// Read directory entries with per-dir FS timing
		match read_directory(&dir_path).await {
			Ok(entries) => {
				let entry_count = entries.len();
				let mut added_count = 0;

				for entry in entries {
					// Check for interruption during entry processing
					ctx.check_interrupt().await?;

					// Skip filtered entries via rules engine (match against basename to avoid ancestor effects)
					let name_path = entry
						.path
						.file_name()
						.map(|n| std::path::PathBuf::from(n))
						.unwrap_or_else(|| entry.path.clone());
					let decision = dir_ruler
						.evaluate_path(
							&name_path,
							&SimpleMetadata {
								is_dir: matches!(entry.kind, EntryKind::Directory),
							},
						)
						.await;
					if matches!(decision, Ok(RulerDecision::Reject)) {
						state.stats.skipped += 1;
						skipped_count += 1;
						eprintln!("[discovery] Filtered entry: {}", entry.path.display());
						continue;
					}
					if let Err(err) = decision {
						state.add_error(IndexError::FilterCheck {
							path: entry.path.to_string_lossy().to_string(),
							error: err.to_string(),
						});
					}

					match entry.kind {
						EntryKind::Directory => {
							state.dirs_to_walk.push_back(entry.path.clone());
							state.stats.dirs += 1;
							state.pending_entries.push(entry);
							added_count += 1;
						}
						EntryKind::File => {
							state.stats.bytes += entry.size;
							state.stats.files += 1;
							state.pending_entries.push(entry);
							added_count += 1;
						}
						EntryKind::Symlink => {
							state.stats.symlinks += 1;
							state.pending_entries.push(entry);
							added_count += 1;
						}
					}
				}

				if added_count > 0 {
					ctx.log(format!(
						"Found {} entries in {} ({} filtered)",
						entry_count,
						dir_path.display(),
						entry_count - added_count
					));
				}

				// Batch entries
				if state.should_create_batch() {
					let batch = state.create_batch();
					state.entry_batches.push(batch);
				}
			}
			Err(e) => {
				let error_msg = format!("Failed to read {}: {}", dir_path.display(), e);
				ctx.add_non_critical_error(error_msg);
				state.add_error(IndexError::ReadDir {
					path: dir_path.to_string_lossy().to_string(),
					error: e.to_string(),
				});
			}
		}

		// Update rate tracking
		state.items_since_last_update += 1;

		// Periodic checkpoint
		if state.stats.files % 5000 == 0 {
			ctx.checkpoint_with_state(state).await?;
		}
	}

	// Final batch
	if !state.pending_entries.is_empty() {
		let final_batch_size = state.pending_entries.len();
		ctx.log(format!(
			"Creating final batch with {} entries",
			final_batch_size
		));
		let batch = state.create_batch();
		state.entry_batches.push(batch);
	}

	ctx.log(format!(
		"Discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} batches created",
		state.stats.files,
		state.stats.dirs,
		state.stats.symlinks,
		skipped_count,
		state.entry_batches.len()
	));

	state.phase = crate::ops::indexing::state::Phase::Processing;
	Ok(())
}

/// Read a directory and extract metadata
async fn read_directory(path: &Path) -> Result<Vec<DirEntry>, std::io::Error> {
	let mut entries = Vec::new();
	let t_rd_start = Instant::now();
	let mut dir = tokio::fs::read_dir(path).await?;
	let mut metadata_ms: u128 = 0;

	while let Some(entry) = dir.next_entry().await? {
		let t_meta = Instant::now();
		let metadata = match entry.metadata().await {
			Ok(m) => m,
			Err(_) => continue, // Skip entries we can't read
		};
		metadata_ms += t_meta.elapsed().as_millis();

		let kind = if metadata.is_dir() {
			EntryKind::Directory
		} else if metadata.is_symlink() {
			EntryKind::Symlink
		} else {
			EntryKind::File
		};

		// Extract inode if available
		let inode = EntryProcessor::get_inode(&metadata);

		entries.push(DirEntry {
			path: entry.path(),
			kind,
			size: metadata.len(),
			modified: metadata.modified().ok(),
			inode,
		});
	}
	let rd_ms = t_rd_start.elapsed().as_millis();
	// Best-effort: attach to a log line so bench parser can extract later (Phase 1)
	tracing::debug!(
		target: "indexing.discovery",
		"read_dir_metrics path={} rd_ms={} metadata_ms={}",
		path.display(), rd_ms, metadata_ms
	);
	Ok(entries)
}
