//! FTS5 query utilities.

/// Escape special FTS5 characters and quote the query.
pub fn sanitize_query(query: &str) -> String {
	// Remove FTS5 special characters
	let cleaned: String = query
		.chars()
		.filter(|c| !matches!(c, '"' | '*' | '-' | '+' | '(' | ')' | '~' | '^'))
		.collect();

	// Quote and add wildcards for prefix matching
	format!("\"{}*\"", cleaned.trim())
}
