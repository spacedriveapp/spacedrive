//! # Directory Discovery Phase
//!
//! `core::ops::indexing::phases::discovery` implements parallel directory traversal
//! using a work-stealing pattern inspired by Rayon. Workers pull directories from a
//! shared queue, read their contents, filter entries against indexing rules, and
//! directly enqueue subdirectories for other workers to process.
//!
//! ## Modified-Time Pruning (Stale Detection)
//!
//! When `IndexMode::Stale` is used, the discovery phase enables mtime pruning.
//! This compares directory modified times between the filesystem and database,
//! skipping unchanged branches to dramatically reduce scanning overhead for
//! stale detection scenarios.

use crate::{
	domain::location::IndexMode,
	infra::job::generic_progress::ToGenericProgress,
	infra::job::prelude::{JobContext, JobError, Progress},
	ops::indexing::{
		database_storage::DatabaseStorage,
		rules::{build_default_ruler, RuleToggles, RulerDecision},
		state::{DirEntry, EntryKind, IndexError, IndexPhase, IndexerProgress, IndexerState},
	},
};
use async_channel as chan;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Instant, SystemTime};
use std::{path::Path, sync::Arc};

struct SimpleMetadata {
	is_dir: bool,
}
impl crate::ops::indexing::rules::MetadataForIndexerRules for SimpleMetadata {
	fn is_dir(&self) -> bool {
		self.is_dir
	}
}

