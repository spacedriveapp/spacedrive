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
use std::time::Instant;
use std::{path::Path, sync::Arc};

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
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
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
			action_context: None, // TODO: Pass action context from job state
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		// Read directory entries with per-dir FS timing
		match read_directory(&dir_path, volume_backend).await {
			Ok(entries) => {
				let entry_count = entries.len();
				let mut added_count = 0;

				for entry in entries {
					// Check for interruption during entry processing
					ctx.check_interrupt().await?;

					// Skip filtered entries via rules engine
					let decision = dir_ruler
						.evaluate_path(
							&entry.path,
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

		// State is automatically saved during job serialization on shutdown
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
///
/// Uses the provided volume backend if available, otherwise creates a LocalBackend fallback.
/// The backend is typically provided once per indexer job from the root volume lookup.
async fn read_directory(
	path: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<Vec<DirEntry>, std::io::Error> {
	// Use provided backend or create LocalBackend fallback
	let backend: Arc<dyn crate::volume::VolumeBackend> = match volume_backend {
		Some(backend) => Arc::clone(backend),
		None => {
			// Fallback: create temporary LocalBackend
			// This happens when no volume is tracked for the indexing path
			Arc::new(crate::volume::LocalBackend::new(
				path.parent().unwrap_or(path),
			))
		}
	};

	read_directory_with_backend(backend.as_ref(), path).await
}

/// Read a directory using a volume backend (local or cloud)
async fn read_directory_with_backend(
	backend: &dyn crate::volume::VolumeBackend,
	path: &Path,
) -> Result<Vec<DirEntry>, std::io::Error> {
	let t_rd_start = Instant::now();

	let raw_entries = backend
		.read_dir(path)
		.await
		.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

	// Convert RawDirEntry to DirEntry
	let entries: Vec<DirEntry> = raw_entries
		.into_iter()
		.map(|raw| DirEntry {
			path: path.join(&raw.name),
			kind: raw.kind,
			size: raw.size,
			modified: raw.modified,
			inode: raw.inode,
		})
		.collect();

	let rd_ms = t_rd_start.elapsed().as_millis();
	tracing::debug!(
		target: "indexing.discovery",
		"read_dir_metrics path={} rd_ms={} entries={}",
		path.display(), rd_ms, entries.len()
	);

	Ok(entries)
}
