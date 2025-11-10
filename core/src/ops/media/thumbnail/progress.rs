//! ThumbnailProgress to GenericProgress conversion

use super::job::ThumbnailProgress;
use super::state::ThumbnailPhase;
use crate::infra::job::generic_progress::{GenericProgress, ToGenericProgress};

impl ToGenericProgress for ThumbnailProgress {
	fn to_generic_progress(&self) -> GenericProgress {
		let (percentage, phase_name, phase_message) = match &self.phase {
			ThumbnailPhase::Discovery => {
				let message = format!("Discovering files for thumbnail generation");
				(0.0, "Discovery".to_string(), message)
			}
			ThumbnailPhase::Processing => {
				// Calculate progress based on completed items vs total
				let completed = self.generated_count + self.skipped_count + self.error_count;
				let progress = if self.total_count > 0 {
					completed as f32 / self.total_count as f32
				} else {
					0.0
				};
				// Map to 0-99% range (reserve 100% for complete)
				let percentage = (progress * 0.99).min(0.99);
				let message = if let Some(ref file) = self.current_file {
					format!("Processing thumbnails: {}", file)
				} else {
					format!(
						"Processing thumbnails ({} generated, {} skipped, {} errors)",
						self.generated_count, self.skipped_count, self.error_count
					)
				};
				(percentage, "Processing".to_string(), message)
			}
			ThumbnailPhase::Cleanup => {
				let message = "Cleaning up".to_string();
				(0.99, "Cleanup".to_string(), message)
			}
			ThumbnailPhase::Complete => {
				let message = "Complete".to_string();
				(1.0, "Complete".to_string(), message)
			}
		};

		let completed = self.generated_count + self.skipped_count + self.error_count;

		let mut progress = GenericProgress::new(percentage, &phase_name, &phase_message);

		// Manually set completion without auto-calculating percentage
		progress.completion.completed = completed;
		progress.completion.total = self.total_count;

		progress
			.with_performance(
				// Calculate rate: items per second (estimated)
				if self.total_count > 0 && completed > 0 {
					// Rough estimate - could be improved with actual timing
					completed as f32 / 60.0 // Assume 60 seconds for rough rate
				} else {
					0.0
				},
				self.estimated_time_remaining,
				None,
			)
			.with_errors(self.error_count, 0)
			.with_metadata(self)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	#[test]
	fn test_discovery_phase_conversion() {
		let thumb_progress = ThumbnailProgress {
			phase: ThumbnailPhase::Discovery,
			generated_count: 0,
			skipped_count: 0,
			error_count: 0,
			total_count: 0,
			current_file: None,
			estimated_time_remaining: None,
		};

		let generic = thumb_progress.to_generic_progress();
		assert_eq!(generic.phase, "Discovery");
		assert_eq!(generic.percentage, 0.0);
		assert!(generic.message.contains("Discovering"));
	}

	#[test]
	fn test_processing_phase_conversion() {
		let thumb_progress = ThumbnailProgress {
			phase: ThumbnailPhase::Processing,
			generated_count: 50,
			skipped_count: 10,
			error_count: 5,
			total_count: 100,
			current_file: Some("photo.jpg".to_string()),
			estimated_time_remaining: Some(Duration::from_secs(30)),
		};

		let generic = thumb_progress.to_generic_progress();
		assert_eq!(generic.phase, "Processing");
		// (50 + 10 + 5) / 100 = 0.65, capped at 0.99 for processing
		assert_eq!(generic.percentage, 0.65 * 0.99);
		assert_eq!(generic.completion.completed, 65);
		assert_eq!(generic.completion.total, 100);
		assert_eq!(generic.performance.error_count, 5);
		assert!(generic.message.contains("photo.jpg"));
	}

	#[test]
	fn test_cleanup_phase_conversion() {
		let thumb_progress = ThumbnailProgress {
			phase: ThumbnailPhase::Cleanup,
			generated_count: 95,
			skipped_count: 5,
			error_count: 0,
			total_count: 100,
			current_file: None,
			estimated_time_remaining: None,
		};

		let generic = thumb_progress.to_generic_progress();
		assert_eq!(generic.phase, "Cleanup");
		assert_eq!(generic.percentage, 0.99);
	}

	#[test]
	fn test_complete_phase_conversion() {
		let thumb_progress = ThumbnailProgress {
			phase: ThumbnailPhase::Complete,
			generated_count: 100,
			skipped_count: 0,
			error_count: 0,
			total_count: 100,
			current_file: None,
			estimated_time_remaining: None,
		};

		let generic = thumb_progress.to_generic_progress();
		assert_eq!(generic.phase, "Complete");
		assert_eq!(generic.percentage, 1.0);
	}
}