/// Runs parallel directory discovery or falls back to sequential for concurrency = 1.
///
/// Spawns worker tasks that walk the directory tree, apply filtering rules, and collect
/// entries into batches for the processing phase. Falls back to sequential traversal
/// when concurrency is 1 to avoid task spawning overhead for single-threaded scenarios.
///
/// ## Mtime Pruning
///
/// When `index_mode.uses_mtime_pruning()` returns true, the discovery phase compares
/// directory modified times against the database. Directories with matching mtimes
/// (within 1-second tolerance) are pruned, skipping their entire subtree.
pub async fn run_discovery_phase(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
	index_mode: &IndexMode,
) -> Result<(), JobError> {
	let concurrency = state.discovery_concurrency;
	let use_mtime_pruning = index_mode.uses_mtime_pruning();

	if use_mtime_pruning {
		ctx.log(format!(
			"Stale detection mode enabled with mtime pruning (inner mode: {:?})",
			index_mode.inner_mode()
		));
	}

	if concurrency <= 1 {
		return run_discovery_phase_sequential(
			state,
			ctx,
			root_path,
			rule_toggles,
			volume_backend,
			cloud_url_base,
			use_mtime_pruning,
		)
		.await;
	}

	ctx.log(format!(
		"Discovery phase starting from: {} (concurrency: {}, mtime_pruning: {})",
		root_path.display(),
		concurrency,
		use_mtime_pruning
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
		use_mtime_pruning,
	)
	.await
}

/// Parallel discovery using work-stealing with N worker tasks and atomic coordination.
///
/// Workers pull directories from a shared queue, read contents, filter against rules,
/// and directly enqueue subdirectories. A monitor task watches `pending_work` (atomic
/// counter) and signals shutdown when it reaches zero, avoiding explicit work completion
/// messages that would require coordinator awareness.
async fn run_parallel_discovery(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
	use_mtime_pruning: bool,
) -> Result<(), JobError> {
	let concurrency = state.discovery_concurrency;

	let (work_tx, work_rx) = chan::unbounded::<PathBuf>();
	let (result_tx, result_rx) = chan::unbounded::<DiscoveryResult>();

	// INVARIANT: `pending_work` is incremented BEFORE enqueuing work and decremented AFTER
	// completing it. When it reaches zero, all work is done and shutdown can be signaled.
	// This avoids coordinator bottlenecks from explicit "work done" messages.
	let pending_work = Arc::new(AtomicUsize::new(0));
	let skipped_count = Arc::new(AtomicU64::new(0));
	let pruned_count = Arc::new(AtomicU64::new(0));
	let shutdown = Arc::new(AtomicBool::new(false));

	// Shared across all workers to prevent duplicate processing when symlinks create cycles
	// or multiple paths (e.g., /home/user/docs and /mnt/docs) lead to the same directory.
	let seen_paths = Arc::new(parking_lot::RwLock::new(std::collections::HashSet::new()));

	// Get database connection for mtime pruning queries
	let db = if use_mtime_pruning {
		Some(Arc::new(ctx.library_db().clone()))
	} else {
		None
	};

	while let Some(dir) = state.dirs_to_walk.pop_front() {
		pending_work.fetch_add(1, Ordering::Release);
		work_tx
			.send(dir)
			.await
			.map_err(|_| JobError::execution("Work channel closed"))?;
	}

	let mut workers = Vec::new();
	for worker_id in 0..concurrency {
		let work_rx = work_rx.clone();
		let work_tx = work_tx.clone();
		let result_tx = result_tx.clone();
		let pending_work = Arc::clone(&pending_work);
		let skipped_count = Arc::clone(&skipped_count);
		let pruned_count = Arc::clone(&pruned_count);
		let shutdown = Arc::clone(&shutdown);
		let seen_paths = Arc::clone(&seen_paths);
		let root_path = root_path.to_path_buf();
		let volume_backend = volume_backend.cloned();
		let cloud_url_base = cloud_url_base.clone();
		let db = db.clone();

		let worker = tokio::spawn(async move {
			discovery_worker_rayon(
				worker_id,
				work_rx,
				work_tx,
				result_tx,
				pending_work,
				skipped_count,
				pruned_count,
				shutdown,
				seen_paths,
				root_path,
				rule_toggles,
				volume_backend,
				cloud_url_base,
				use_mtime_pruning,
				db,
			)
			.await
		});

		workers.push(worker);
	}

	// Monitor polls `pending_work` and signals shutdown when it hits zero, allowing workers
	// to exit gracefully without needing explicit "I'm done" messages to a coordinator.
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

	drop(work_tx);
	drop(result_tx);

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
				pruned,
			} => {
				state.stats.files += files;
				state.stats.dirs += dirs;
				state.stats.symlinks += symlinks;
				state.stats.bytes += bytes;
				state.stats.pruned += pruned;
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
				unreachable!("Workers should not send QueueDirectories in Rayon-style mode");
			}
		}

		ctx.check_interrupt().await?;
	}

	monitor
		.await
		.map_err(|e| JobError::execution(format!("Monitor task failed: {}", e)))?;

	for worker in workers {
		worker
			.await
			.map_err(|e| JobError::execution(format!("Worker task failed: {}", e)))?;
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

	let skipped = skipped_count.load(Ordering::SeqCst);
	let pruned = pruned_count.load(Ordering::SeqCst);
	state.stats.skipped = skipped;
	state.stats.pruned = pruned;

	if use_mtime_pruning {
		ctx.log(format!(
			"Parallel discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} pruned (mtime), {} batches created",
			state.stats.files,
			state.stats.dirs,
			state.stats.symlinks,
			skipped,
			pruned,
			state.entry_batches.len()
		));
	} else {
		ctx.log(format!(
			"Parallel discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} batches created",
			state.stats.files,
			state.stats.dirs,
			state.stats.symlinks,
			skipped,
			state.entry_batches.len()
		));
	}

	state.phase = crate::ops::indexing::state::Phase::Processing;
	Ok(())
}

/// Messages sent from workers to the coordinator via the result channel.
///
/// Workers send entries, stats updates, progress notifications, and errors through this
/// enum instead of directly mutating shared state. QueueDirectories is unused in the
/// work-stealing implementation (workers directly enqueue subdirectories).
enum DiscoveryResult {
	Entry(DirEntry),
	QueueDirectories(Vec<PathBuf>),
	Stats {
		files: u64,
		dirs: u64,
		symlinks: u64,
		bytes: u64,
		/// Directories pruned via mtime comparison (stale detection)
		pruned: u64,
	},
	Error(IndexError),
	Progress {
		dirs_queued: usize,
	},
}

