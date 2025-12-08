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
use async_channel as chan;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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

/// Run the discovery phase of indexing with parallel directory walking
pub async fn run_discovery_phase(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
) -> Result<(), JobError> {
	let concurrency = state.discovery_concurrency;

	if concurrency <= 1 {
		// Fall back to sequential discovery for concurrency = 1
		return run_discovery_phase_sequential(
			state,
			ctx,
			root_path,
			rule_toggles,
			volume_backend,
			cloud_url_base,
		)
		.await;
	}

	ctx.log(format!(
		"Discovery phase starting from: {} (concurrency: {})",
		root_path.display(),
		concurrency
	));
	ctx.log(format!(
		"Initial directories to walk: {}",
		state.dirs_to_walk.len()
	));

	run_parallel_discovery(
		state,
		ctx,
		root_path,
		rule_toggles,
		volume_backend,
		cloud_url_base,
	)
	.await
}

/// Parallel discovery implementation using Rayon-style work-stealing
async fn run_parallel_discovery(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
) -> Result<(), JobError> {
	let concurrency = state.discovery_concurrency;

	// Use unbounded channels to avoid backpressure/deadlock issues
	let (work_tx, work_rx) = chan::unbounded::<PathBuf>();
	let (result_tx, result_rx) = chan::unbounded::<DiscoveryResult>();

	// Atomic counter tracking work in progress + shutdown signal
	// INVARIANT: incremented BEFORE sending to work channel, decremented AFTER processing
	let pending_work = Arc::new(AtomicUsize::new(0));
	let skipped_count = Arc::new(AtomicU64::new(0));
	let shutdown = Arc::new(AtomicBool::new(false));

	// Shared seen_paths across all workers to prevent duplicate processing
	// (handles symlink loops and same directory reached via different paths)
	let seen_paths = Arc::new(parking_lot::RwLock::new(std::collections::HashSet::new()));

	// Seed initial work
	while let Some(dir) = state.dirs_to_walk.pop_front() {
		pending_work.fetch_add(1, Ordering::Release);
		work_tx
			.send(dir)
			.await
			.map_err(|_| JobError::execution("Work channel closed"))?;
	}

	// Spawn worker tasks
	let mut workers = Vec::new();
	for worker_id in 0..concurrency {
		let work_rx = work_rx.clone();
		let work_tx = work_tx.clone();
		let result_tx = result_tx.clone();
		let pending_work = Arc::clone(&pending_work);
		let skipped_count = Arc::clone(&skipped_count);
		let shutdown = Arc::clone(&shutdown);
		let seen_paths = Arc::clone(&seen_paths);
		let root_path = root_path.to_path_buf();
		let volume_backend = volume_backend.cloned();
		let cloud_url_base = cloud_url_base.clone();

		let worker = tokio::spawn(async move {
			discovery_worker_rayon(
				worker_id,
				work_rx,
				work_tx,
				result_tx,
				pending_work,
				skipped_count,
				shutdown,
				seen_paths,
				root_path,
				rule_toggles,
				volume_backend,
				cloud_url_base,
			)
			.await
		});

		workers.push(worker);
	}

	// Monitor task: signals shutdown when all work is done
	let monitor = tokio::spawn({
		let shutdown = Arc::clone(&shutdown);
		let pending_work = Arc::clone(&pending_work);
		async move {
			loop {
				tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
				if pending_work.load(Ordering::Acquire) == 0 {
					shutdown.store(true, Ordering::Release);
					break;
				}
			}
		}
	});

	// Drop our copies so channels close when workers are done
	drop(work_tx);
	drop(result_tx);

	// Collect results
	let mut total_processed = 0u64;
	while let Ok(result) = result_rx.recv().await {
		match result {
			DiscoveryResult::Entry(entry) => {
				state.pending_entries.push(entry);
				total_processed += 1;

				if state.should_create_batch() {
					let batch = state.create_batch();
					state.entry_batches.push(batch);
				}
			}
			DiscoveryResult::Stats {
				files,
				dirs,
				symlinks,
				bytes,
			} => {
				state.stats.files += files;
				state.stats.dirs += dirs;
				state.stats.symlinks += symlinks;
				state.stats.bytes += bytes;
			}
			DiscoveryResult::Error(error) => {
				state.add_error(error);
			}
			DiscoveryResult::Progress { dirs_queued } => {
				let indexer_progress = IndexerProgress {
					phase: IndexPhase::Discovery { dirs_queued },
					current_path: root_path.display().to_string(),
					total_found: state.stats,
					processing_rate: state.calculate_rate(),
					estimated_remaining: state.estimate_remaining(),
					scope: None,
					persistence: None,
					is_ephemeral: false,
					action_context: None,
				};
				ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));
				state.items_since_last_update += 1;
			}
			DiscoveryResult::QueueDirectories(_) => {
				// Workers queue directly, this shouldn't happen
				unreachable!("Workers should not send QueueDirectories in Rayon-style mode");
			}
		}

		ctx.check_interrupt().await?;
	}

	// Wait for monitor and workers
	monitor
		.await
		.map_err(|e| JobError::execution(format!("Monitor task failed: {}", e)))?;

	for worker in workers {
		worker
			.await
			.map_err(|e| JobError::execution(format!("Worker task failed: {}", e)))?;
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

	let skipped = skipped_count.load(Ordering::SeqCst);
	state.stats.skipped = skipped;

	ctx.log(format!(
		"Parallel discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} batches created",
		state.stats.files,
		state.stats.dirs,
		state.stats.symlinks,
		skipped,
		state.entry_batches.len()
	));

	state.phase = crate::ops::indexing::state::Phase::Processing;
	Ok(())
}

