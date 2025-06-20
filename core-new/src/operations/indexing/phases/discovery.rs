//! Discovery phase - walks directories and collects entries

use crate::{
    infrastructure::jobs::prelude::{JobContext, JobError, Progress},
    infrastructure::jobs::generic_progress::ToGenericProgress,
    operations::indexing::{
        state::{IndexerState, DirEntry, EntryKind, IndexPhase, IndexError, IndexerProgress},
        filters::should_skip_path,
        entry::EntryProcessor,
    },
};
use std::path::Path;

/// Run the discovery phase of indexing
pub async fn run_discovery_phase(
    state: &mut IndexerState,
    ctx: &JobContext<'_>,
    root_path: &Path,
) -> Result<(), JobError> {
    ctx.log(format!("Discovery phase starting from: {}", root_path.display()));
    ctx.log(format!("Initial directories to walk: {}", state.dirs_to_walk.len()));
    
    let mut skipped_count = 0u64;
    
    while let Some(dir_path) = state.dirs_to_walk.pop_front() {
        ctx.check_interrupt().await?;
        
        // Skip if already seen (handles symlink loops)
        if !state.seen_paths.insert(dir_path.clone()) {
            continue;
        }
        
        // Check if we should skip this directory
        if should_skip_path(&dir_path) {
            state.stats.skipped += 1;
            skipped_count += 1;
            ctx.log(format!("Skipping directory: {}", dir_path.display()));
            continue;
        }
        
        // Update progress
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Discovery { 
                dirs_queued: state.dirs_to_walk.len() 
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
        
        // Read directory entries
        match read_directory(&dir_path).await {
            Ok(entries) => {
                let entry_count = entries.len();
                let mut added_count = 0;
                
                for entry in entries {
                    // Skip filtered entries
                    if should_skip_path(&entry.path) {
                        state.stats.skipped += 1;
                        skipped_count += 1;
                        continue;
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
                    error: e.to_string() 
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
        ctx.log(format!("Creating final batch with {} entries", final_batch_size));
        let batch = state.create_batch();
        state.entry_batches.push(batch);
    }
    
    ctx.log(format!(
        "Discovery complete: {} files, {} dirs, {} symlinks, {} skipped, {} batches created", 
        state.stats.files, state.stats.dirs, state.stats.symlinks,
        skipped_count, state.entry_batches.len()
    ));
    
    state.phase = crate::operations::indexing::state::Phase::Processing;
    Ok(())
}

/// Read a directory and extract metadata
async fn read_directory(path: &Path) -> Result<Vec<DirEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(path).await?;
    
    while let Some(entry) = dir.next_entry().await? {
        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue, // Skip entries we can't read
        };
        
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
    
    Ok(entries)
}