/// Worker task that pulls directories, reads contents, filters entries, and enqueues subdirectories.
///
/// Workers check the shutdown signal, pull work with a timeout to avoid blocking forever,
/// skip already-seen paths (using the shared RwLock), apply filtering rules, and directly
/// enqueue subdirectories for other workers. The atomic `pending_work` counter tracks
/// in-flight work: incremented before enqueue, decremented after processing completes.
///
/// When `use_mtime_pruning` is enabled, directories with matching modified times (within
/// 1-second tolerance) are pruned, skipping their entire subtree.
#[allow(clippy::too_many_arguments)]
async fn discovery_worker_rayon(
	_worker_id: usize,
	work_rx: chan::Receiver<PathBuf>,
	work_tx: chan::Sender<PathBuf>,
	result_tx: chan::Sender<DiscoveryResult>,
	pending_work: Arc<AtomicUsize>,
	skipped_count: Arc<AtomicU64>,
	pruned_count: Arc<AtomicU64>,
	shutdown: Arc<AtomicBool>,
	seen_paths: Arc<parking_lot::RwLock<std::collections::HashSet<PathBuf>>>,
	root_path: PathBuf,
	rule_toggles: RuleToggles,
	volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
	use_mtime_pruning: bool,
	db: Option<Arc<DatabaseConnection>>,
) {
	loop {
		if shutdown.load(Ordering::Acquire) {
			break;
		}

		let dir_path = match tokio::time::timeout(
			tokio::time::Duration::from_millis(50),
			work_rx.recv(),
		)
		.await
		{
			Ok(Ok(path)) => path,
			Ok(Err(_)) => break,
			Err(_) => continue,
		};

		{
			let mut seen = seen_paths.write();
			if !seen.insert(dir_path.clone()) {
				pending_work.fetch_sub(1, Ordering::Release);
				continue;
			}
		}

		let dir_ruler = build_default_ruler(rule_toggles, &root_path, &dir_path).await;

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
							// Check mtime pruning for directories
							if use_mtime_pruning {
								if let Some(ref db_conn) = db {
									if should_prune_directory(&entry, db_conn.as_ref()).await {
										local_stats.pruned += 1;
										pruned_count.fetch_add(1, Ordering::Relaxed);
										// Don't enqueue this directory - skip entire subtree
										continue;
									}
								}
							}

							local_stats.dirs += 1;
							// Increment BEFORE enqueuing so the monitor never sees pending_work=0 while
							// work is in flight. Decrement only happens after processing completes.
							pending_work.fetch_add(1, Ordering::Release);
							if work_tx.send(entry.path.clone()).await.is_err() {
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

				let _ = result_tx
					.send(DiscoveryResult::Stats {
						files: local_stats.files,
						dirs: local_stats.dirs,
						symlinks: local_stats.symlinks,
						bytes: local_stats.bytes,
						pruned: local_stats.pruned,
					})
					.await;

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

		pending_work.fetch_sub(1, Ordering::Release);
	}
}

#[derive(Default)]
struct LocalStats {
	files: u64,
	dirs: u64,
	symlinks: u64,
	bytes: u64,
	pruned: u64,
}

// =============================================================================
// Mtime Pruning Functions (Stale Detection)
// =============================================================================

/// Check if a directory should be pruned based on modified time comparison.
///
/// Compares the filesystem modified time with the database record. If they match
/// (within 1-second tolerance), the directory is considered unchanged and can be
/// pruned, skipping its entire subtree.
async fn should_prune_directory(entry: &DirEntry, db: &DatabaseConnection) -> bool {
	// Get filesystem modified time
	let Some(fs_mtime) = entry.modified else {
		return false; // No mtime available, can't prune (safe default)
	};

	// Query database for existing entry's modified time
	let db_mtime = match query_entry_mtime(db, &entry.path).await {
		Ok(Some(mtime)) => mtime,
		Ok(None) => return false, // Not in DB, definitely changed
		Err(_) => return false,   // Query failed, don't prune (safe default)
	};

	// Compare modified times with tolerance
	times_match(fs_mtime, db_mtime)
}

/// Query database for entry's modified time using directory_paths cache.
///
/// Uses the directory_paths table for O(1) path-based lookup, then joins to
/// the entries table to get the modified time.
async fn query_entry_mtime(
	db: &DatabaseConnection,
	path: &Path,
) -> Result<Option<DateTime<Utc>>, sea_orm::DbErr> {
	use crate::infra::db::entities::{directory_paths, entry};
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	let path_str = path.to_string_lossy().to_string();

	// Query directory_paths to find the entry_id, then get the entry's modified_at
	let result = directory_paths::Entity::find()
		.filter(directory_paths::Column::Path.eq(path_str))
		.one(db)
		.await?;

	let Some(dir_path_record) = result else {
		return Ok(None);
	};

	// Get the entry's modified time
	let entry_result = entry::Entity::find_by_id(dir_path_record.entry_id)
		.one(db)
		.await?;

	Ok(entry_result.map(|e| e.modified_at.into()))
}

/// Compare filesystem time with database time (1-second tolerance).
///
/// Filesystem timestamps can have different precision depending on the platform
/// and filesystem. Using a 1-second tolerance accounts for these differences.
fn times_match(fs_time: SystemTime, db_time: DateTime<Utc>) -> bool {
	let fs_datetime: DateTime<Utc> = fs_time.into();
	let diff = (fs_datetime - db_time).num_seconds().abs();
	diff <= 1
}

/// Single-threaded directory traversal fallback for concurrency = 1.
///
/// Uses a simple queue-based approach without task spawning overhead. Processes
/// directories one at a time, applies filters, and accumulates entries into batches.
/// Useful for debugging or when parallel overhead exceeds benefits (small directory trees).
async fn run_discovery_phase_sequential(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	root_path: &Path,
	rule_toggles: RuleToggles,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<String>,
	use_mtime_pruning: bool,
) -> Result<(), JobError> {
	ctx.log(format!(
		"Discovery phase starting from: {} (sequential mode, mtime_pruning: {})",
		root_path.display(),
		use_mtime_pruning
	));

	let mut skipped_count = 0u64;
	let mut pruned_count = 0u64;

	// Get database connection for mtime pruning queries
	let db = if use_mtime_pruning {
		Some(ctx.library_db())
	} else {
		None
	};

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
							// Check mtime pruning for directories
							if use_mtime_pruning {
								if let Some(db_conn) = &db {
									if should_prune_directory(&entry, db_conn).await {
										state.stats.pruned += 1;
										pruned_count += 1;
										// Don't add to dirs_to_walk - skip entire subtree
										continue;
									}
								}
							}

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

	if use_mtime_pruning {
		ctx.log(format!(
			"Discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} pruned (mtime), {} batches created",
			state.stats.files,
			state.stats.dirs,
			state.stats.symlinks,
			skipped_count,
			pruned_count,
			state.entry_batches.len()
		));
	} else {
		ctx.log(format!(
			"Discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} batches created",
			state.stats.files,
			state.stats.dirs,
			state.stats.symlinks,
			skipped_count,
			state.entry_batches.len()
		));
	}

	state.phase = crate::ops::indexing::state::Phase::Processing;
	Ok(())
}

/// Reads a directory through a volume backend, falling back to LocalBackend if none provided.
///
/// Volume backends abstract local filesystems and cloud storage (S3, Dropbox) behind a
/// unified interface. When indexing managed locations, the backend is provided upfront from
/// volume registration. For ephemeral browsing or untracked paths, this creates a temporary
/// LocalBackend on demand.
async fn read_directory(
	path: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	cloud_url_base: Option<&str>,
) -> Result<Vec<DirEntry>, std::io::Error> {
	let backend: Arc<dyn crate::volume::VolumeBackend> = match volume_backend {
		Some(backend) => Arc::clone(backend),
		None => Arc::new(crate::volume::LocalBackend::new(
			path.parent().unwrap_or(path),
		)),
	};

	read_directory_with_backend(backend.as_ref(), path, cloud_url_base).await
}

/// Reads directory contents via a volume backend and converts paths for cloud vs local.
///
/// For cloud volumes, prepends the cloud URL base (e.g., "s3://bucket/") to build proper
/// hierarchical paths. For local volumes, uses standard PathBuf joins. This ensures cloud
/// entries have full URIs like "s3://bucket/folder/file.txt" instead of relative paths.
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

	let entries: Vec<DirEntry> = raw_entries
		.into_iter()
		.map(|raw| {
			let full_path = if let Some(base) = cloud_url_base {
				let relative = path.to_string_lossy();
				let joined = if relative.is_empty() {
					raw.name.clone()
				} else {
					format!("{}/{}", relative.trim_end_matches('/'), raw.name)
				};
				PathBuf::from(format!("{}{}", base, joined))
			} else {
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

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::TimeZone;
	use std::time::Duration;

	#[test]
	fn test_times_match_exact() {
		// Exactly the same time should match
		let db_time = Utc.with_ymd_and_hms(2024, 12, 24, 12, 0, 0).unwrap();
		let fs_time: SystemTime = db_time.into();
		assert!(times_match(fs_time, db_time));
	}

	#[test]
	fn test_times_match_within_tolerance() {
		// Within 1 second should match
		let db_time = Utc.with_ymd_and_hms(2024, 12, 24, 12, 0, 0).unwrap();
		let fs_time: SystemTime =
			SystemTime::from(db_time) + Duration::from_millis(500);
		assert!(times_match(fs_time, db_time));

		// Exactly at 1 second boundary should match
		let fs_time_at_boundary: SystemTime =
			SystemTime::from(db_time) + Duration::from_secs(1);
		assert!(times_match(fs_time_at_boundary, db_time));
	}

	#[test]
	fn test_times_dont_match_beyond_tolerance() {
		// Beyond 1 second should not match
		let db_time = Utc.with_ymd_and_hms(2024, 12, 24, 12, 0, 0).unwrap();
		let fs_time: SystemTime =
			SystemTime::from(db_time) + Duration::from_secs(2);
		assert!(!times_match(fs_time, db_time));

		// Negative difference beyond tolerance
		let fs_time_earlier: SystemTime =
			SystemTime::from(db_time) - Duration::from_secs(2);
		assert!(!times_match(fs_time_earlier, db_time));
	}

	#[test]
	fn test_index_mode_mtime_pruning() {
		use crate::domain::location::IndexMode;

		// Stale mode should enable pruning
		let stale_mode = IndexMode::Stale(Box::new(IndexMode::Deep));
		assert!(stale_mode.uses_mtime_pruning());

		// Regular modes should not enable pruning
		assert!(!IndexMode::None.uses_mtime_pruning());
		assert!(!IndexMode::Shallow.uses_mtime_pruning());
		assert!(!IndexMode::Content.uses_mtime_pruning());
		assert!(!IndexMode::Deep.uses_mtime_pruning());
	}

	#[test]
	fn test_index_mode_inner_mode() {
		use crate::domain::location::IndexMode;

		// Stale mode should return inner mode
		let stale_deep = IndexMode::Stale(Box::new(IndexMode::Deep));
		assert!(matches!(stale_deep.inner_mode(), IndexMode::Deep));

		let stale_shallow = IndexMode::Stale(Box::new(IndexMode::Shallow));
		assert!(matches!(stale_shallow.inner_mode(), IndexMode::Shallow));

		// Regular modes should return themselves
		assert!(matches!(IndexMode::Deep.inner_mode(), IndexMode::Deep));
		assert!(matches!(IndexMode::Shallow.inner_mode(), IndexMode::Shallow));
	}

	#[test]
	fn test_index_mode_unwrap() {
		use crate::domain::location::IndexMode;

		// Unwrap should return inner mode for Stale
		let stale_content = IndexMode::Stale(Box::new(IndexMode::Content));
		let unwrapped = stale_content.unwrap_mode();
		assert!(matches!(unwrapped, IndexMode::Content));

		// Unwrap should return same mode for non-Stale
		let deep = IndexMode::Deep;
		assert!(matches!(deep.unwrap_mode(), IndexMode::Deep));
	}
}