/// Result types sent from workers back to coordinator
enum DiscoveryResult {
	Entry(DirEntry),
	QueueDirectories(Vec<PathBuf>),
	Stats {
		files: u64,
		dirs: u64,
		symlinks: u64,
		bytes: u64,
	},
	Error(IndexError),
	Progress {
		dirs_queued: usize,
	},
}

/// Rayon-style worker: processes directories and directly enqueues new work
async fn discovery_worker_rayon(
	_worker_id: usize,
	work_rx: chan::Receiver<PathBuf>,
	work_tx: chan::Sender<PathBuf>,
	result_tx: chan::Sender<DiscoveryResult>,
	pending_work: Arc<AtomicUsize>,
	skipped_count: Arc<AtomicU64>,
	shutdown: Arc<AtomicBool>,
	seen_paths: Arc<parking_lot::RwLock<std::collections::HashSet<PathBuf>>>,
	root_path: PathBuf,
	rule_toggles: RuleToggles,
	volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
) {
	loop {
		// Check shutdown signal
		if shutdown.load(Ordering::Acquire) {
			break;
		}

		// Try to get work with a timeout to periodically check shutdown
		let dir_path = match tokio::time::timeout(
			tokio::time::Duration::from_millis(50),
			work_rx.recv(),
		)
		.await
		{
			Ok(Ok(path)) => path,
			Ok(Err(_)) => break, // Channel closed
			Err(_) => continue,  // Timeout, check shutdown flag again
		};

		// Skip if already seen (handles symlink loops across ALL workers)
		{
			let mut seen = seen_paths.write();
			if !seen.insert(dir_path.clone()) {
				pending_work.fetch_sub(1, Ordering::Release);
				continue;
			}
		}

		// Build rules for this directory
		let dir_ruler = build_default_ruler(rule_toggles, &root_path, &dir_path).await;

		// Read directory
		match read_directory(
			&dir_path,
			volume_backend.as_ref(),
			cloud_url_base.as_deref(),
		)
		.await
		{
			Ok(entries) => {
				let mut local_stats = LocalStats::default();

				for entry in entries {
					// Apply rules
					let decision = dir_ruler
						.evaluate_path(
							&entry.path,
							&SimpleMetadata {
								is_dir: matches!(entry.kind, EntryKind::Directory),
							},
						)
						.await;

					if matches!(decision, Ok(RulerDecision::Reject)) {
						skipped_count.fetch_add(1, Ordering::Relaxed);
						continue;
					}

					if let Err(err) = decision {
						let _ = result_tx
							.send(DiscoveryResult::Error(IndexError::FilterCheck {
								path: entry.path.to_string_lossy().to_string(),
								error: err.to_string(),
							}))
							.await;
						continue;
					}

					match entry.kind {
						EntryKind::Directory => {
							local_stats.dirs += 1;
							// Rayon-style: increment BEFORE queueing, worker directly enqueues
							pending_work.fetch_add(1, Ordering::Release);
							if work_tx.send(entry.path.clone()).await.is_err() {
								// Channel closed, decrement and continue
								pending_work.fetch_sub(1, Ordering::Release);
							}
							let _ = result_tx.send(DiscoveryResult::Entry(entry)).await;
						}
						EntryKind::File => {
							local_stats.files += 1;
							local_stats.bytes += entry.size;
							let _ = result_tx.send(DiscoveryResult::Entry(entry)).await;
						}
						EntryKind::Symlink => {
							local_stats.symlinks += 1;
							let _ = result_tx.send(DiscoveryResult::Entry(entry)).await;
						}
					}
				}

				// Send stats update
				let _ = result_tx
					.send(DiscoveryResult::Stats {
						files: local_stats.files,
						dirs: local_stats.dirs,
						symlinks: local_stats.symlinks,
						bytes: local_stats.bytes,
					})
					.await;

				// Send progress update
				let dirs_queued = pending_work.load(Ordering::Acquire);
				let _ = result_tx
					.send(DiscoveryResult::Progress { dirs_queued })
					.await;
			}
			Err(e) => {
				let _ = result_tx
					.send(DiscoveryResult::Error(IndexError::ReadDir {
						path: dir_path.to_string_lossy().to_string(),
						error: e.to_string(),
					}))
					.await;
			}
		}

		// Decrement AFTER processing complete
		pending_work.fetch_sub(1, Ordering::Release);
	}
}

