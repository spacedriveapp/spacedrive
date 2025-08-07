//! Processing phase - creates/updates database entries

use crate::{
    infrastructure::{
        jobs::prelude::{JobContext, JobError, Progress},
        jobs::generic_progress::ToGenericProgress,
        database::entities,
    },
    operations::indexing::{
        state::{IndexerState, IndexPhase, IndexError, EntryKind, IndexerProgress},
        entry::EntryProcessor,
        IndexMode,
        change_detection::{ChangeDetector, Change},
    },
};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use uuid::Uuid;
use std::path::Path;

/// Run the processing phase of indexing
pub async fn run_processing_phase(
    location_id: Uuid,
    state: &mut IndexerState,
    ctx: &JobContext<'_>,
    mode: IndexMode,
    location_root_path: &Path,
) -> Result<(), JobError> {
    let total_batches = state.entry_batches.len();
    ctx.log(format!("Processing phase starting with {} batches", total_batches));
    
    // Get the actual location record from database
    let location_record = entities::location::Entity::find()
        .filter(entities::location::Column::Uuid.eq(location_id))
        .one(ctx.library_db())
        .await
        .map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
        .ok_or_else(|| JobError::execution("Location not found in database".to_string()))?;
    
    let device_id = location_record.device_id;
    let location_id_i32 = location_record.id;
    ctx.log(format!("Found location record: device_id={}, location_id={}", device_id, location_id_i32));
    
    // Load existing entries for change detection scoped to the indexing path
    // Note: location_root_path is the actual path being indexed (could be a subpath of the location)
    let mut change_detector = ChangeDetector::new();
    if !state.existing_entries.is_empty() || mode != IndexMode::Shallow {
        ctx.log("Loading existing entries for change detection...");
        change_detector.load_existing_entries(ctx, location_id_i32, location_root_path).await?;
        ctx.log(format!("Loaded {} existing entries", change_detector.entry_count()));
    }
    
    let mut total_processed = 0;
    let mut batch_number = 0;
    
    while let Some(mut batch) = state.entry_batches.pop() {
        ctx.check_interrupt().await?;
        
        batch_number += 1;
        let batch_size = batch.len();
        
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Processing { 
                batch: batch_number, 
                total_batches 
            },
            current_path: format!("Batch {}/{}", batch_number, total_batches),
            total_found: state.stats,
            processing_rate: state.calculate_rate(),
            estimated_remaining: state.estimate_remaining(),
            scope: None,
            persistence: None,
            is_ephemeral: false,
        };
        ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));
        
        // Sort batch by path depth first, then by type to ensure parents are processed before children
        batch.sort_by(|a, b| {
            let a_depth = a.path.components().count();
            let b_depth = b.path.components().count();
            
            // First sort by depth (parents before children)
            match a_depth.cmp(&b_depth) {
                std::cmp::Ordering::Equal => {
                    // Then sort by type (directories before files at same depth)
                    let a_priority = match a.kind {
                        EntryKind::Directory => 0,
                        EntryKind::Symlink => 1,
                        EntryKind::File => 2,
                    };
                    let b_priority = match b.kind {
                        EntryKind::Directory => 0,
                        EntryKind::Symlink => 1,
                        EntryKind::File => 2,
                    };
                    a_priority.cmp(&b_priority)
                }
                other => other,
            }
        });
        
        // Process batch - check for changes and create/update entries
        for entry in batch {
            // Get metadata for change detection
            let metadata = match std::fs::metadata(&entry.path) {
                Ok(m) => m,
                Err(e) => {
                    ctx.add_non_critical_error(format!("Failed to get metadata for {}: {}", entry.path.display(), e));
                    continue;
                }
            };
            
            // Check for changes
            let change = change_detector.check_path(&entry.path, &metadata, entry.inode);
            
            match change {
                Some(Change::New(_)) => {
                    // Create new entry
                    match EntryProcessor::create_entry(state, ctx, &entry, location_id_i32, device_id, location_root_path).await {
                        Ok(entry_id) => {
                            ctx.log(format!("✅ Created entry {}: {}", entry_id, entry.path.display()));
                            total_processed += 1;
                            
                            // Track for content identification if needed
                            if mode >= IndexMode::Content && entry.kind == EntryKind::File {
                                state.entries_for_content.push((entry_id, entry.path));
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to create entry for {}: {}", entry.path.display(), e);
                            ctx.add_non_critical_error(error_msg);
                            state.add_error(IndexError::CreateEntry { 
                                path: entry.path.to_string_lossy().to_string(), 
                                error: e.to_string() 
                            });
                        }
                    }
                }
                
                Some(Change::Modified { entry_id, .. }) => {
                    // Update existing entry
                    match EntryProcessor::update_entry(ctx, entry_id, &entry).await {
                        Ok(()) => {
                            ctx.log(format!("📝 Updated entry {}: {}", entry_id, entry.path.display()));
                            total_processed += 1;
                            
                            // Re-process content if needed
                            if mode >= IndexMode::Content && entry.kind == EntryKind::File {
                                state.entries_for_content.push((entry_id, entry.path));
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to update entry {}: {}", entry_id, e);
                            ctx.add_non_critical_error(error_msg);
                            state.add_error(IndexError::CreateEntry { 
                                path: entry.path.to_string_lossy().to_string(), 
                                error: e.to_string() 
                            });
                        }
                    }
                }
                
                Some(Change::Moved { old_path, new_path, entry_id, .. }) => {
                    // Handle move - update path in database
                    ctx.log(format!("🔄 Detected move: {} -> {}", old_path.display(), new_path.display()));
                    match EntryProcessor::move_entry(state, ctx, entry_id, &old_path, &new_path, location_root_path).await {
                        Ok(()) => {
                            ctx.log(format!("✅ Moved entry {}: {} -> {}", entry_id, old_path.display(), new_path.display()));
                            total_processed += 1;
                            
                            // Re-process content if needed for moved files
                            if mode >= IndexMode::Content && entry.kind == EntryKind::File {
                                state.entries_for_content.push((entry_id, new_path));
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to move entry {}: {}", entry_id, e);
                            ctx.add_non_critical_error(error_msg);
                            state.add_error(IndexError::CreateEntry { 
                                path: new_path.to_string_lossy().to_string(), 
                                error: e.to_string() 
                            });
                        }
                    }
                }
                
                Some(Change::Deleted { .. }) => {
                    // This shouldn't happen during processing of found entries
                    // Deleted entries are handled after processing
                }
                
                None => {
                    // No change - skip
                    ctx.log(format!("⏭️  No change for: {}", entry.path.display()));
                }
            }
        }
        
        ctx.log(format!("Processed batch {}/{}: {} entries", batch_number, total_batches, batch_size));
        ctx.checkpoint_with_state(state).await?;
    }
    
    // Handle deleted entries
    if change_detector.entry_count() > 0 {
        ctx.log("Checking for deleted entries...");
        let seen_paths: std::collections::HashSet<_> = state.seen_paths.iter().cloned().collect();
        let deleted = change_detector.find_deleted(&seen_paths);
        
        if !deleted.is_empty() {
            ctx.log(format!("Found {} deleted entries", deleted.len()));
            for change in deleted {
                if let Change::Deleted { path, entry_id } = change {
                    ctx.log(format!("❌ Marking as deleted: {} (id: {})", path.display(), entry_id));
                    // TODO: Handle deletion (mark as deleted or remove from DB)
                }
            }
        }
    }
    
    ctx.log(format!("Processing phase complete: {} entries processed", total_processed));
    state.phase = crate::operations::indexing::state::Phase::Aggregation;
    Ok(())
}