//! Content identification phase - generates CAS IDs and links content

use crate::{
    infrastructure::jobs::prelude::{JobContext, JobError, Progress},
    infrastructure::jobs::generic_progress::ToGenericProgress,
    operations::indexing::{
        state::{IndexerState, IndexPhase, IndexError, IndexerProgress},
        entry::EntryProcessor,
    },
    domain::content_identity::ContentHashGenerator,
};

/// Run the content identification phase
pub async fn run_content_phase(
    state: &mut IndexerState,
    ctx: &JobContext<'_>,
    library_id: uuid::Uuid,
) -> Result<(), JobError> {
    let total = state.entries_for_content.len();
    ctx.log(format!("Content identification phase starting with {} files", total));
    
    if total == 0 {
        ctx.log("No files to process for content identification");
        state.phase = crate::operations::indexing::state::Phase::Complete;
        return Ok(());
    }
    
    let mut processed = 0;
    let mut success_count = 0;
    let mut error_count = 0;
    
    // Process in chunks for better performance and memory usage
    const CHUNK_SIZE: usize = 100;
    
    while !state.entries_for_content.is_empty() {
        ctx.check_interrupt().await?;
        
        let chunk_size = CHUNK_SIZE.min(state.entries_for_content.len());
        let chunk: Vec<_> = state.entries_for_content.drain(..chunk_size).collect();
        let chunk_len = chunk.len();
        
        // Report progress BEFORE processing (using current processed count)
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::ContentIdentification { 
                current: processed, 
                total 
            },
            current_path: format!("Generating content identities ({}/{})", processed, total),
            total_found: state.stats,
            processing_rate: state.calculate_rate(),
            estimated_remaining: state.estimate_remaining(),
            scope: None,
            persistence: None,
            is_ephemeral: false,
        };
        ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));
        
        // Process chunk in parallel for better performance
        let content_hash_futures: Vec<_> = chunk.iter()
            .map(|(entry_id, path)| async move {
                let hash_result = ContentHashGenerator::generate_content_hash(path).await;
                (*entry_id, path.clone(), hash_result)
            })
            .collect();
        
        // Wait for all content hash generations to complete
        let hash_results = futures::future::join_all(content_hash_futures).await;
        
        // Process results
        for (entry_id, path, hash_result) in hash_results {
            match hash_result {
                Ok(content_hash) => {
                    match EntryProcessor::link_to_content_identity(ctx, entry_id, &path, content_hash.clone(), library_id).await {
                        Ok(()) => {
                            ctx.log(format!("✅ Created content identity for {}: {}", path.display(), content_hash));
                            success_count += 1;
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to create content identity for {}: {}", path.display(), e);
                            ctx.add_non_critical_error(error_msg);
                            state.add_error(IndexError::ContentId { 
                                path: path.to_string_lossy().to_string(), 
                                error: e.to_string() 
                            });
                            error_count += 1;
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to generate content hash for {}: {}", path.display(), e);
                    ctx.add_non_critical_error(error_msg);
                    state.add_error(IndexError::ContentId { 
                        path: path.to_string_lossy().to_string(), 
                        error: e.to_string() 
                    });
                    error_count += 1;
                }
            }
        }
        
        // Update processed count AFTER processing chunk
        processed += chunk_len;
        
        // Update rate tracking
        state.items_since_last_update += chunk_len as u64;
        
        // Periodic checkpoint
        if processed % 1000 == 0 || processed == total {
            ctx.checkpoint_with_state(state).await?;
        }
    }
    
    ctx.log(format!(
        "Content identification complete: {} successful, {} errors out of {} total",
        success_count, error_count, total
    ));
    
    state.phase = crate::operations::indexing::state::Phase::Complete;
    Ok(())
}