#[derive(Default)]
struct LocalStats {
	files: u64,
	dirs: u64,
	symlinks: u64,
	bytes: u64,
}

/// Sequential discovery fallback (original implementation)
async fn run_discovery_phase_sequential(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
) -> Result<(), JobError> {
	ctx.log(format!(
		"Discovery phase starting from: {} (sequential mode)",
		root_path.display()
	));

	let mut skipped_count = 0u64;

	while let Some(dir_path) = state.dirs_to_walk.pop_front() {
		ctx.check_interrupt().await?;

		if !state.seen_paths.insert(dir_path.clone()) {
			continue;
		}

		let dir_ruler = build_default_ruler(rule_toggles, root_path, &dir_path).await;

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
			action_context: None,
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		match read_directory(&dir_path, volume_backend, cloud_url_base.as_deref()).await {
			Ok(entries) => {
				let entry_count = entries.len();
				let mut added_count = 0;

				for entry in entries {
					ctx.check_interrupt().await?;

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

		state.items_since_last_update += 1;
	}

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
	cloud_url_base: Option<&str>,
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

	read_directory_with_backend(backend.as_ref(), path, cloud_url_base).await
}

/// Read a directory using a volume backend (local or cloud)
async fn read_directory_with_backend(
	backend: &dyn crate::volume::VolumeBackend,
	path: &Path,
	cloud_url_base: Option<&str>,
) -> Result<Vec<DirEntry>, std::io::Error> {
	let t_rd_start = Instant::now();

	let raw_entries = backend
		.read_dir(path)
		.await
		.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

	// Convert RawDirEntry to DirEntry
	let entries: Vec<DirEntry> = raw_entries
		.into_iter()
		.map(|raw| {
			// For cloud volumes, prepend the cloud URL base to build proper hierarchical paths
			let full_path = if let Some(base) = cloud_url_base {
				// Cloud: s3://bucket/ + relative_path + filename
				let relative = path.to_string_lossy();
				let joined = if relative.is_empty() {
					raw.name.clone()
				} else {
					format!("{}/{}", relative.trim_end_matches('/'), raw.name)
				};
				PathBuf::from(format!("{}{}", base, joined))
			} else {
				// Local: just join normally
				path.join(&raw.name)
			};

			DirEntry {
				path: full_path,
				kind: raw.kind,
				size: raw.size,
				modified: raw.modified,
				inode: raw.inode,
			}
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
