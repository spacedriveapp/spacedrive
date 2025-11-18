//! Message tokenization for pattern matching.

use crate::types::Token;

/// Tokenize a message into comparable units.
///
/// Splits on whitespace and punctuation while preserving structure.
pub fn tokenize(message: &str) -> Vec<Token> {
	let mut tokens = Vec::new();
	let mut current = String::new();

	for ch in message.chars() {
		if ch.is_whitespace() || "(){}[],:=".contains(ch) {
			if !current.is_empty() {
				tokens.push(Token::Word(current.clone()));
				current.clear();
			}
			if !ch.is_whitespace() {
				tokens.push(Token::Punctuation(ch));
			}
		} else {
			current.push(ch);
		}
	}

	if !current.is_empty() {
		tokens.push(Token::Word(current));
	}

	tokens
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tokenize_simple() {
		let tokens = tokenize("Recorded ACK from peer");
		assert_eq!(tokens.len(), 4);
		assert_eq!(tokens[0], Token::Word("Recorded".to_string()));
		assert_eq!(tokens[1], Token::Word("ACK".to_string()));
		assert_eq!(tokens[2], Token::Word("from".to_string()));
		assert_eq!(tokens[3], Token::Word("peer".to_string()));
	}

	#[test]
	fn test_tokenize_with_punctuation() {
		let tokens = tokenize("peer=1817e146 hlc=HLC(123,1,:device)");

		// peer = 1817e146 hlc = HLC ( 123 , 1 , : device )
		assert!(tokens.contains(&Token::Word("peer".to_string())));
		assert!(tokens.contains(&Token::Punctuation('=')));
		assert!(tokens.contains(&Token::Punctuation('(')));
		assert!(tokens.contains(&Token::Punctuation(',')));
	}

	#[test]
	fn test_tokenize_empty() {
		let tokens = tokenize("");
		assert_eq!(tokens.len(), 0);
	}
}


