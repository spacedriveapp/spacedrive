//! Sequence pattern detection for repeating log patterns.

use serde::{Deserialize, Serialize};

use crate::types::LogGroup;

/// A detected sequence pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencePattern {
	pub id: u64,
	pub template_sequence: Vec<u64>,
	pub repetitions: usize,
	pub description: String,
	pub group_indices: Vec<usize>,
}

/// Detect repeating sequences in collapsed groups.
pub fn detect_sequences(groups: &[LogGroup]) -> Vec<SequencePattern> {
	let window_sizes = [2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 20]; // Check for 2-20 step sequences
	let mut detected_sequences = Vec::new();
	let mut sequence_id = 0u64;

	for &window_size in &window_sizes {
		if groups.len() < window_size * 2 {
			continue; // Not enough groups for this window size
		}

		let mut used_indices = vec![false; groups.len()];

		// Extract template ID sequence
		let template_ids: Vec<u64> = groups.iter().map(|g| g.template_id).collect();

		let mut i = 0;
		while i + window_size * 2 <= template_ids.len() {
			// Skip if already used
			if used_indices[i] {
				i += 1;
				continue;
			}

			// Get candidate pattern
			let pattern = &template_ids[i..i + window_size];

			// Count repetitions
			let mut repetitions = 0;
			let mut current = i;

			while current + window_size <= template_ids.len() {
				if !used_indices[current]
					&& template_ids[current..current + window_size] == *pattern
				{
				repetitions += 1;
				// Mark as used
				for item in used_indices.iter_mut().skip(current).take(window_size) {
					*item = true;
				}
					current += window_size;
				} else {
					break;
				}
			}

			// If we found a significant pattern (>= 10 repetitions), record it
			if repetitions >= 10 {
				let group_indices: Vec<usize> = (i..i + (window_size * repetitions)).collect();

				detected_sequences.push(SequencePattern {
					id: sequence_id,
					template_sequence: pattern.to_vec(),
					repetitions,
					description: format!("{}-step sequence", window_size),
					group_indices,
				});

				sequence_id += 1;
			}

			i = current;
		}
	}

	detected_sequences
}

/// Calculate compression stats with sequences.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompressionStats {
	pub original_log_count: usize,
	pub group_count: usize,
	pub sequence_count: usize,
	pub final_count: usize,
	pub compression_ratio: f64,
}

pub fn calculate_compression(
	log_count: usize,
	group_count: usize,
	sequences: &[SequencePattern],
) -> CompressionStats {
	// Count groups that are part of sequences
	let mut groups_in_sequences = 0;
	for seq in sequences {
		groups_in_sequences += seq.group_indices.len();
	}

	// Final count = standalone groups + sequences
	// Use saturating_sub to avoid overflow if sequences overlap
	let standalone_groups = group_count.saturating_sub(groups_in_sequences);
	let final_count = standalone_groups + sequences.len();

	CompressionStats {
		original_log_count: log_count,
		group_count,
		sequence_count: sequences.len(),
		final_count,
		compression_ratio: if log_count > 0 {
			1.0 - (final_count as f64 / log_count as f64)
		} else {
			0.0
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	fn create_test_group(template_id: u64) -> LogGroup {
		LogGroup {
			template_id,
			count: 1,
			start_time: Utc::now(),
			end_time: Utc::now(),
			duration_ms: 0,
			variable_stats: Default::default(),
			sample_indices: vec![],
		}
	}

	#[test]
	fn test_detect_simple_sequence() {
		// Pattern: [1, 2] repeating 10 times
		let mut groups = Vec::new();
		for _ in 0..10 {
			groups.push(create_test_group(1));
			groups.push(create_test_group(2));
		}

		let sequences = detect_sequences(&groups);
		assert!(!sequences.is_empty());
		assert_eq!(sequences[0].repetitions, 10);
		assert_eq!(sequences[0].template_sequence, vec![1, 2]);
	}

	#[test]
	fn test_no_sequence_detected() {
		// No repeating pattern
		let groups = vec![
			create_test_group(1),
			create_test_group(2),
			create_test_group(3),
			create_test_group(4),
		];

		let sequences = detect_sequences(&groups);
		assert!(sequences.is_empty()); // < 10 repetitions threshold
	}

	#[test]
	fn test_compression_calculation() {
		let stats = calculate_compression(10000, 100, &[]);
		assert_eq!(stats.final_count, 100);
		assert_eq!(stats.compression_ratio, 0.99);
	}
}
