//! Longest Common Subsequence algorithm for finding common tokens.

use crate::types::Token;

/// Find positions of common tokens across all token sequences.
///
/// Returns a set of positions that have the same token in all sequences.
pub fn find_common_positions(token_sequences: &[Vec<Token>]) -> Vec<usize> {
	if token_sequences.is_empty() {
		return Vec::new();
	}

	if token_sequences.len() == 1 {
		// All positions are "common" for a single sequence
		return (0..token_sequences[0].len()).collect();
	}

	let first = &token_sequences[0];
	let mut common_positions = Vec::new();

	for (pos, token) in first.iter().enumerate() {
		// Check if this token at this position is the same in all sequences
		let all_match = token_sequences
			.iter()
			.skip(1)
			.all(|seq| seq.get(pos).map(|t| t == token).unwrap_or(false));

		if all_match {
			common_positions.push(pos);
		}
	}

	common_positions
}

/// Find variable positions (positions that differ across sequences).
pub fn find_variable_positions(token_sequences: &[Vec<Token>]) -> Vec<usize> {
	if token_sequences.is_empty() {
		return Vec::new();
	}

	let common = find_common_positions(token_sequences);
	let max_len = token_sequences.iter().map(|s| s.len()).max().unwrap_or(0);

	(0..max_len).filter(|pos| !common.contains(pos)).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_common_positions_identical() {
		let seq1 = vec![
			Token::Word("Recorded".to_string()),
			Token::Word("ACK".to_string()),
		];
		let seq2 = vec![
			Token::Word("Recorded".to_string()),
			Token::Word("ACK".to_string()),
		];

		let common = find_common_positions(&[seq1, seq2]);
		assert_eq!(common, vec![0, 1]);
	}

	#[test]
	fn test_find_common_positions_different() {
		let seq1 = vec![
			Token::Word("Recorded".to_string()),
			Token::Word("123".to_string()),
		];
		let seq2 = vec![
			Token::Word("Recorded".to_string()),
			Token::Word("456".to_string()),
		];

		let common = find_common_positions(&[seq1, seq2]);
		assert_eq!(common, vec![0]);
	}

	#[test]
	fn test_find_variable_positions() {
		let seq1 = vec![
			Token::Word("peer".to_string()),
			Token::Punctuation('='),
			Token::Word("123".to_string()),
		];
		let seq2 = vec![
			Token::Word("peer".to_string()),
			Token::Punctuation('='),
			Token::Word("456".to_string()),
		];

		let variable_pos = find_variable_positions(&[seq1, seq2]);
		assert_eq!(variable_pos, vec![2]); // Only position 2 varies
	}
}


