//! # IndexerProgress to GenericProgress Conversion
//!
//! Maps indexer-specific progress (phases, stats) to the generic job progress format for UI display.
//! Each phase is assigned a percentage range to show continuous progress across all four stages.
//! The converter handles path filtering to distinguish between real filesystem paths and status messages.

use super::state::{IndexPhase, IndexerProgress};
use crate::{
	domain::addressing::SdPath,
	infra::job::generic_progress::{GenericProgress, ToGenericProgress},
};
use std::path::PathBuf;

impl ToGenericProgress for IndexerProgress {
	fn to_generic_progress(&self) -> GenericProgress {
		// Extract phase info
		let (phase_based_pct, completion_info, phase_name, phase_message) = match &self.phase {
			IndexPhase::Discovery { dirs_queued } => {
				let message = format!("Discovering files and directories ({} queued)", dirs_queued);
				let percentage = if *dirs_queued > 0 { 0.0 } else { 0.05 };
				(percentage, (0, 0), "Discovery".to_string(), message)
			}
			IndexPhase::Processing {
				batch,
				total_batches,
			} => {
				let batch_progress = if *total_batches > 0 {
					*batch as f32 / *total_batches as f32
				} else {
					0.0
				};
				let percentage = 0.2 + (batch_progress * 0.4);
				let message = format!("Processing entries (batch {}/{})", batch, total_batches);
				(
					percentage,
					(*batch as u64, *total_batches as u64),
					"Processing".to_string(),
					message,
				)
			}
			IndexPhase::ContentIdentification { current, total } => {
				let content_progress = if *total > 0 {
					(*current as f32 / *total as f32).min(1.0)
				} else {
					0.0
				};
				let percentage = 0.7 + (content_progress * 0.28);
				let message = format!("Generating content identities ({}/{})", current, total);
				(
					percentage,
					(*current as u64, *total as u64),
					"Content Identification".to_string(),
					message,
				)
			}
			IndexPhase::Finalizing { processed, total } => {
				let finalizing_progress = if *total > 0 {
					(*processed as f32 / *total as f32).min(0.99)
				} else {
					0.99
				};
				let percentage = 0.99 + (finalizing_progress * 0.01);
				let message = format!("Finalizing ({}/{})", processed, total);
				(
					percentage,
					(*processed as u64, *total as u64),
					"Finalizing".to_string(),
					message,
				)
			}
		};

		// Use volume-based percentage if available, otherwise use phase-based
		let percentage = if let Some(volume_capacity) = self.volume_total_capacity {
			if volume_capacity > 0 {
				// Calculate actual progress as bytes_indexed / total_volume_capacity
				(self.total_found.bytes as f64 / volume_capacity as f64).min(1.0) as f32
			} else {
				phase_based_pct
			}
		} else {
			phase_based_pct
		};

		// Filter out status messages from current_path - only convert real filesystem paths to SdPath.
		let current_path = if !self.current_path.is_empty()
			&& !self.current_path.starts_with("Aggregating directory")
			&& !self.current_path.starts_with("Finalizing")
		{
			let path_buf = PathBuf::from(&self.current_path);
			if path_buf.is_absolute()
				|| self.current_path.contains('/')
				|| self.current_path.contains('\\')
			{
				SdPath::from_uri(&self.current_path)
					.ok()
					.or_else(|| Some(SdPath::local(path_buf)))
			} else {
				None
			}
		} else {
			None
		};

		let final_completion = completion_info;

		let mut progress = GenericProgress::new(percentage, &phase_name, &phase_message)
			.with_bytes(
				self.total_found.bytes,
				self.volume_total_capacity.unwrap_or(self.total_found.bytes),
			)
			.with_performance(self.processing_rate, self.estimated_remaining, None)
			.with_errors(self.total_found.errors, 0)
			.with_metadata(self);

		// Finalizing phase uses manual completion to preserve custom percentage ranges.
		match &self.phase {
			IndexPhase::Finalizing { .. } => {
				progress.completion.completed = final_completion.0;
				progress.completion.total = final_completion.1;
			}
			_ => {
				progress = progress.with_completion(final_completion.0, final_completion.1);
			}
		}

		if let Some(path) = current_path {
			progress = progress.with_current_path(path);
		}

		progress
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::state::{IndexPhase, IndexerStats};
	use std::time::Duration;

	#[test]
	fn test_discovery_phase_conversion() {
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::Discovery { dirs_queued: 42 },
			current_path: "/home/user/documents".to_string(),
			total_found: IndexerStats::default(),
			processing_rate: 0.0,
			estimated_remaining: None,
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None,
			volume_total_capacity: None,
		};

		let generic = indexer_progress.to_generic_progress();
		assert_eq!(generic.phase, "Discovery");
		assert_eq!(generic.percentage, 0.0);
		assert!(generic.message.contains("42 queued"));
	}

	#[test]
	fn test_processing_phase_conversion() {
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::Processing {
				batch: 3,
				total_batches: 10,
			},
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
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None,
			volume_total_capacity: None,
		};

		let generic = indexer_progress.to_generic_progress();
		assert_eq!(generic.phase, "Processing");
		assert_eq!(generic.percentage, 0.32); // 0.2 + (0.3 * 0.4) = 0.32
		assert_eq!(generic.completion.completed, 3);
		assert_eq!(generic.completion.total, 10);
		assert_eq!(generic.performance.rate, 25.5);
		assert_eq!(
			generic.performance.estimated_remaining,
			Some(Duration::from_secs(120))
		);
		assert_eq!(generic.performance.error_count, 1);
	}

	#[test]
	fn test_content_identification_conversion() {
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::ContentIdentification {
				current: 75,
				total: 100,
			},
			current_path: "/home/user/videos/movie.mp4".to_string(),
			total_found: IndexerStats::default(),
			processing_rate: 12.0,
			estimated_remaining: Some(Duration::from_secs(30)),
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None,
			volume_total_capacity: None,
		};

		let generic = indexer_progress.to_generic_progress();
		assert_eq!(generic.phase, "Content Identification");
		assert_eq!(generic.percentage, 0.91); // 0.7 + (0.75 * 0.28) = 0.91
		assert_eq!(generic.completion.completed, 75);
		assert_eq!(generic.completion.total, 100);
	}

	#[test]
	fn test_finalizing_phase_conversion() {
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::Finalizing {
				processed: 95,
				total: 100,
			},
			current_path: "Aggregating directory data...".to_string(),
			total_found: IndexerStats::default(),
			processing_rate: 0.0,
			estimated_remaining: Some(Duration::from_secs(5)),
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None,
			volume_total_capacity: None,
		};

		let generic = indexer_progress.to_generic_progress();
		assert_eq!(generic.phase, "Finalizing");
		// With 95/100 progress: 0.99 + (0.95 * 0.01) = 0.9995
		assert!((generic.percentage - 0.9995).abs() < 0.0001);
		assert_eq!(generic.completion.completed, 95);
		assert_eq!(generic.completion.total, 100);
	}
}
