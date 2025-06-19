//! Example of how to use the GenericProgress system in the indexer
//! 
//! This demonstrates how indexer phases can convert their progress
//! to GenericProgress for better job monitoring.

use crate::{
    infrastructure::jobs::prelude::{JobContext, Progress, ToGenericProgress},
    operations::indexing::state::{IndexerProgress, IndexPhase, IndexerStats},
};
use std::time::Duration;

/// Example of sending generic progress in the discovery phase
pub fn send_discovery_progress(ctx: &JobContext<'_>, dirs_queued: usize, current_path: &str) {
    let indexer_progress = IndexerProgress {
        phase: IndexPhase::Discovery { dirs_queued },
        current_path: current_path.to_string(),
        total_found: IndexerStats::default(),
        processing_rate: 0.0,
        estimated_remaining: None,
    };
    
    // Convert to generic progress and send
    let generic_progress = indexer_progress.to_generic_progress();
    ctx.progress(Progress::generic(generic_progress));
}

/// Example of sending generic progress in the processing phase  
pub fn send_processing_progress(
    ctx: &JobContext<'_>, 
    batch: usize, 
    total_batches: usize,
    current_path: &str,
    stats: IndexerStats,
    rate: f32,
    eta: Option<Duration>
) {
    let indexer_progress = IndexerProgress {
        phase: IndexPhase::Processing { batch, total_batches },
        current_path: current_path.to_string(),
        total_found: stats,
        processing_rate: rate,
        estimated_remaining: eta,
    };
    
    // Convert to generic progress and send
    let generic_progress = indexer_progress.to_generic_progress();
    ctx.progress(Progress::generic(generic_progress));
}

/// Example of sending generic progress in the content identification phase
pub fn send_content_progress(
    ctx: &JobContext<'_>,
    current: usize,
    total: usize,
    current_file: &str,
    stats: IndexerStats,
    rate: f32,
    eta: Option<Duration>
) {
    let indexer_progress = IndexerProgress {
        phase: IndexPhase::ContentIdentification { current, total },
        current_path: current_file.to_string(), 
        total_found: stats,
        processing_rate: rate,
        estimated_remaining: eta,
    };
    
    // Convert to generic progress and send
    let generic_progress = indexer_progress.to_generic_progress();
    ctx.progress(Progress::generic(generic_progress));
}

/// Example of sending finalization progress
pub fn send_finalizing_progress(
    ctx: &JobContext<'_>,
    message: &str,
    stats: IndexerStats
) {
    let indexer_progress = IndexerProgress {
        phase: IndexPhase::Finalizing,
        current_path: message.to_string(),
        total_found: stats,
        processing_rate: 0.0,
        estimated_remaining: Some(Duration::from_secs(5)),
    };
    
    // Convert to generic progress and send
    let generic_progress = indexer_progress.to_generic_progress();
    ctx.progress(Progress::generic(generic_progress));
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_progress_conversion_flow() {
        // Test that we can create indexer progress and convert it
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Processing { batch: 5, total_batches: 20 },
            current_path: "/test/path".to_string(),
            total_found: IndexerStats {
                files: 100,
                dirs: 10,
                bytes: 1024 * 1024,
                symlinks: 2,
                skipped: 3,
                errors: 1,
            },
            processing_rate: 15.5,
            estimated_remaining: Some(Duration::from_secs(60)),
        };
        
        let generic = indexer_progress.to_generic_progress();
        
        // Verify the conversion worked correctly
        assert_eq!(generic.phase, "Processing");
        assert_eq!(generic.percentage, 0.25); // 5/20
        assert_eq!(generic.completion.completed, 5);
        assert_eq!(generic.completion.total, 20);
        assert_eq!(generic.performance.rate, 15.5);
        assert_eq!(generic.performance.error_count, 1);
        
        // Verify it formats nicely
        let formatted = generic.format_progress();
        assert!(formatted.contains("25.0%"));
        
        // Verify Progress enum can extract percentage
        let progress = Progress::generic(generic);
        assert_eq!(progress.as_percentage(), Some(0.25));
    }
}