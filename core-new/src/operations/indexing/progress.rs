//! IndexerProgress to GenericProgress conversion

use crate::{
    infrastructure::jobs::generic_progress::{GenericProgress, ToGenericProgress},
    shared::types::SdPath,
};
use super::state::{IndexerProgress, IndexPhase};
use std::path::PathBuf;

impl ToGenericProgress for IndexerProgress {
    fn to_generic_progress(&self) -> GenericProgress {
        let (percentage, completion_info, phase_name) = match &self.phase {
            IndexPhase::Discovery { dirs_queued } => {
                // Discovery phase - indeterminate but show queue size
                let message = format!("Discovering files and directories ({} queued)", dirs_queued);
                (0.0, (0, 0), "Discovery".to_string())
            }
            IndexPhase::Processing { batch, total_batches } => {
                // Processing phase - show batch progress
                let percentage = if *total_batches > 0 {
                    *batch as f32 / *total_batches as f32
                } else {
                    0.0
                };
                let message = format!("Processing entries (batch {}/{})", batch, total_batches);
                (percentage, (*batch as u64, *total_batches as u64), "Processing".to_string())
            }
            IndexPhase::ContentIdentification { current, total } => {
                // Content ID phase - show item progress
                let percentage = if *total > 0 {
                    *current as f32 / *total as f32
                } else {
                    0.0
                };
                let message = format!("Generating content identities ({}/{})", current, total);
                (percentage, (*current as u64, *total as u64), "Content Identification".to_string())
            }
            IndexPhase::Finalizing => {
                // Final phase - nearly complete
                let message = "Finalizing index data...".to_string();
                (0.95, (0, 0), "Finalizing".to_string())
            }
        };

        // Convert current_path string to SdPath if possible
        let current_path = if !self.current_path.is_empty() {
            // For now, create a simple SdPath - this would need proper device UUID in real implementation
            Some(SdPath::new(
                uuid::Uuid::nil(), // TODO: Get actual device UUID
                PathBuf::from(&self.current_path)
            ))
        } else {
            None
        };

        // Create the generic progress
        let mut progress = GenericProgress::new(percentage, &phase_name, &self.current_path)
            .with_completion(completion_info.0, completion_info.1)
            .with_bytes(self.total_found.bytes, self.total_found.bytes) // Total bytes found so far
            .with_performance(
                self.processing_rate,
                self.estimated_remaining,
                None, // Could calculate elapsed time from start
            )
            .with_errors(self.total_found.errors, 0) // No separate warning count in IndexerStats
            .with_metadata(self); // Include original indexer progress as metadata

        // Set current path if available
        if let Some(path) = current_path {
            progress = progress.with_current_path(path);
        }

        progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::indexing::state::{IndexerStats, IndexPhase};
    use std::time::Duration;

    #[test]
    fn test_discovery_phase_conversion() {
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Discovery { dirs_queued: 42 },
            current_path: "/home/user/documents".to_string(),
            total_found: IndexerStats::default(),
            processing_rate: 0.0,
            estimated_remaining: None,
        };

        let generic = indexer_progress.to_generic_progress();
        assert_eq!(generic.phase, "Discovery");
        assert_eq!(generic.percentage, 0.0);
        assert!(generic.message.contains("42 queued"));
    }

    #[test]
    fn test_processing_phase_conversion() {
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Processing { batch: 3, total_batches: 10 },
            current_path: "/home/user/photos".to_string(),
            total_found: IndexerStats {
                files: 150,
                dirs: 20,
                bytes: 1024 * 1024 * 500, // 500MB
                symlinks: 5,
                skipped: 2,
                errors: 1,
            },
            processing_rate: 25.5,
            estimated_remaining: Some(Duration::from_secs(120)),
        };

        let generic = indexer_progress.to_generic_progress();
        assert_eq!(generic.phase, "Processing");
        assert_eq!(generic.percentage, 0.3); // 3/10
        assert_eq!(generic.completion.completed, 3);
        assert_eq!(generic.completion.total, 10);
        assert_eq!(generic.performance.rate, 25.5);
        assert_eq!(generic.performance.estimated_remaining, Some(Duration::from_secs(120)));
        assert_eq!(generic.performance.error_count, 1);
    }

    #[test]
    fn test_content_identification_conversion() {
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::ContentIdentification { current: 75, total: 100 },
            current_path: "/home/user/videos/movie.mp4".to_string(),
            total_found: IndexerStats::default(),
            processing_rate: 12.0,
            estimated_remaining: Some(Duration::from_secs(30)),
        };

        let generic = indexer_progress.to_generic_progress();
        assert_eq!(generic.phase, "Content Identification");
        assert_eq!(generic.percentage, 0.75); // 75/100
        assert_eq!(generic.completion.completed, 75);
        assert_eq!(generic.completion.total, 100);
    }

    #[test]
    fn test_finalizing_phase_conversion() {
        let indexer_progress = IndexerProgress {
            phase: IndexPhase::Finalizing,
            current_path: "Aggregating directory data...".to_string(),
            total_found: IndexerStats::default(),
            processing_rate: 0.0,
            estimated_remaining: Some(Duration::from_secs(5)),
        };

        let generic = indexer_progress.to_generic_progress();
        assert_eq!(generic.phase, "Finalizing");
        assert_eq!(generic.percentage, 0.95); // Nearly complete
    }
}